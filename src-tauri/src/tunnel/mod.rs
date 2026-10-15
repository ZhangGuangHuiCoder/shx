//! TUN 模式：类似 VPN，本机流量经 SSH 服务器转发。
//!
//! 虚拟网卡 + 路由把流量引入，用户态协议栈还原 TCP 连接，每条连接开一个 direct-tcpip 通道。
//! 仅支持 IPv4 TCP（及 DNS）；服务器端无需 root 或安装任何东西。
//! 只改动网卡和路由（及 macOS 接管本地网络时的两处路由，见 `route::table::Takeover`），关闭时全部还原。
//!
//! - 本文件：开关、状态与前端命令。
//! - `route.rs`：分流规则与路由表。
//! - `stack.rs`：用户态协议栈，TCP 和 DNS 转发。
//! - `helper.rs`：提权助手进程，负责建网卡和加路由；`wire.rs` 为双方协议。
//! - `macos.rs`、`windows.rs`：平台相关的提权与通信。

// 未实现的平台上助手相关代码闲置。
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

use self::route::{Cidr, Local, Rule, Rules, Via};
use self::stack::{Conn, Dialer, Tunnel};
use self::wire::FrameWriter;
use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::ssh::{AppState, Session};

/// 连接在本机终结，MTU 可以开得很大以减少开销。
pub const MTU: u16 = 9000;
/// 网卡地址，取自测试保留网段。
pub const TUN_ADDR: Ipv4Addr = Ipv4Addr::new(198, 19, 255, 253);
pub const TUN_PREFIX: u8 = 30;
/// 路由条数上限。
pub const MAX_ROUTES: usize = 1024;

/// 规则条数上限。
const MAX_RULES: usize = 256;
/// 关闭时等待助手撤除网卡的超时。
const CLEANUP_TIMEOUT: Duration = Duration::from_secs(15);
/// 开启前探测通道的超时。
const PROBE_TIMEOUT: Duration = Duration::from_secs(10);
/// 探测远端 DNS 的超时。
const RESOLVER_TIMEOUT: Duration = Duration::from_secs(3);

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

#[derive(Deserialize, Debug)]
struct RuleText {
    net: String,
    via: Via,
}

#[derive(Deserialize, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    /// 兜底走向。
    default: Via,
    #[serde(default)]
    rules: Vec<RuleText>,
    /// 本机网段的走向（网段自动探测）；None 表示不特殊处理。
    #[serde(default)]
    lan: Option<Via>,
}

impl Config {
    /// 校验规则；无隧道路由等情况由 [`routes`] 报告。
    fn explicit(&self) -> Result<Rules> {
        if self.rules.len() > MAX_RULES {
            return Err(AppError::msg(tr("规则数量超出上限", "Too many rules")));
        }
        // 只支持 IPv4，IPv6 网段视为非法。
        let parse = |rule: &RuleText| match rule.net.parse() {
            Ok(net) => Ok(Rule { net, via: rule.via }),
            Err(()) => Err(AppError::msg(format!("{}{}", tr("无效的 IPv4 网段：", "Invalid IPv4 network: "), rule.net))),
        };
        let list = self.rules.iter().map(parse).collect::<Result<Vec<Rule>>>()?;
        Ok(Rules { default: self.default, list })
    }

    /// 用户规则加上本地网络规则；local 为本机直连网络。
    fn rules(&self, explicit: &Rules, local: &Local) -> Rules {
        // 虚拟机/容器内部网络总是直连；本机网络按“本地网络”规则走。同网段以用户规则为准。
        let via = |lan: &route::Lan| if lan.uplink { self.lan } else { Some(Via::Direct) };
        let named = |net: &Cidr| explicit.list.iter().any(|rule| rule.net == *net);
        let implied = local.lans.iter().filter(|lan| !named(&lan.net)).filter_map(|lan| Some(Rule { net: lan.net, via: via(lan)? }));
        Rules { default: self.default, list: implied.chain(explicit.list.iter().copied()).collect() }
    }
}

/// 进隧道的网段，各加一条路由；never 为永不进隧道的地址。
fn routes(rules: &Rules, never: &[Cidr], local: &Local) -> Result<Vec<Cidr>> {
    let routes = route::outrank(route::plan(rules, never), &local.shadows());
    // 全直连或过于零碎时报错，不开空隧道。
    if routes.is_empty() {
        return Err(AppError::msg(tr("没有流量会走隧道，请检查规则", "No traffic would use the tunnel; check the rules")));
    }
    if routes.len() > MAX_ROUTES {
        return Err(AppError::msg(tr("规则把网段切得太碎，请减少或合并", "The rules split the networks into too many routes; remove or merge some")));
    }
    Ok(routes)
}

