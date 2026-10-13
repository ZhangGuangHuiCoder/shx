//! Windows 上主进程和助手怎么接上头：经一个命名管道对话，提权靠 UAC。
//! 虚拟网卡是 Wintun，`wintun.dll` 随程序分发。

use std::ffi::OsStr;
use std::io;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions};
use tun_rs::DeviceBuilder;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows_sys::Win32::System::Threading::GetProcessId;
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use super::helper::{denied, not_started, CONNECT_TIMEOUT};
use crate::error::{AppError, Result};

/// 主进程等助手来连的管道，名字后面跟一串随机的十六进制数。
const PIPE_PREFIX: &str = r"\\.\pipe\shx-tun-";
const PIPE_BUFFER: u32 = 1024 * 1024;
/// 虚拟网卡在系统里的名字。
const ADAPTER: &str = "shx";
const WINTUN: &str = "wintun.dll";

/// 主进程和助手之间的连接（主进程这一头）。
pub type Stream = NamedPipeServer;

// ---- 主进程：启动助手 ---------------------------------------------------------

/// 以零结尾的宽字符串，Windows 的接口要这种写法。
fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// 请用户授权（UAC），以管理员身份启动 `exe`，返回新进程的编号。用户不同意时以 `Cancelled` 失败。
fn elevate(exe: &OsStr, parameters: &str) -> Result<u32> {
    let (verb, file, parameters) = (wide(OsStr::new("runas")), wide(exe), wide(OsStr::new(parameters)));
    // SAFETY: 全零对这个只含整数和指针的结构体是合法的值。
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    // 要回新进程的句柄，好知道连上管道的是不是它。
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = SW_HIDE;
    // SAFETY: `info` 填好了大小，里面的指针指向在调用期间一直有效的、以零结尾的字符串。
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let error = io::Error::last_os_error();
        // 用户在 UAC 的提示里点了“否”。
        if error.raw_os_error() == Some(ERROR_CANCELLED as i32) {
            return Err(AppError::Cancelled);
        }
        return Err(denied(error));
    }
    if info.hProcess.is_null() {
        return Err(not_started());
    }
    // SAFETY: 调用成功并带了 SEE_MASK_NOCLOSEPROCESS，`hProcess` 是归我们所有的有效句柄，用完关掉。
    let pid = unsafe {
        let pid = GetProcessId(info.hProcess);
        CloseHandle(info.hProcess);
        pid
    };
    Ok(pid)
}

/// 连在管道另一头的进程的编号。
fn client_pid(pipe: &NamedPipeServer) -> io::Result<u32> {
    let mut pid = 0u32;
    // SAFETY: 管道的句柄在 `pipe` 活着期间有效，`pid` 是可写的。
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(pid)
}

/// 请用户授权并启动助手，等它连上来。
pub async fn connect_helper() -> Result<Stream> {
    let name = format!("{PIPE_PREFIX}{}", uuid::Uuid::new_v4().simple());
    // 只此一个实例：这个名字的管道不会有第二条，也只接一个客户端。
    let pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .max_instances(1)
        .in_buffer_size(PIPE_BUFFER)
        .out_buffer_size(PIPE_BUFFER)
        .create(&name)?;

    let exe = std::env::current_exe()?;
    let parameters = format!("--tun-helper {name}");
    // UAC 的提示要等用户回答，这期间不能占着异步运行时的线程。
    let launching = tokio::task::spawn_blocking(move || elevate(exe.as_os_str(), &parameters));
    let helper = launching.await.map_err(|e| AppError::msg(e.to_string()))??;

    tokio::time::timeout(CONNECT_TIMEOUT, pipe.connect()).await.map_err(|_| not_started())??;
    // 别的进程也可能抢先连上这个管道；只认刚才授权启动的那一个。
    if client_pid(&pipe)? != helper {
        return Err(not_started());
    }
    Ok(pipe)
}

// ---- 助手：以下都以管理员身份运行 ---------------------------------------------

/// 连到主进程等着的那个管道。
pub async fn connect(address: &str) -> io::Result<NamedPipeClient> {
    let random = address.strip_prefix(PIPE_PREFIX).unwrap_or_default();
    if random.is_empty() || !random.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a helper pipe"));
    }
    ClientOptions::new().open(address)
}

/// 这个系统上助手只有 `--tun-helper` 一种用法。
pub fn dispatch(_command: &str) -> Option<i32> {
    None
}

/// 这个系统上助手收不到“要关机了”之类的通知；它也没有留下什么需要自己来收拾的东西。
pub async fn terminated() {
    std::future::pending().await
}

/// 这个系统上建网卡要额外交代的：用哪份驱动库，网卡叫什么。
pub fn configure(builder: DeviceBuilder) -> std::result::Result<DeviceBuilder, String> {
    // 驱动库只认和本程序放在一起的那一份，不到别处去找。
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe.parent().ok_or("the program has no directory")?;
    let library = [dir.join(WINTUN), dir.join("resources").join(WINTUN)].into_iter().find(|path| path.is_file());
    let library = library.ok_or_else(|| format!("{WINTUN} is missing next to the program"))?;
    Ok(builder.name(ADAPTER).wintun_file(library.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn only_this_programs_pipes_are_connected_to() {
        for bad in ["", r"\\.\pipe\shx-tun-", r"\\.\pipe\other", r"\\.\pipe\shx-tun-..\x", r"\\server\pipe\shx-tun-00ff", "C:\\file"] {
            assert_eq!(connect(bad).await.unwrap_err().kind(), io::ErrorKind::InvalidInput, "{bad:?}");
        }
    }

    #[test]
    fn strings_for_windows_end_with_a_zero() {
        assert_eq!(wide(OsStr::new("runas")), [0x72, 0x75, 0x6e, 0x61, 0x73, 0]);
    }
}
