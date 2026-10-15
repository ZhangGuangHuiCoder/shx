//! macOS：Unix socket 通信，osascript 密码框提权。
//!
//! 可安装免密助手（[`install_helper`]）：root 所有、带 setuid 的程序副本。
//! 任何程序都能调用它，因此只允许助手用法（[`dispatch`]），参数仍受 helper.rs 校验约束。

use std::io;
use std::net::Ipv4Addr;
use std::os::unix::fs::{DirBuilderExt, MetadataExt};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use tokio::net::{UnixListener, UnixStream};
use tokio::signal::unix::{signal, SignalKind};
use tun_rs::DeviceBuilder;

use super::helper::{denied, not_started, CONNECT_TIMEOUT};
use super::HelperState;
use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::ssh::sh_quote;

const OSASCRIPT: &str = "/usr/bin/osascript";
/// 免密助手路径，目录仅 root 可写。
const INSTALLED_DIR: &str = "/Library/PrivilegedHelperTools";
const INSTALLED: &str = "/Library/PrivilegedHelperTools/cn.ofis.shx.tun-helper";
/// 让免密助手卸载自身的参数。
const REMOVE: &str = "--tun-helper-remove";

pub type Stream = UnixStream;

// ---- 主进程侧 ----

/// 仅本用户可访问的临时目录，存放 socket，用完即删。
struct SocketDir(PathBuf);