async fn local() -> Local {
    #[cfg(any(target_os = "macos", windows))]
    return tokio::task::spawn_blocking(route::table::local).await.unwrap_or_default();
    #[cfg(not(any(target_os = "macos", windows)))]
    Local::default()
}

fn resolvers() -> Vec<Ipv4Addr> {
    #[cfg(any(target_os = "macos", windows))]
    return os::resolvers();
    #[cfg(not(any(target_os = "macos", windows)))]
    Vec::new()
}

/// 被隧道顺带接管（非用户明确指定）的本地 DNS 服务器。
fn swept_resolvers(resolvers: &[Ipv4Addr], explicit: &Rules, routes: &[Cidr], local: &Local) -> Vec<Ipv4Addr> {
    let swept = |ip: &Ipv4Addr| {
        let named = explicit.ruling(&Cidr::host(*ip)).is_some_and(|rule| rule.via == Via::Tunnel);
        local.on_lan(*ip) && routes.iter().any(|net| net.contains(*ip)) && !named
    };
    let mut swept: Vec<Ipv4Addr> = resolvers.iter().copied().filter(swept).collect();
    swept.sort();
    swept.dedup();
    swept
}

/// 本地 DNS 常为路由器，进隧道后由远端同地址应答；远端答不上的返回出来留在本地，避免无法解析。
async fn stranded_resolvers(session: &Session, swept: Vec<Ipv4Addr>) -> Vec<Ipv4Addr> {
    let answers = |ip: Ipv4Addr| async move { matches!(probe(session, &ip.to_string(), stack::DNS_PORT, RESOLVER_TIMEOUT).await, Some(Ok(()))) };
    let answered = futures::future::join_all(swept.iter().map(|ip| answers(*ip))).await;
    swept.into_iter().zip(answered).filter(|(_, answered)| !answered).map(|(ip, _)| ip).collect()
}

fn stranded_warning(stranded: &[Ipv4Addr]) -> Option<String> {
    if stranded.is_empty() {
        return None;
    }
    let list: Vec<String> = stranded.iter().map(Ipv4Addr::to_string).collect();
    let what = tr("本机的 DNS 服务器在远端无应答，保持直连：", "This computer's DNS server does not answer at the far end and stays direct: ");
    Some(format!("{what}{}", list.join(tr("、", ", "))))
}

#[derive(Clone, Copy, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
enum StateKind {
    Running,
    /// SSH 已断开，等待前端重连；流量仍被导入隧道但无法送出。
    Reconnecting,
    Stopped,
}

/// 通过 `tunnel-update` 事件发给前端。
#[derive(Clone, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Status {
    state: StateKind,
    device: String,
    /// 只有该地址绕开隧道，重连须连同一地址。
    server: String,
    warning: Option<String>,
    error: Option<String>,
    connections: u64,
    up: u64,
    down: u64,
}

