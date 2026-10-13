//! macOS 上主进程和助手怎么接上头：经一个 Unix socket 对话，提权靠系统的密码框（`osascript`）。
//!
//! 每次开隧道都要输一遍密码。不想每次都输的，可以把助手装进系统（[`install_helper`]）：
//! 本程序的一份副本，归 root 所有并带 setuid 位，谁运行它都是管理员身份，此后不再询问。
//! 代价是它认不出运行它的是谁：这台机器上的任何程序都能叫它建网卡、加路由。所以它只当助手用
//! （见 [`dispatch`]），做的事和每一项参数照旧受 `helper.rs` 里那些校验的约束。

use std::io;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{signal, SignalKind};
use tun_rs::DeviceBuilder;

use super::helper::{denied, not_started, CONNECT_TIMEOUT};
use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::ssh::sh_quote;

const OSASCRIPT: &str = "/usr/bin/osascript";
/// 装进系统的助手。所在的目录只有 root 能写。
const INSTALLED_DIR: &str = "/Library/PrivilegedHelperTools";
const INSTALLED: &str = "/Library/PrivilegedHelperTools/cn.ofis.shx.tun-helper";
/// 叫装进系统的助手把自己删掉。
const REMOVE: &str = "--tun-helper-remove";

/// 主进程和助手之间的连接。
pub type Stream = UnixStream;

// ---- 主进程：启动助手 ---------------------------------------------------------

/// 只有本用户进得去的临时目录，里面是等助手来连的 socket；用完即删。
struct SocketDir(PathBuf);

impl SocketDir {
    fn create() -> io::Result<Self> {
        let id = uuid::Uuid::new_v4().simple().to_string();
        // socket 的路径长度有限（约一百字节），名字尽量短。
        let dir = std::env::temp_dir().join(format!("shx-{}", &id[..8]));
        std::fs::DirBuilder::new().mode(0o700).create(&dir)?;
        Ok(Self(dir))
    }

    fn socket(&self) -> PathBuf {
        self.0.join("tun.sock")
    }
}

impl Drop for SocketDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.socket());
        let _ = std::fs::remove_dir(&self.0);
    }
}

/// 写成 AppleScript 的字符串字面量。
fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 以管理员身份执行 `command` 的 AppleScript。系统会弹出密码框，`prompt` 是上面的说明。
fn elevation_script(command: &str, prompt: &str) -> String {
    format!("do shell script {} with prompt {} with administrator privileges", applescript_string(command), applescript_string(prompt))
}

/// 本程序以助手身份运行的 shell 命令。
fn helper_command(exe: &Path, args: &[&str]) -> String {
    let mut command = sh_quote(&exe.to_string_lossy());
    for arg in args {
        command.push(' ');
        command.push_str(&sh_quote(arg));
    }
    command
}

/// 请用户授权（密码框上的说明是 `prompt`），然后以管理员身份执行 `command`；等它执行完。
async fn elevate(command: String, prompt: &str) -> Result<()> {
    let script = elevation_script(&command, prompt);
    let run = tokio::task::spawn_blocking(move || Command::new(OSASCRIPT).args(["-e", &script]).output());
    let output = run.await.map_err(|e| AppError::msg(e.to_string()))??;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    // 用户在密码框里点了“取消”。
    if detail.contains("-128") {
        return Err(AppError::Cancelled);
    }
    Err(denied(detail.trim()))
}

