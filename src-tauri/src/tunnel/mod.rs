//! TUN 模式：类似 VPN，本机的流量经 SSH 服务器出去。
//!
//! 一块虚拟网卡加上几条路由，把流量引到本程序手里；用户态协议栈把它还原成一条条 TCP 连接，
//! 每条各开一个 SSH 的 direct-tcpip 通道转发。只管 IPv4 的 TCP：SSH 带不了别的，UDP 和 ICMP
//! 丢掉（DNS 查询除外），IPv6 的流量不经过隧道。服务器上不需要 root，也不需要装任何东西。
//!
//! 除了那块网卡和指向它的路由，不改系统的任何设置；网卡一撤，路由跟着消失，一切复原。
//!
//! - 本文件：隧道的开关和状态，给前端的命令。
//! - `route.rs`：哪些网段进隧道，系统的路由表。
//! - `stack.rs`：用户态协议栈，TCP 和 DNS 的转发。
//! - `helper.rs`：建网卡、加路由需要管理员权限，由提权的助手进程来做；`wire.rs` 是它和
//!   主进程之间的约定。
//! - `macos.rs`、`windows.rs`：怎么提权、主进程和助手经什么对话。别的系统上还没有实现。

// 没有实现的系统上，只有助手才用得到的部分是闲置的。
#![cfg_attr(not(any(target_os = "macos", windows)), allow(dead_code))]

mod route;
mod stack;
mod wire;

#[cfg(any(target_os = "macos", windows))]
pub mod helper;
#[cfg(target_os = "macos")]
#[path = "macos.rs"]
mod os;
#[cfg(windows)]
#[path = "windows.rs"]
mod os;

use std::io;
use std::net::{IpAddr, Ipv4Addr};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager as _, State};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use self::route::Cidr;
use self::stack::{Conn, Dialer, Tunnel};
use self::wire::FrameWriter;
use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::ssh::{AppState, Session};

/// 虚拟网卡的 MTU。连接在本机就终结了，包不会原样上路，所以可以开得很大：包少，开销就小。
pub const MTU: u16 = 9000;
/// 虚拟网卡自己的地址，取自保留给测试用途、公网上不会出现的网段。
pub const TUN_ADDR: Ipv4Addr = Ipv4Addr::new(198, 19, 255, 253);
pub const TUN_PREFIX: u8 = 30;
/// 进隧道的网段挖得太碎时，路由的条数上限。
pub const MAX_ROUTES: usize = 1024;

/// 一次最多填这么多个网段。
const MAX_NETWORKS: usize = 256;
/// 关闭时等助手撤掉网卡的时间上限。
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(15);
/// 开启前试开一条通道的时间上限。
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);

/// 其他系统上还没有实现。
#[cfg(not(any(target_os = "macos", windows)))]
mod helper {
    use super::*;

    pub struct Link {
        pub device: String,
        pub warning: Option<String>,
        pub stream: tokio::io::DuplexStream,
    }

    pub async fn open(_routes: Vec<Cidr>) -> Result<Link> {
        Err(AppError::msg(tr("当前系统暂不支持隧道", "The tunnel is not supported on this system")))
    }
}

#[derive(Deserialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
enum RouteMode {
    /// 全局：所有流量进隧道。
    All,
    /// 分流：只有列出的网段进隧道。
    Include,
}

/// 前端给出的隧道配置。
#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    route: RouteMode,
    /// 分流模式下进隧道的网段。
    #[serde(default)]
    cidrs: Vec<String>,
    /// 不进隧道的网段。
    #[serde(default)]
    bypass: Vec<String>,
}

fn networks(list: &[String]) -> Result<Vec<Cidr>> {
    if list.len() > MAX_NETWORKS {
        return Err(AppError::msg(tr("网段数量超出上限", "Too many networks")));
    }
    // 隧道只管 IPv4：IPv6 的网段在这里和写错了的一样对待。
    let parse = |text: &String| text.parse().map_err(|_| AppError::msg(format!("{}{text}", tr("无效的 IPv4 网段：", "Invalid IPv4 network: "))));
    list.iter().map(parse).collect()
}