struct Core {
    device: String,
    server: IpAddr,
    warning: Option<String>,
    /// 断开到重连期间为 None。
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

/// 同一时刻至多一条隧道。
#[derive(Default)]
pub struct Manager {
    active: Mutex<Option<Active>>,
    /// 启动中，禁止再次启动。
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

/// 每条连接用当前 SSH 连接开一个 direct-tcpip 通道。
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

/// 服务器明确拒绝转发（如 AllowTcpForwarding no），连接失败或超时不算。
fn forbidden(error: &russh::Error) -> bool {
    matches!(error, russh::Error::ChannelOpenFailure(russh::ChannelOpenFailure::AdministrativelyProhibited))
}

/// 在请求授权前探测服务器是否允许转发；只有明确拒绝才报错。
fn check_forwarding(reply: &Option<std::result::Result<(), russh::Error>>) -> Result<()> {
    match reply {
        Some(Err(error)) if forbidden(error) => Err(AppError::msg(tr(
            "服务器禁止 TCP 转发（AllowTcpForwarding），隧道无法使用",
            "The server does not allow TCP forwarding (AllowTcpForwarding); the tunnel cannot work",
        ))),
        _ => Ok(()),
    }
}

/// 试开通道后立即关闭；超时返回 None。
async fn probe(session: &Session, host: &str, port: u16, timeout: Duration) -> Option<std::result::Result<(), russh::Error>> {
    let opening = session.handle.channel_open_direct_tcpip(host, u32::from(port), "127.0.0.1", 0);
    let opened = tokio::time::timeout(timeout, opening).await.ok()?;
    Some(match opened {
        Ok(channel) => {
            let _ = channel.close().await;
            Ok(())
        }
        Err(error) => Err(error),
    })
}

async fn probe_server(session: &Session) -> Option<std::result::Result<(), russh::Error>> {
    probe(session, "127.0.0.1", session.peer.port(), PROBE_TIMEOUT).await
}

/// 能开通道或被拒绝都说明连接正常。
fn answered(reply: &Option<std::result::Result<(), russh::Error>>) -> bool {
    matches!(reply, Some(Ok(()) | Err(russh::Error::ChannelOpenFailure(_))))
}

/// 请助手收尾并等待完成（撤网卡、还原路由后才断开连接）。
async fn shut<R, W>(reader: &mut R, writer: &mut FrameWriter<W>)
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let cleanup = async {
        let _ = writer.close().await;
        let _ = tokio::io::copy(reader, &mut tokio::io::sink()).await;
    };
    let _ = tokio::time::timeout(CLEANUP_TIMEOUT, cleanup).await;
}

/// 每秒检查连接、状态和流量，永不返回。
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

/// 守护隧道直到被关闭或与助手断开。
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
    shut(&mut reader, &mut writer).await;

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

#[tauri::command]
pub fn tunnel_supported() -> bool {
    cfg!(any(target_os = "macos", windows))
}

#[derive(Clone, Copy, Serialize, PartialEq, Debug)]
#[serde(rename_all = "camelCase")]
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub enum HelperState {
    /// 未安装，每次开启都需授权。
    Missing,
    /// 版本过旧，下次开启时需更新并授权一次。
    Outdated,
    /// 已安装当前版本，开启无需授权。
    Installed,
}

/// 不需要助手的系统返回 None。
#[tauri::command]
pub async fn tunnel_helper() -> Option<HelperState> {
    // 比对文件属阻塞操作，放到阻塞线程。
    #[cfg(target_os = "macos")]
    return tokio::task::spawn_blocking(os::helper_state).await.ok();
    #[cfg(not(target_os = "macos"))]
    None
}

/// 安装（需授权）或卸载助手。
#[tauri::command]
pub async fn tunnel_helper_set(installed: bool) -> Result<()> {
    #[cfg(target_os = "macos")]
    return if installed { os::install_helper().await } else { os::remove_helper().await };
    #[cfg(not(target_os = "macos"))]
    Err(AppError::msg(format!("{}{installed}{}", tr("当前系统暂不支持（", "Not supported on this system ("), tr("）", ")"))))
}

/// 本机网段，供界面说明。
#[tauri::command]
pub async fn tunnel_local() -> Vec<String> {
    let mut nets: Vec<String> = local().await.lans.iter().filter(|lan| lan.uplink).map(|lan| lan.net.to_string()).collect();
    nets.dedup();
    nets
}

/// 开启隧道，需管理员权限，拒绝时以 cancelled 失败。
#[tauri::command]
pub async fn tunnel_start(app: AppHandle, state: State<'_, AppState>, session_id: String, config: Config) -> Result<Status> {
    let session = state.session(&session_id)?;
    let _starting = state.tunnel.begin()?;

    let server = session.peer.ip();
    // 先规划路由并探测转发，失败则不必请求授权。
    let local = local().await;
    let explicit = config.explicit()?;
    let rules = config.rules(&explicit, &local);
    let mut never = route::never(server, &local);
    let mut planned = routes(&rules, &never, &local)?;
    // 两项探测互不相关，并发进行。
    let swept = swept_resolvers(&resolvers(), &explicit, &planned, &local);
    let (before, stranded) = tokio::join!(probe_server(&session), stranded_resolvers(&session, swept));
    check_forwarding(&before)?;
    if !stranded.is_empty() {
        never.extend(stranded.iter().copied().map(Cidr::host));
        planned = routes(&rules, &never, &local)?;
    }
    let mut link = helper::open(planned).await?;

    // 改路由后承载连接可能断开（接管本地路由时尤甚），此时撤销并报错；改路由前就不应答的不验证。
    if answered(&before) && !answered(&probe_server(&session).await) {
        let (mut reader, writer) = tokio::io::split(&mut link.stream);
        shut(&mut reader, &mut FrameWriter::new(writer)).await;
        return Err(AppError::msg(tr(
            "开启后与服务器的连接中断，隧道已撤销。可以把“本地网络”设为直连再试",
            "The connection to the server broke once the tunnel was up, so it was taken down. Try again with “Local Network” set to direct",
        )));
    }
    let warnings: Vec<String> = link.warning.take().into_iter().chain(stranded_warning(&stranded)).collect();
    let warning = (!warnings.is_empty()).then(|| warnings.join(tr("；", "; ")));

    let slot = Arc::new(RwLock::new(Some(session)));
    let tunnel = Arc::new(Tunnel::new(ssh_dialer(slot.clone())));
    let core = Arc::new(Core { device: link.device, server, warning, session: slot, tunnel });
    let status = core.current();

    let (stop, stopped) = oneshot::channel();
    // 先登记再启动守护任务，避免其结束时先于登记注销。
    let mut active = state.tunnel.active.lock();
    let task = tokio::spawn(supervise(app, core.clone(), link.stream, stopped));
    *active = Some(Active { core, stop, task });
    Ok(status)
}

/// 把隧道接到重连后的新会话。
#[tauri::command]
pub fn tunnel_attach(app: AppHandle, state: State<'_, AppState>, session_id: String) -> Result<Status> {
    let session = state.session(&session_id)?;
    let active = state.tunnel.active.lock();
    let Some(active) = active.as_ref() else {
        return Err(AppError::msg(tr("隧道未运行", "The tunnel is not running")));
    };
    let core = &active.core;
    // 只有原地址绕开隧道，否则连接会被隧道吞掉。
    if session.peer.ip() != core.server {
        return Err(AppError::msg(tr("服务器地址已变更，请重新开启隧道", "The server address has changed; turn the tunnel on again")));
    }
    *core.session.write() = Some(session);
    let status = core.current();
    let _ = app.emit("tunnel-update", &status);
    Ok(status)
}

/// 返回时网卡和路由已撤除；未开启时无操作。
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

