//! Windows：命名管道通信，UAC 提权；虚拟网卡使用随程序分发的 Wintun。

use std::ffi::{CStr, OsStr};
use std::io;
use std::net::Ipv4Addr;
use std::os::windows::ffi::OsStrExt;
use std::os::windows::io::AsRawHandle;

use tokio::net::windows::named_pipe::{ClientOptions, NamedPipeClient, NamedPipeServer, ServerOptions};
use tun_rs::DeviceBuilder;
use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, ERROR_SUCCESS};
use windows_sys::Win32::NetworkManagement::IpHelper::{GetNetworkParams, FIXED_INFO_W2KSP1, IP_ADDR_STRING};
use windows_sys::Win32::System::Pipes::GetNamedPipeClientProcessId;
use windows_sys::Win32::System::Threading::GetProcessId;
use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

use super::helper::{denied, not_started, CONNECT_TIMEOUT};
use crate::error::{AppError, Result};

/// 管道名前缀，后接随机十六进制串。
const PIPE_PREFIX: &str = r"\\.\pipe\shx-tun-";
const PIPE_BUFFER: u32 = 1024 * 1024;
const ADAPTER: &str = "shx";
const WINTUN: &str = "wintun.dll";

pub type Stream = NamedPipeServer;

// ---- 主进程侧 ----

/// 以零结尾的宽字符串。
fn wide(text: &OsStr) -> Vec<u16> {
    text.encode_wide().chain(std::iter::once(0)).collect()
}

/// 通过 UAC 以管理员身份启动，返回进程 id；拒绝时以 Cancelled 失败。
fn elevate(exe: &OsStr, parameters: &str) -> Result<u32> {
    let (verb, file, parameters) = (wide(OsStr::new("runas")), wide(exe), wide(OsStr::new(parameters)));
    // SAFETY: 该结构体只含整数和指针，全零合法。
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    // 取回进程句柄，用于校验管道客户端。
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = parameters.as_ptr();
    info.nShow = SW_HIDE;
    // SAFETY: info 已设置大小，指针指向调用期间有效的零结尾字符串。
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let error = io::Error::last_os_error();
        // 用户拒绝 UAC。
        if error.raw_os_error() == Some(ERROR_CANCELLED as i32) {
            return Err(AppError::Cancelled);
        }
        return Err(denied(error));
    }
    if info.hProcess.is_null() {
        return Err(not_started());
    }
    // SAFETY: 设置了 SEE_MASK_NOCLOSEPROCESS，hProcess 为有效句柄，用后关闭。
    let pid = unsafe {
        let pid = GetProcessId(info.hProcess);
        CloseHandle(info.hProcess);
        pid
    };
    Ok(pid)
}

/// 管道客户端的进程 id。
fn client_pid(pipe: &NamedPipeServer) -> io::Result<u32> {
    let mut pid = 0u32;
    // SAFETY: pipe 存活期间句柄有效，pid 可写。
    if unsafe { GetNamedPipeClientProcessId(pipe.as_raw_handle(), &mut pid) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(pid)
}

pub async fn connect_helper() -> Result<Stream> {
    let name = format!("{PIPE_PREFIX}{}", uuid::Uuid::new_v4().simple());
    // 单实例，只接受一个客户端。
    let pipe = ServerOptions::new()
        .first_pipe_instance(true)
        .reject_remote_clients(true)
        .max_instances(1)
        .in_buffer_size(PIPE_BUFFER)
        .out_buffer_size(PIPE_BUFFER)
        .create(&name)?;

    let exe = std::env::current_exe()?;
    let parameters = format!("--tun-helper {name}");
    // UAC 会阻塞，放到阻塞线程。
    let launching = tokio::task::spawn_blocking(move || elevate(exe.as_os_str(), &parameters));
    let helper = launching.await.map_err(|e| AppError::msg(e.to_string()))??;

    tokio::time::timeout(CONNECT_TIMEOUT, pipe.connect()).await.map_err(|_| not_started())??;
    // 只接受刚启动的助手进程。
    if client_pid(&pipe)? != helper {
        return Err(not_started());
    }
    Ok(pipe)
}

// ---- 助手侧（以管理员身份运行） ----

pub async fn connect(address: &str) -> io::Result<NamedPipeClient> {
    let random = address.strip_prefix(PIPE_PREFIX).unwrap_or_default();
    if random.is_empty() || !random.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a helper pipe"));
    }
    ClientOptions::new().open(address)
}

/// 此平台只支持 `--tun-helper`。
pub fn dispatch(_command: &str) -> Option<i32> {
    None
}

/// 此平台收不到关机通知，也无需额外清理。
pub async fn terminated() {
    std::future::pending().await
}

/// 系统 IPv4 DNS 服务器。
pub fn resolvers() -> Vec<Ipv4Addr> {
    let mut size = 0u32;
    // 先查询所需大小。
    // SAFETY: 无缓冲区时只写入 size。
    unsafe { GetNetworkParams(std::ptr::null_mut(), &mut size) };
    // 用 u64 数组保证对齐。
    let mut buffer = vec![0u64; (size as usize).div_ceil(size_of::<u64>())];
    let info = buffer.as_mut_ptr().cast::<FIXED_INFO_W2KSP1>();
    // SAFETY: info 指向对齐且至少 size 字节的可写内存。
    if (size as usize) < size_of::<FIXED_INFO_W2KSP1>() || unsafe { GetNetworkParams(info, &mut size) } != ERROR_SUCCESS {
        return Vec::new();
    }
    let mut servers = Vec::new();
    // SAFETY: 成功后 DnsServerList 为 buffer 内以空指针结尾的链表，地址为零结尾字符串。
    unsafe {
        let mut entry: *const IP_ADDR_STRING = &raw const (*info).DnsServerList;
        while !entry.is_null() {
            let text = CStr::from_ptr((*entry).IpAddress.String.as_ptr().cast());
            servers.extend(text.to_str().ok().and_then(|text| text.parse::<Ipv4Addr>().ok()));
            entry = (*entry).Next;
        }
    }
    servers
}

/// 建网卡所需的驱动库和网卡名。
pub fn configure(builder: DeviceBuilder) -> std::result::Result<DeviceBuilder, String> {
    // 只加载程序目录下的驱动库。
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

    /// 在真实系统上只读查询。
    #[test]
    fn the_dns_servers_in_use_can_be_listed() {
        let _ = resolvers();
    }

    #[test]
    fn strings_for_windows_end_with_a_zero() {
        assert_eq!(wide(OsStr::new("runas")), [0x72, 0x75, 0x6e, 0x61, 0x73, 0]);
    }
}