/// 进隧道的网段，每个加一条指向虚拟网卡的路由。`server` 是 SSH 服务器的地址：承载隧道的
/// 连接自己不能进隧道。
fn routes(config: &Config, server: IpAddr) -> Result<Vec<Cidr>> {
    let include = match config.route {
        RouteMode::All => None,
        RouteMode::Include if config.cidrs.is_empty() => {
            return Err(AppError::msg(tr("未指定隧道网段", "No tunnelled networks specified")));
        }
        RouteMode::Include => Some(networks(&config.cidrs)?),
    };
    let routes = route::plan(include.as_deref(), &networks(&config.bypass)?, server);
    // 全都绕开了，或者挖得太碎：说清楚，而不是开一条什么都不走的隧道。
    if routes.is_empty() {
        return Err(AppError::msg(tr("隧道网段已全部被排除", "All tunnelled networks are excluded")));
    }
    if routes.len() > MAX_ROUTES {
        return Err(AppError::msg(tr("排除网段过多，请减少或合并", "Too many excluded networks; remove or merge some")));
    }
    Ok(routes)
}

#[derive(Clone, Copy, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
enum StateKind {
    Running,
    /// 承载隧道的 SSH 连接断了，等前端重连。这期间流量仍被引进隧道，只是哪儿也去不了。
    Reconnecting,
    Stopped,
}

/// 隧道的状态，随 `tunnel-update` 事件发给前端。
#[derive(Clone, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    state: StateKind,
    /// 虚拟网卡的名字。
    device: String,
    /// SSH 服务器的地址；重连时必须连回同一个地址，只有它绕开了隧道。
    server: String,
    warning: Option<String>,
    /// 隧道意外中断的原因。
    error: Option<String>,
    connections: u64,
    up: u64,
    down: u64,
}

/// 一条运行中的隧道。
struct Core {
    device: String,
    server: IpAddr,
    warning: Option<String>,
    /// 承载隧道的 SSH 连接；断开之后、接上新的之前是 None。
    session: Arc<RwLock<Option<Arc<Session>>>>,
    tunnel: Arc<Tunnel>,
}

impl Core {
    fn status(&self, state: StateKind, error: Option<String>) -> Status {
        Status {
            state,
            device: self.device.clone(),
            server: self.server.to_string(),
            warning: self.warning.clone(),
            error,
            connections: self.tunnel.connections.load(Ordering::Relaxed),
            up: self.tunnel.up.load(Ordering::Relaxed),
            down: self.tunnel.down.load(Ordering::Relaxed),
        }
    }

    fn current(&self) -> Status {
        let state = if self.session.read().is_some() { StateKind::Running } else { StateKind::Reconnecting };
        self.status(state, None)
    }
}

struct Active {
    core: Arc<Core>,
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}

/// 机器上同一时刻只有一条隧道。
#[derive(Default)]
pub struct Manager {
    active: Mutex<Option<Active>>,
    /// 正在启动（等用户授权、等助手准备）：这期间不能再启动一条。
    starting: AtomicBool,
}

struct Starting<'a>(&'a AtomicBool);

impl Drop for Starting<'_> {
    fn drop(&mut self) {
        self.0.store(false, Ordering::SeqCst);
    }
}

impl Manager {
    fn begin(&self) -> Result<Starting<'_>> {
        if self.active.lock().is_some() || self.starting.swap(true, Ordering::SeqCst) {
            return Err(AppError::msg(tr("隧道已在运行", "The tunnel is already running")));
        }
        Ok(Starting(&self.starting))
    }
}

/// 每条连接各开一个 direct-tcpip 通道，由 `slot` 里当前的 SSH 连接承载。
fn ssh_dialer(slot: Arc<RwLock<Option<Arc<Session>>>>) -> Dialer {
    Arc::new(move |host, port, from| {
        let session = slot.read().clone();
        Box::pin(async move {
            let session = session.ok_or(io::ErrorKind::NotConnected)?;
            let opening = session.handle.channel_open_direct_tcpip(host, u32::from(port), from.ip().to_string(), u32::from(from.port()));
            Ok(Box::new(opening.await.map_err(io::Error::other)?.into_stream()) as Conn)
        })
    })
}