    fn config(value: serde_json::Value) -> Config {
        serde_json::from_value(value).unwrap()
    }

    /// 与开启时相同的规则构建流程。
    fn all(config: &Config, local: &Local) -> Result<Rules> {
        Ok(config.rules(&config.explicit()?, local))
    }

    fn home() -> Local {
        let ip = |text: &str| text.parse::<Ipv4Addr>().unwrap();
        let lan = route::Lan { uplink: true, ..route::Lan::new(14, ip("192.168.1.30"), ip("255.255.255.0")).unwrap() };
        // 虚拟机内部网络。
        let vms = route::Lan::new(25, ip("192.168.139.3"), ip("255.255.254.0")).unwrap();
        Local { lans: vec![lan, vms], pinned: vec![(ip("192.168.1.1"), 14)] }
    }

    #[test]
    fn config_matches_what_the_frontend_sends() {
        let split = config(serde_json::json!({
            "default": "direct",
            "rules": [{ "net": "10.1.2.3/8", "via": "tunnel" }, { "net": "10.9.0.0/16", "via": "direct" }],
            "lan": "direct",
        }));
        assert!(split.lan == Some(Via::Direct) && split.default == Via::Direct);
        let rules = all(&split, &Local::default()).unwrap();
        assert_eq!(rules.list.iter().map(|rule| (rule.net.to_string(), rule.via)).collect::<Vec<_>>(), [("10.0.0.0/8".to_string(), Via::Tunnel), ("10.9.0.0/16".to_string(), Via::Direct)]);

        let minimal = config(serde_json::json!({ "default": "tunnel" }));
        assert!(minimal.rules.is_empty() && minimal.lan.is_none() && minimal.default == Via::Tunnel);
        assert!(serde_json::from_value::<Config>(serde_json::json!({ "default": "some" })).is_err());
        assert!(serde_json::from_value::<Config>(serde_json::json!({ "default": "tunnel", "rules": [{ "net": "10.0.0.0/8", "via": "maybe" }] })).is_err());
        // 忽略旧版本的多余字段。
        assert!(serde_json::from_value::<Config>(serde_json::json!({ "default": "tunnel", "dns": true, "ipv6": false })).is_ok());
    }