/// 请用户授权并启动助手，等它连上来。
pub async fn connect_helper() -> Result<Stream> {
    let dir = SocketDir::create()?;
    let socket = dir.socket();
    let listener = UnixListener::bind(&socket)?;

    let exe = std::env::current_exe()?;
    let socket = socket.to_string_lossy();
    let args = ["--tun-helper", &*socket];
    if helper_installed() {
        // 应用更新过：副本换成新的，这一次要输密码。
        let latest = exe.clone();
        if !blocking(move || same_file(&latest, Path::new(INSTALLED))).await? {
            install(&exe).await?;
        }
        let mut helper = Command::new(INSTALLED).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        // 它退出时得有人收走，否则会留下一个僵尸进程。
        std::thread::spawn(move || helper.wait());
    } else {
        let prompt = tr("shx 需要管理员权限以创建虚拟网卡和路由。", "shx requires administrator privileges to create a virtual interface and routes.");
        // 助手要一直运行到隧道关闭：放到后台，授权的那条命令立刻返回。
        elevate(format!("{} >/dev/null 2>&1 &", helper_command(&exe, &args)), prompt).await?;
    }

    let (stream, _) = tokio::time::timeout(CONNECT_TIMEOUT, listener.accept()).await.map_err(|_| not_started())??;
    // 能连这个 socket 的还有本用户的其他进程；只认刚才授权的那个。
    if stream.peer_cred()?.uid() != 0 {
        return Err(not_started());
    }
    Ok(stream)
}

// ---- 主进程：把助手装进系统 ---------------------------------------------------

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|e| AppError::msg(e.to_string()))
}

/// `path` 是归 root 所有、带 setuid 位的普通文件。
fn privileged(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.uid() == 0 && meta.mode() & 0o4000 != 0)
}

fn same_file(a: &Path, b: &Path) -> bool {
    let size = |path: &Path| std::fs::metadata(path).map(|meta| meta.len()).ok();
    size(a).is_some() && size(a) == size(b) && std::fs::read(a).ok() == std::fs::read(b).ok()
}

/// 助手是不是装进了系统。
pub fn helper_installed() -> bool {
    privileged(Path::new(INSTALLED))
}

/// 把 `exe` 装成系统里的助手。
async fn install(exe: &Path) -> Result<()> {
    let prompt = tr("shx 将安装隧道助手，之后开启隧道无需密码。", "shx will install the tunnel helper; turning on the tunnel will no longer require a password.");
    let copy = helper_command(Path::new("/usr/bin/install"), &["-o", "root", "-g", "wheel", "-m", "4755", &exe.to_string_lossy(), INSTALLED]);
    elevate(format!("/bin/mkdir -p {INSTALLED_DIR} && {copy}"), prompt).await
}

/// 把助手装进系统；会请用户授权，这是最后一次。
pub async fn install_helper() -> Result<()> {
    install(&std::env::current_exe()?).await
}

/// 把装进系统的助手删掉。不用授权：它有管理员身份，自己删自己。
pub async fn remove_helper() -> Result<()> {
    if !helper_installed() {
        return Ok(());
    }
    let removed = blocking(|| Command::new(INSTALLED).arg(REMOVE).status()).await??;
    if removed.success() {
        return Ok(());
    }
    Err(AppError::msg(tr("无法移除隧道助手", "Could not remove the tunnel helper")))
}

// ---- 助手：以下都以 root 身份运行 ---------------------------------------------

extern "C" {
    fn getuid() -> u32;
    fn geteuid() -> u32;
}

/// 以装进系统的助手的身份运行时，处理 `--tun-helper` 之外的命令；否则返回 None。
pub fn dispatch(command: &str) -> Option<i32> {
    // SAFETY: 这两个系统调用没有参数，也不会失败。
    let setuid = unsafe { geteuid() == 0 && getuid() != 0 };
    if !setuid {
        return None;
    }
    // 这份副本只当助手用：别的用法（比如当成应用启动）一律拒绝，它不该以管理员身份去做。
    Some(match command {
        REMOVE => i32::from(std::fs::remove_file(INSTALLED).is_err()),
        _ => 1,
    })
}

/// 连到主进程等着的那个 socket。
pub async fn connect(address: &str) -> io::Result<Stream> {
    UnixStream::connect(address).await
}