/// 服务器明确拒绝转发：`AllowTcpForwarding no` 之类。连不上、超时都不算。
fn forbidden(error: &russh::Error) -> bool {
    matches!(error, russh::Error::ChannelOpenFailure(russh::ChannelOpenFailure::AdministrativelyProhibited))
}

/// 服务器让不让转发。不让的话隧道开起来什么都不通，而每条连接各自失败时没有地方报告：
/// 趁还没去打扰用户授权，先试开一条通道问清楚。目标是服务器上的 sshd 自己；只认“明确拒绝”
/// 这一种答复，别的结果说明不了什么，照常往下走。
async fn check_forwarding(session: &Session) -> Result<()> {
    let opening = session.handle.channel_open_direct_tcpip("127.0.0.1", u32::from(session.peer.port()), "127.0.0.1", 0);
    match tokio::time::timeout(PROBE_TIMEOUT, opening).await {
        Ok(Ok(channel)) => {
            let _ = channel.close().await;
            Ok(())
        }
        Ok(Err(error)) if forbidden(&error) => Err(AppError::msg(tr(
            "服务器禁止 TCP 转发（AllowTcpForwarding），隧道无法使用",
            "The server does not allow TCP forwarding (AllowTcpForwarding); the tunnel cannot work",
        ))),
        _ => Ok(()),
    }
}

/// 每秒看一眼：SSH 连接还在不在，状态和流量有没有变化。永不返回。
async fn watch(app: &AppHandle, core: &Core) {
    let mut last = core.current();
    let mut tick = tokio::time::interval(Duration::from_secs(1));
    loop {
        tick.tick().await;
        let lost = core.session.read().as_ref().is_some_and(|session| session.handle.is_closed());
        if lost {
            *core.session.write() = None;
        }
        let status = core.current();
        if status != last {
            let _ = app.emit("tunnel-update", &status);
            last = status;
        }
    }
}

/// 守着一条隧道直到它结束：被要求关闭，或者和助手之间的链路断了。
async fn supervise<S>(app: AppHandle, core: Arc<Core>, stream: S, stopped: oneshot::Receiver<()>)
where
    S: AsyncRead + AsyncWrite + Send + Unpin,
{
    let (mut reader, writer) = tokio::io::split(stream);
    let mut writer = FrameWriter::new(writer);
    let broken = tokio::select! {
        error = stack::run(&mut reader, &mut writer, core.tunnel.clone()) => Some(error),
        _ = stopped => None,
        _ = watch(&app, &core) => None,
    };
    // 请助手收尾，并等它做完：它撤掉网卡之后才会关掉这条连接。
    let cleanup = async {
        let _ = writer.close().await;
        let _ = tokio::io::copy(&mut reader, &mut tokio::io::sink()).await;
    };
    let _ = tokio::time::timeout(CLEANUP_TIMEOUT, cleanup).await;

    {
        let state = app.state::<AppState>();
        let mut active = state.tunnel.active.lock();
        if active.as_ref().is_some_and(|active| Arc::ptr_eq(&active.core, &core)) {
            *active = None;
        }
    }
    let error = broken.map(|e| format!("{}{e}", tr("隧道异常中断：", "The tunnel stopped unexpectedly: ")));
    let _ = app.emit("tunnel-update", core.status(StateKind::Stopped, error));
}

/// 这个系统上能不能用 TUN 模式。
#[tauri::command]
pub fn tunnel_supported() -> bool {
    cfg!(any(target_os = "macos", windows))
}

/// 助手是不是装进了系统（装了之后开隧道不用再输密码）；没有这回事的系统上是 None。
#[tauri::command]
pub fn tunnel_helper() -> Option<bool> {
    #[cfg(target_os = "macos")]
    return Some(os::helper_installed());
    #[cfg(not(target_os = "macos"))]
    None
}