    #[test]
    fn rules_are_validated_before_anything_is_changed() {
        let with = |nets: &[&str]| config(serde_json::json!({ "default": "direct", "rules": nets.iter().map(|net| serde_json::json!({ "net": net, "via": "tunnel" })).collect::<Vec<_>>() }));
        let none = Local::default();
        assert_eq!(with(&["10.0.0.0/8", "office"]).explicit().unwrap_err().to_string(), "无效的 IPv4 网段：office");
        assert_eq!(with(&["fd00::/8"]).explicit().unwrap_err().to_string(), "无效的 IPv4 网段：fd00::/8");
        assert!(with(&vec!["10.0.0.0/8"; MAX_RULES + 1]).explicit().is_err());
        // 兜底直连且无隧道规则时无法开启。
        let nothing = |config: Config| routes(&all(&config, &none).unwrap(), &route::never("203.0.113.9".parse().unwrap(), &none), &none).is_err();
        assert!(nothing(with(&[])));
        assert!(nothing(config(serde_json::json!({ "default": "direct", "rules": [{ "net": "10.0.0.0/8", "via": "direct" }] }))));
    }

    #[test]
    fn the_routes_are_settled_before_asking_for_privileges() {
        let server: IpAddr = "203.0.113.9".parse().unwrap();
        let planned = |value: serde_json::Value, local: &Local| {
            let rules = all(&config(value), local)?;
            routes(&rules, &route::never(server, local), local).map(|nets| nets.iter().map(Cidr::to_string).collect::<Vec<_>>())
        };
        let none = Local::default();

        assert_eq!(planned(serde_json::json!({ "default": "direct", "rules": [{ "net": "10.0.0.0/8", "via": "tunnel" }] }), &none).unwrap(), ["10.0.0.0/8"]);
        let global = planned(serde_json::json!({ "default": "tunnel", "rules": [{ "net": "10.0.0.0/8", "via": "direct" }] }), &none).unwrap();
        assert!(global.len() > 10 && global.len() <= MAX_ROUTES);
        assert!(!global.iter().any(|net| net == "0.0.0.0/0"));

        // 全直连或过于零碎时报错。
        let covered = serde_json::json!({ "default": "direct", "rules": [{ "net": "203.0.113.9", "via": "tunnel" }] });
        assert!(planned(covered, &none).is_err());
        let scattered: Vec<_> = (0..250).map(|i| serde_json::json!({ "net": format!("10.{i}.{i}.77"), "via": "direct" })).collect();
        assert!(planned(serde_json::json!({ "default": "tunnel", "rules": scattered }), &none).is_err());
    }

    #[test]
    fn the_local_network_is_taken_over_unless_it_is_kept_direct() {
        let server: IpAddr = "203.0.113.9".parse().unwrap();
        let home = home();
        let tunnelled = |value: serde_json::Value, ip: &str| {
            let rules = all(&config(value), &home).unwrap();
            let routes = routes(&rules, &route::never(server, &home), &home).unwrap();
            routes.iter().any(|net| net.contains(ip.parse().unwrap()))
        };
        // 兜底走隧道：网关地址也走隧道，本机地址不走。
        let all = || serde_json::json!({ "default": "tunnel" });
        assert!(tunnelled(all(), "192.168.1.1") && tunnelled(all(), "192.168.1.77") && !tunnelled(all(), "192.168.1.30"));
        // 本地网络直连。
        let kept = || serde_json::json!({ "default": "tunnel", "lan": "direct" });
        assert!(!tunnelled(kept(), "192.168.1.1") && !tunnelled(kept(), "192.168.1.77") && tunnelled(kept(), "192.168.2.1"));
        // 更具体的规则优先。
        let one = || serde_json::json!({ "default": "tunnel", "lan": "direct", "rules": [{ "net": "192.168.1.1", "via": "tunnel" }] });
        assert!(tunnelled(one(), "192.168.1.1") && !tunnelled(one(), "192.168.1.77"));
        // 同网段以用户规则为准。
        let named = || serde_json::json!({ "default": "direct", "lan": "direct", "rules": [{ "net": "192.168.1.0/24", "via": "tunnel" }] });
        assert!(tunnelled(named(), "192.168.1.1") && tunnelled(named(), "192.168.1.77"));
        // 本地网络走隧道：兜底直连时仅它进隧道，更大的直连网段不覆盖它。
        let taken = || serde_json::json!({ "default": "direct", "lan": "tunnel", "rules": [{ "net": "192.168.0.0/16", "via": "direct" }] });
        assert!(tunnelled(taken(), "192.168.1.1") && !tunnelled(taken(), "192.168.2.1") && !tunnelled(taken(), "8.8.8.8"));

        // 虚拟机/容器内部网络默认留在本地。
        assert!(!tunnelled(all(), "192.168.139.96") && tunnelled(all(), "192.168.140.1"));
        let wide = || serde_json::json!({ "default": "direct", "rules": [{ "net": "192.168.0.0/16", "via": "tunnel" }] });
        assert!(!tunnelled(wide(), "192.168.139.96") && tunnelled(wide(), "192.168.1.1"));
        let vms = || serde_json::json!({ "default": "direct", "rules": [{ "net": "192.168.138.0/23", "via": "tunnel" }] });
        assert!(tunnelled(vms(), "192.168.139.96") && !tunnelled(vms(), "192.168.139.3"));
    }