impl SocketDir {
    fn create() -> io::Result<Self> {
        let id = uuid::Uuid::new_v4().simple().to_string();
        // socket 路径长度有限（约 100 字节）。
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

/// AppleScript 字符串字面量转义。
fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

/// 以管理员身份执行命令的 AppleScript，prompt 为密码框说明。
fn elevation_script(command: &str, prompt: &str) -> String {
    format!("do shell script {} with prompt {} with administrator privileges", applescript_string(command), applescript_string(prompt))
}

fn helper_command(exe: &Path, args: &[&str]) -> String {
    let mut command = sh_quote(&exe.to_string_lossy());
    for arg in args {
        command.push(' ');
        command.push_str(&sh_quote(arg));
    }
    command
}

/// 授权后以管理员身份执行命令并等待完成。
async fn elevate(command: String, prompt: &str) -> Result<()> {
    let script = elevation_script(&command, prompt);
    let run = tokio::task::spawn_blocking(move || Command::new(OSASCRIPT).args(["-e", &script]).output());
    let output = run.await.map_err(|e| AppError::msg(e.to_string()))??;
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    // 用户取消授权。
    if detail.contains("-128") {
        return Err(AppError::Cancelled);
    }
    Err(denied(detail.trim()))
}

pub async fn connect_helper() -> Result<Stream> {
    let dir = SocketDir::create()?;
    let socket = dir.socket();
    let listener = UnixListener::bind(&socket)?;

    let exe = std::env::current_exe()?;
    let socket = socket.to_string_lossy();
    let args = ["--tun-helper", &*socket];
    if helper_installed() {
        // 版本过旧，需更新并授权。
        let latest = exe.clone();
        if !blocking(move || same_file(&latest, Path::new(INSTALLED))).await? {
            install(&exe).await?;
        }
        let mut helper = Command::new(INSTALLED).args(args).stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn()?;
        // 回收子进程，避免僵尸进程。
        std::thread::spawn(move || helper.wait());
    } else {
        let prompt = tr("Shell Xplore 需要管理员权限以创建虚拟网卡和路由。", "Shell Xplore requires administrator privileges to create a virtual interface and routes.");
        // 助手放到后台运行，授权命令立即返回。
        elevate(format!("{} >/dev/null 2>&1 &", helper_command(&exe, &args)), prompt).await?;
    }

    let (stream, _) = tokio::time::timeout(CONNECT_TIMEOUT, listener.accept()).await.map_err(|_| not_started())??;
    // 只接受刚授权启动的助手。
    if stream.peer_cred()?.uid() != 0 {
        return Err(not_started());
    }
    Ok(stream)
}

// ---- 安装免密助手 ----

async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> Result<T> {
    tokio::task::spawn_blocking(work).await.map_err(|e| AppError::msg(e.to_string()))
}

fn privileged(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|meta| meta.is_file() && meta.uid() == 0 && meta.mode() & 0o4000 != 0)
}

fn same_file(a: &Path, b: &Path) -> bool {
    let size = |path: &Path| std::fs::metadata(path).map(|meta| meta.len()).ok();
    size(a).is_some() && size(a) == size(b) && std::fs::read(a).ok() == std::fs::read(b).ok()
}

pub fn helper_installed() -> bool {
    privileged(Path::new(INSTALLED))
}

/// 已安装但与本程序不一致时报告为 Outdated（下次仍需授权）。
pub fn helper_state() -> HelperState {
    if !helper_installed() {
        return HelperState::Missing;
    }
    match std::env::current_exe() {
        Ok(exe) if same_file(&exe, Path::new(INSTALLED)) => HelperState::Installed,
        _ => HelperState::Outdated,
    }
}

async fn install(exe: &Path) -> Result<()> {
    let prompt = tr("Shell Xplore 将安装隧道助手，之后开启隧道无需密码。", "Shell Xplore will install the tunnel helper; turning on the tunnel will no longer require a password.");
    let copy = helper_command(Path::new("/usr/bin/install"), &["-o", "root", "-g", "wheel", "-m", "4755", &exe.to_string_lossy(), INSTALLED]);
    elevate(format!("/bin/mkdir -p {INSTALLED_DIR} && {copy}"), prompt).await
}

/// 安装免密助手（需最后一次授权）。
pub async fn install_helper() -> Result<()> {
    install(&std::env::current_exe()?).await
}

/// 卸载免密助手，由其自行删除，无需授权。
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

// ---- 助手侧（以 root 身份运行） ----

extern "C" {
    fn getuid() -> u32;
    fn geteuid() -> u32;
}

/// 免密助手处理 `--tun-helper` 以外的命令；否则返回 None。
pub fn dispatch(command: &str) -> Option<i32> {
    // SAFETY: 无参数且不会失败的系统调用。
    let setuid = unsafe { geteuid() == 0 && getuid() != 0 };
    if !setuid {
        return None;
    }
    // setuid 副本只允许助手用法，拒绝其他用法。
    Some(match command {
        REMOVE => i32::from(std::fs::remove_file(INSTALLED).is_err()),
        _ => 1,
    })
}

pub async fn connect(address: &str) -> io::Result<Stream> {
    UnixStream::connect(address).await
}

/// 收到终止信号时返回。
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

/// 从 resolv.conf 读取系统 IPv4 DNS 服务器。
pub fn resolvers() -> Vec<Ipv4Addr> {
    std::fs::read_to_string("/etc/resolv.conf").map(|text| nameservers(&text)).unwrap_or_default()
}

fn nameservers(resolv_conf: &str) -> Vec<Ipv4Addr> {
    let server = |line: &str| {
        let mut words = line.split_whitespace();
        (words.next() == Some("nameserver")).then(|| words.next()?.parse().ok()).flatten()
    };
    resolv_conf.lines().filter_map(server).collect()
}

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
        // 反斜杠和双引号须转义。
        let script = elevation_script(&command, "say \"yes\"");
        assert!(script.starts_with(r#"do shell script "'/Applications/My \"Apps\"/it'\\''s shx.app"#));
        assert!(script.ends_with(r#" with prompt "say \"yes\"" with administrator privileges"#));
        assert_eq!(script.matches('"').count() - script.matches("\\\"").count(), 4);
    }

    /// 只编译不执行，验证语法。
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
    fn the_dns_servers_in_use_are_read_from_the_systems_list() {
        let text = "# macOS Notice\nsearch lan\nnameserver 192.168.1.1\nnameserver fe80::1%en0\n nameserver\t223.5.5.5 # backup\nnameserver\noptions ndots:1\n";
        assert_eq!(nameservers(text), ["192.168.1.1".parse::<Ipv4Addr>().unwrap(), "223.5.5.5".parse().unwrap()]);
        assert_eq!(nameservers(""), Vec::<Ipv4Addr>::new());
        // 在真实系统上只读。
        let _ = resolvers();
    }

    #[test]
    fn only_a_root_owned_setuid_file_counts_as_the_installed_helper() {
        let dir = SocketDir::create().unwrap();
        let (copy, other) = (dir.0.join("a"), dir.0.join("b"));
        std::fs::write(&copy, "helper").unwrap();
        std::fs::write(&other, "helpex").unwrap();
        // 非 root 所有、不存在或目录都不算。
        assert!(!privileged(&copy) && !privileged(&dir.0) && !privileged(&dir.0.join("missing")));
        assert!(privileged(Path::new("/usr/bin/sudo")));

        assert!(same_file(&copy, &copy) && !same_file(&copy, &other) && !same_file(&copy, &dir.0.join("missing")));
        assert!(!same_file(&dir.0.join("missing"), &dir.0.join("missing")));
        let _ = (std::fs::remove_file(copy), std::fs::remove_file(other));
    }

    /// 在真实系统上只读查询。
    #[test]
    fn a_helper_that_is_not_this_very_program_does_not_count_as_installed() {
        // 测试进程不是已安装的副本，最多为 Outdated。
        let expected = if helper_installed() { HelperState::Outdated } else { HelperState::Missing };
        assert_eq!(helper_state(), expected);
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