/// 把助手装进系统（会请用户授权一次），或者从系统里删掉。
#[tauri::command]
pub async fn tunnel_helper_set(installed: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    return if installed { os::install_helper().await } else { os::remove_helper().await };
    #[cfg(not(target_os = "macos"))]
    Err(AppError::msg(format!("{}{installed}{}", tr("当前系统暂不支持（", "Not supported on this system ("), tr("）", ")"))))
}

/// 开启隧道，流量经会话 `session_id` 的 SSH 连接出去。会请用户授予管理员权限；
/// 用户不同意时以 `cancelled` 失败。
#[tauri::command]
pub async fn tunnel_start(app: AppHandle, state: State<'_, AppState>, session_id: String, config: Config) -> Result<Status> {
    let session = state.session(&session_id)?;
    let _starting = state.tunnel.begin()?;

    let server = session.peer.ip();
    // 先把要做的事定下来、问清服务器让不让转发：不成的话，就不必去打扰用户授权了。
    let routes = routes(&config, server)?;
    check_forwarding(&session).await?;
    let link = helper::open(routes).await?;

    let slot = Arc::new(RwLock::new(Some(session)));
    let tunnel = Arc::new(Tunnel::new(ssh_dialer(slot.clone())));
    let core = Arc::new(Core { device: link.device, server, warning: link.warning, session: slot, tunnel });
    let status = core.current();

    let (stop, stopped) = oneshot::channel();
    // 先占住登记的位置再启动守护任务：它结束时要来这里注销，不能让它抢在登记之前。
    let mut active = state.tunnel.active.lock();
    let task = tokio::spawn(supervise(app, core.clone(), link.stream, stopped));
    *active = Some(Active { core, stop, task });
    Ok(status)
}

/// SSH 连接断开后，把隧道接到重连出来的新会话上。
#[tauri::command]
pub fn tunnel_attach(app: AppHandle, state: State<'_, AppState>, session_id: String) -> Result<Status> {
    let session = state.session(&session_id)?;
    let active = state.tunnel.active.lock();
    let Some(active) = active.as_ref() else {
        return Err(AppError::msg(tr("隧道未运行", "The tunnel is not running")));
    };
    let core = &active.core;
    // 只有原来那个地址绕开了隧道；连到别的地址的连接会被自己的隧道吞掉。
    if session.peer.ip() != core.server {
        return Err(AppError::msg(tr("服务器地址已变更，请重新开启隧道", "The server address has changed; turn the tunnel on again")));
    }
    *core.session.write() = Some(session);
    let status = core.current();
    let _ = app.emit("tunnel-update", &status);
    Ok(status)
}

/// 关闭隧道；返回时网卡和路由已经撤掉。没有开启时什么都不做。
#[tauri::command]
pub async fn tunnel_stop(state: State<'_, AppState>) -> Result<()> {
    let active = state.tunnel.active.lock().take();
    if let Some(Active { stop, task, .. }) = active {
        let _ = stop.send(());
        let _ = task.await;
    }
    Ok(())
}