    #[test]
    fn a_local_dns_server_is_only_swept_into_the_tunnel_when_nobody_asked_for_it() {
        let server: IpAddr = "203.0.113.9".parse().unwrap();
        let home = home();
        let ip = |text: &str| text.parse::<Ipv4Addr>().unwrap();
        let resolvers = [ip("192.168.1.1"), ip("192.168.1.1"), ip("223.5.5.5")];
        let swept = |value: serde_json::Value| {
            let config = config(value);
            let routes = routes(&all(&config, &home).unwrap(), &route::never(server, &home), &home).unwrap();
            swept_resolvers(&resolvers, &config.explicit().unwrap(), &routes, &home)
        };
        // 兜底走隧道时本地 DNS 被顺带接管，需探测；公网 DNS 不需要。
        assert_eq!(swept(serde_json::json!({ "default": "tunnel" })), [ip("192.168.1.1")]);
        // 用户明确指定或本就留在本地时无需探测。
        assert!(swept(serde_json::json!({ "default": "tunnel", "rules": [{ "net": "192.168.1.0/24", "via": "tunnel" }] })).is_empty());
        assert!(swept(serde_json::json!({ "default": "tunnel", "lan": "direct" })).is_empty());
        // 本地网络走隧道时同样需探测。
        assert_eq!(swept(serde_json::json!({ "default": "direct", "lan": "tunnel" })), [ip("192.168.1.1")]);
        assert!(swept(serde_json::json!({ "default": "direct", "rules": [{ "net": "10.0.0.0/8", "via": "tunnel" }] })).is_empty());

        assert_eq!(stranded_warning(&[]), None);
        assert_eq!(stranded_warning(&[ip("192.168.1.1")]).unwrap(), "本机的 DNS 服务器在远端无应答，保持直连：192.168.1.1");
    }

    #[test]
    fn the_server_answering_at_all_means_the_connection_is_alive() {
        use russh::ChannelOpenFailure::{AdministrativelyProhibited, ConnectFailed};
        assert!(answered(&Some(Ok(()))));
        assert!(answered(&Some(Err(russh::Error::ChannelOpenFailure(ConnectFailed)))));
        assert!(answered(&Some(Err(russh::Error::ChannelOpenFailure(AdministrativelyProhibited)))));
        // 超时或连接已断。
        assert!(!answered(&None));
        assert!(!answered(&Some(Err(russh::Error::Disconnect))));

        // 只有明确拒绝才阻止开启。
        assert!(check_forwarding(&Some(Err(russh::Error::ChannelOpenFailure(AdministrativelyProhibited)))).is_err());
        assert!(check_forwarding(&Some(Err(russh::Error::ChannelOpenFailure(ConnectFailed)))).is_ok());
        assert!(check_forwarding(&Some(Ok(()))).is_ok() && check_forwarding(&None).is_ok());
    }

    #[test]
    fn only_an_explicit_refusal_means_forwarding_is_forbidden() {
        use russh::ChannelOpenFailure::{AdministrativelyProhibited, ConnectFailed};
        assert!(forbidden(&russh::Error::ChannelOpenFailure(AdministrativelyProhibited)));
        // 目标不可达不能说明服务器是否允许转发。
        assert!(!forbidden(&russh::Error::ChannelOpenFailure(ConnectFailed)));
        assert!(!forbidden(&russh::Error::Disconnect));
    }

    #[test]
    fn the_helper_state_is_reported_in_the_words_the_frontend_reads() {
        let words = [HelperState::Missing, HelperState::Outdated, HelperState::Installed].map(|state| serde_json::to_value(state).unwrap());
        assert_eq!(words, ["missing", "outdated", "installed"]);
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