/// 收到终止信号（关机、注销、被人结束）时返回。
pub async fn terminated() {
    let signals = (signal(SignalKind::terminate()), signal(SignalKind::interrupt()), signal(SignalKind::hangup()));
    let (Ok(mut terminate), Ok(mut interrupt), Ok(mut hangup)) = signals else {
        return std::future::pending().await;
    };
    tokio::select! {
        _ = terminate.recv() => {}
        _ = interrupt.recv() => {}
        _ = hangup.recv() => {}
    }
}

/// 这个系统上建网卡不需要额外交代什么。
pub fn configure(builder: DeviceBuilder) -> std::result::Result<DeviceBuilder, String> {
    Ok(builder)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_helper_command_survives_both_layers_of_quoting() {
        let exe = Path::new("/Applications/My \"Apps\"/it's shx.app/Contents/MacOS/shx");
        let command = helper_command(exe, &["--tun-helper", "/var/folders/ab/T/shx-1a2b3c4d/tun.sock"]);
        assert_eq!(
            command,
            r#"'/Applications/My "Apps"/it'\''s shx.app/Contents/MacOS/shx' '--tun-helper' '/var/folders/ab/T/shx-1a2b3c4d/tun.sock'"#
        );
        // 进了 AppleScript 的字符串之后，反斜杠和双引号都要转义，引号成对。
        let script = elevation_script(&command, "say \"yes\"");
        assert!(script.starts_with(r#"do shell script "'/Applications/My \"Apps\"/it'\\''s shx.app"#));
        assert!(script.ends_with(r#" with prompt "say \"yes\"" with administrator privileges"#));
        assert_eq!(script.matches('"').count() - script.matches("\\\"").count(), 4);
    }

    /// 生成的脚本语法上成立（只编译，不执行）。
    #[test]
    fn the_elevation_script_compiles() {
        let command = helper_command(Path::new("/tmp/it's \"shx\""), &["--tun-helper", "/tmp/a b/tun.sock"]);
        let script = elevation_script(&format!("{command} >/dev/null 2>&1 &"), "shx 需要管理员权限。");
        let out = std::env::temp_dir().join(format!("shx-test-{}.scpt", uuid::Uuid::new_v4()));
        let status = Command::new("/usr/bin/osacompile").args(["-e", &script, "-o"]).arg(&out).status().unwrap();
        let _ = std::fs::remove_file(&out);
        assert!(status.success());
    }

    #[test]
    fn only_a_root_owned_setuid_file_counts_as_the_installed_helper() {
        let dir = SocketDir::create().unwrap();
        let (copy, other) = (dir.0.join("a"), dir.0.join("b"));
        std::fs::write(&copy, "helper").unwrap();
        std::fs::write(&other, "helpex").unwrap();
        // 自己建的文件不归 root 所有；不存在的、目录也都不算。
        assert!(!privileged(&copy) && !privileged(&dir.0) && !privileged(&dir.0.join("missing")));
        assert!(privileged(Path::new("/usr/bin/sudo")));

        assert!(same_file(&copy, &copy) && !same_file(&copy, &other) && !same_file(&copy, &dir.0.join("missing")));
        assert!(!same_file(&dir.0.join("missing"), &dir.0.join("missing")));
        let _ = (std::fs::remove_file(copy), std::fs::remove_file(other));
    }

    #[test]
    fn the_app_itself_is_not_taken_for_the_installed_helper() {
        assert_eq!(dispatch(REMOVE), None);
        assert_eq!(dispatch(""), None);
    }

    #[test]
    fn the_socket_directory_is_private_and_cleans_up_after_itself() {
        use std::os::unix::fs::PermissionsExt;
        let dir = SocketDir::create().unwrap();
        let path = dir.0.clone();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o700);
        assert!(dir.socket().to_string_lossy().len() < 100, "must fit in a socket address");
        std::fs::write(dir.socket(), "").unwrap();
        drop(dir);
        assert!(!path.exists());
    }
}