#[tauri::command]
pub fn tunnel_status(state: State<'_, AppState>) -> Option<Status> {
    state.tunnel.active.lock().as_ref().map(|active| active.core.current())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_matches_what_the_frontend_sends() {
        let config: Config = serde_json::from_value(serde_json::json!({
            "route": "include",
            "cidrs": ["10.0.0.0/8"],
            "bypass": ["10.9.0.0/16"],
        }))
        .unwrap();
        assert_eq!(config.route, RouteMode::Include);
        assert_eq!((config.cidrs.as_slice(), config.bypass.as_slice()), (&["10.0.0.0/8".to_string()][..], &["10.9.0.0/16".to_string()][..]));

        let minimal: Config = serde_json::from_value(serde_json::json!({ "route": "all" })).unwrap();
        assert_eq!(minimal.route, RouteMode::All);
        assert!(minimal.cidrs.is_empty() && minimal.bypass.is_empty());
        assert!(serde_json::from_value::<Config>(serde_json::json!({ "route": "some" })).is_err());
        // 旧版本界面存下来的多余的项不妨碍读取。
        assert!(serde_json::from_value::<Config>(serde_json::json!({ "route": "all", "dns": true, "ipv6": false })).is_ok());
    }

    #[test]
    fn networks_are_validated_before_anything_is_changed() {
        let list = |items: &[&str]| items.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        assert_eq!(networks(&list(&["10.1.2.3/8", "192.168.1.7"])).unwrap().iter().map(Cidr::to_string).collect::<Vec<_>>(), ["10.0.0.0/8", "192.168.1.7/32"]);
        assert_eq!(networks(&list(&["10.0.0.0/8", "office"])).unwrap_err().to_string(), "无效的 IPv4 网段：office");
        assert_eq!(networks(&list(&["fd00::/8"])).unwrap_err().to_string(), "无效的 IPv4 网段：fd00::/8");
        assert!(networks(&vec!["10.0.0.0/8".to_string(); MAX_NETWORKS + 1]).is_err());
    }

    #[test]
    fn the_routes_are_settled_before_asking_for_privileges() {
        let config = |route, cidrs: &[&str], bypass: &[&str]| {
            let list = |items: &[&str]| items.iter().map(|s| s.to_string()).collect();
            Config { route, cidrs: list(cidrs), bypass: list(bypass) }
        };
        let server: IpAddr = "203.0.113.9".parse().unwrap();
        let planned = |config: &Config| routes(config, server).map(|nets| nets.iter().map(Cidr::to_string).collect::<Vec<_>>());

        assert_eq!(planned(&config(RouteMode::Include, &["10.0.0.0/8"], &[])).unwrap(), ["10.0.0.0/8"]);
        let global = planned(&config(RouteMode::All, &[], &["10.0.0.0/8"])).unwrap();
        assert!(global.len() > 10 && global.len() <= MAX_ROUTES);
        assert!(!global.iter().any(|net| net == "0.0.0.0/0"));

        // 没填网段、全都绕开了，或者挖得太碎：说清楚，而不是开一条什么都不走的隧道。
        assert!(planned(&config(RouteMode::Include, &[], &[])).is_err());
        assert!(planned(&config(RouteMode::Include, &["10.1.0.0/16"], &["10.0.0.0/8"])).is_err());
        let scattered: Vec<String> = (0..250).map(|i| format!("10.{i}.{i}.77")).collect();
        let scattered: Vec<&str> = scattered.iter().map(String::as_str).collect();
        assert!(planned(&config(RouteMode::All, &[], &scattered)).is_err());
    }

    #[test]
    fn only_an_explicit_refusal_means_forwarding_is_forbidden() {
        use russh::ChannelOpenFailure::{AdministrativelyProhibited, ConnectFailed};
        assert!(forbidden(&russh::Error::ChannelOpenFailure(AdministrativelyProhibited)));
        // 目标连不上、连接断了：说明不了服务器让不让转发。
        assert!(!forbidden(&russh::Error::ChannelOpenFailure(ConnectFailed)));
        assert!(!forbidden(&russh::Error::Disconnect));
    }

    #[test]
    fn only_one_tunnel_can_be_starting() {
        let manager = Manager::default();
        let first = manager.begin().unwrap();
        assert!(manager.begin().is_err());
        drop(first);
        assert!(manager.begin().is_ok());
    }

    #[test]
    fn status_is_reported_in_the_shape_the_frontend_reads() {
        let status = Status {
            state: StateKind::Reconnecting,
            device: "utun7".into(),
            server: "203.0.113.9".into(),
            warning: None,
            error: None,
            connections: 3,
            up: 10,
            down: 20,
        };
        assert_eq!(
            serde_json::to_value(&status).unwrap(),
            serde_json::json!({
                "state": "reconnecting", "device": "utun7", "server": "203.0.113.9",
                "warning": null, "error": null, "connections": 3, "up": 10, "down": 20,
            })
        );
    }
}
