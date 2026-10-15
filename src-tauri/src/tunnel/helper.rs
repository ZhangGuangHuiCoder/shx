//! 提权助手进程（以 `--tun-helper <地址>` 启动）及主进程侧的调用。
//!
//! 助手只做特权操作：建网卡、按清单加路由、在网卡与主进程间搬运 IP 包；SSH 与协议栈都在主进程。
//! 主进程请求收尾或断开时，助手撤网卡并还原路由后退出。协议见 `wire.rs`，平台差异在 `os` 中。

use std::fmt::Display;
use std::io;
use std::net::Ipv4Addr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tun_rs::{AsyncDevice, DeviceBuilder};

use super::route::table::{self, Takeover};
use super::route::Cidr;
use super::wire::{read_json, write_json, Device, Ready, Setup};
use super::{os, MAX_ROUTES, MTU, TUN_ADDR, TUN_PREFIX};
use crate::error::{AppError, Result};
use crate::i18n::tr;

/// 授权后助手连接的超时。
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// 等待助手建网卡和路由的超时；首次可能需安装驱动，留得宽松。
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// 助手等待清单的超时。
const SETUP_TIMEOUT: Duration = Duration::from_secs(30);
/// 路由抽查数量上限。
const MAX_PROBES: usize = 8;
const READ_BUFFER: usize = 256 * 1024;
const MAX_PACKET: usize = 65535;

pub fn not_started() -> AppError {
    AppError::msg(tr("助手进程启动失败", "The helper process failed to start"))
}

pub fn denied(detail: impl Display) -> AppError {
    AppError::msg(format!("{}{detail}", tr("未获得管理员权限：", "Administrator privileges were not granted: ")))
}

// ---- 主进程侧 ----

/// 与助手的链路，传输网卡的 IP 包。
pub struct Link {
    pub device: String,
    /// 非致命问题（如路由被占用）。
    pub warning: Option<String>,
    pub stream: os::Stream,
}

/// 建网卡并加路由，需授权，拒绝时以 Cancelled 失败。
pub async fn open(routes: Vec<Cidr>) -> Result<Link> {
    let mut stream = os::connect_helper().await?;

    let setup = Setup { mtu: MTU, routes };
    write_json(&mut stream, &setup).await?;
    let ready = tokio::time::timeout(READY_TIMEOUT, read_json::<_, Ready>(&mut stream)).await;
    let failed = |detail: &str| AppError::msg(format!("{}{detail}", tr("网络配置失败：", "Network setup failed: ")));
    let device = match ready {
        Ok(Ok(Ready::Device(device))) => device,
        Ok(Ok(Ready::Error(error))) => return Err(failed(&error)),
        Ok(Err(e)) => return Err(failed(&e.to_string())),
        Err(_) => return Err(failed(tr("助手进程无响应", "the helper process did not respond"))),
    };

    // 查路由表属阻塞操作。
    let index = device.index;
    let intruder = tokio::task::spawn_blocking(move || intruder(index, &setup.routes)).await.ok().flatten();
    let intruder = intruder.map(|interface| {
        let what = tr("路由被其他 VPN 或代理占用，部分流量不经隧道（", "Routes are held by another VPN or proxy; some traffic will bypass the tunnel (");
        format!("{what}{interface}{}", tr("）", ")"))
    });
    let warnings: Vec<String> = skipped_warning(&device.skipped).into_iter().chain(intruder).collect();
    let warning = (!warnings.is_empty()).then(|| warnings.join(tr("；", "; ")));
    Ok(Link { device: device.name, warning, stream })
}

/// 未能添加路由的网段提示，只列出前几个。
fn skipped_warning(skipped: &[Cidr]) -> Option<String> {
    const SHOWN: usize = 5;
    if skipped.is_empty() {
        return None;
    }
    let shown: Vec<String> = skipped.iter().take(SHOWN).map(Cidr::to_string).collect();
    let what = tr(" 个网段的路由未能添加，流量不经隧道：", " network routes could not be added and bypass the tunnel: ");
    let more = if skipped.len() > SHOWN { "…" } else { "" };
    Some(format!("{}{what}{}{more}", skipped.len(), shown.join(tr("、", ", "))))
}

fn probe(net: &Cidr) -> Option<Ipv4Addr> {
    // 单地址网段查自身，否则查第二个地址。
    let probe = Ipv4Addr::from(u32::from(net.addr) | u32::from(net.prefix < 32));
    // 0.0.0.0/8 不可路由。
    (probe.octets()[0] != 0).then_some(probe)
}

/// 在网段中均匀抽取地址。
fn probes(routes: &[Cidr]) -> Vec<Ipv4Addr> {
    let spacing = routes.len().div_ceil(MAX_PROBES).max(1);
    routes.iter().step_by(spacing).filter_map(probe).collect()
}

/// 验证路由是否生效；流量去了其他网卡（如别的 VPN 占用）时返回该网卡名。
fn intruder(device: u32, routes: &[Cidr]) -> Option<String> {
    let mut carriers = probes(routes).into_iter().filter_map(table::carrier);
    carriers.find(|(index, _)| *index != device).map(|(_, name)| name)
}

// ---- 助手侧（以管理员身份运行） ----

/// 以助手身份运行时返回退出码，否则返回 None。
pub fn dispatch() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    let command = args.next().unwrap_or_default();
    let Some(address) = args.next().filter(|_| command == "--tun-helper") else {
        return os::dispatch(&command);
    };
    let served = tokio::runtime::Builder::new_current_thread().enable_all().build().and_then(|runtime| runtime.block_on(serve(&address)));
    Some(i32::from(served.is_err()))
}

async fn serve(address: &str) -> io::Result<()> {
    let mut stream = os::connect(address).await?;
    let setup = tokio::time::timeout(SETUP_TIMEOUT, read_json::<_, Setup>(&mut stream)).await??;

    let (takeover, device, described) = match check(&setup).and_then(|()| bring_up(&setup)) {
        Ok(up) => up,
        Err(error) => return write_json(&mut stream, &Ready::Error(error)).await,
    };
    let result = match write_json(&mut stream, &Ready::Device(described)).await {
        Ok(()) => pump(&device, &mut stream).await,
        Err(e) => Err(e),
    };
    // 先撤网卡（路由随之消失），再还原接管的路由。
    drop(device);
    drop(takeover);
    // 最后关闭连接，表示一切已还原。
    drop(stream);
    result
}

/// 清单来自非特权进程，逐项校验后才用于特权操作。
fn check(setup: &Setup) -> std::result::Result<(), String> {
    if !(576..=MTU).contains(&setup.mtu) {
        return Err(format!("invalid MTU {}", setup.mtu));
    }
    if setup.routes.len() > MAX_ROUTES {
        return Err("too many routes".into());
    }
    // 拒绝 0.0.0.0/0，避免与默认路由冲突后无法恢复。
    if let Some(all) = setup.routes.iter().find(|net| net.prefix == 0) {
        return Err(format!("refusing to replace the default route ({all})"));
    }
    Ok(())
}

/// 建网卡并加路由；返回的接管记录须在撤网卡后还原。
fn bring_up(setup: &Setup) -> std::result::Result<(Takeover, AsyncDevice, Device), String> {
    let builder = os::configure(DeviceBuilder::new().ipv4(TUN_ADDR, TUN_PREFIX, None).mtu(setup.mtu))?;
    let device = builder.build_async().map_err(|e| format!("create the network interface: {e}"))?;
    let name = device.name().map_err(|e| format!("name the network interface: {e}"))?;
    let index = device.if_index().map_err(|e| format!("find the network interface: {e}"))?;

    // 与本地网络重叠的网段需接管直连路由；本地网络由助手自行查询，不信任主进程。
    let local = table::local();
    let mut takeover = Takeover::prepare(&local, &setup.routes);
    // 个别路由失败不致命，记录后由主进程提示；全部失败才报错。
    let mut skipped = Vec::new();
    let mut last = None;
    for net in &setup.routes {
        // 失败且被网关专用路由占位时，替换后重试。
        let added = table::add(net, index).or_else(|error| if takeover.make_room(net) { table::add(net, index) } else { Err(error) });
        if let Err(error) = added {
            skipped.push(*net);
            last = Some(error);
        }
    }
    if let Some(error) = last.filter(|_| skipped.len() == setup.routes.len()) {
        drop(device);
        return Err(format!("add routes: {error}"));
    }
    Ok((takeover, device, Device { name, index, skipped }))
}

/// 搬运 IP 包，直到收尾请求、断开或收到终止信号。
async fn pump<S>(device: &AsyncDevice, stream: &mut S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let outgoing = async {
        // 预留两字节长度前缀，一次写出。
        let mut frame = vec![0u8; 2 + MAX_PACKET];
        loop {
            let n = device.recv(&mut frame[2..]).await?;
            // 空消息表示收尾，不能发送。
            let Some(len) = u16::try_from(n).ok().filter(|len| *len > 0) else { continue };
            frame[..2].copy_from_slice(&len.to_be_bytes());
            writer.write_all(&frame[..2 + n]).await?;
        }
    };
    let incoming = async {
        let mut reader = BufReader::with_capacity(READ_BUFFER, reader);
        let mut packet = vec![0u8; MAX_PACKET];
        loop {
            let n = usize::from(reader.read_u16().await?);
            // 空消息：收尾请求。
            if n == 0 {
                return Ok(());
            }
            reader.read_exact(&mut packet[..n]).await?;
            // 网卡写满时丢包，由上层重传。
            let _ = device.send(&packet[..n]).await;
        }
    };
    tokio::select! {
        result = outgoing => result,
        result = incoming => result,
        _ = os::terminated() => Ok(()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(list: &[&str]) -> Vec<Cidr> {
        list.iter().map(|net| net.parse().unwrap()).collect()
    }

    #[test]
    fn networks_left_without_a_route_are_named() {
        assert_eq!(skipped_warning(&[]), None);
        assert_eq!(skipped_warning(&nets(&["10.0.0.0/8", "172.16.0.0/12"])).unwrap(), "2 个网段的路由未能添加，流量不经隧道：10.0.0.0/8、172.16.0.0/12");
        // 过多时只列前几个。
        let many: Vec<Cidr> = (0..40).map(|i| format!("10.{i}.0.0/16").parse().unwrap()).collect();
        let warning = skipped_warning(&many).unwrap();
        assert!(warning.starts_with("40 ") && warning.ends_with("10.4.0.0/16…"), "{warning}");
    }

    #[test]
    fn probes_stay_inside_their_network() {
        let probe = |net: &str| probe(&net.parse().unwrap()).map(|ip| ip.to_string());
        assert_eq!(probe("128.0.0.0/1").as_deref(), Some("128.0.0.1"));
        assert_eq!(probe("10.0.0.0/8").as_deref(), Some("10.0.0.1"));
        assert_eq!(probe("192.168.1.6/31").as_deref(), Some("192.168.1.7"));
        assert_eq!(probe("192.168.1.7/32").as_deref(), Some("192.168.1.7"));
        assert_eq!(probe("0.0.0.0/5"), None);
    }

    #[test]
    fn a_few_routes_are_probed_however_many_there_are() {
        let routes = |count: usize| (0..count).map(|i| format!("10.{}.{}.0/24", i / 256, i % 256).parse().unwrap()).collect::<Vec<Cidr>>();
        assert_eq!(probes(&routes(3)), ["10.0.0.1", "10.0.1.1", "10.0.2.1"].map(|ip| ip.parse::<Ipv4Addr>().unwrap()));
        for count in [8, 9, 100, 1000] {
            let probed = probes(&routes(count));
            assert!((MAX_PROBES / 2..=MAX_PROBES).contains(&probed.len()), "{count} routes, {} probes", probed.len());
            // 抽样覆盖到后段网段。
            assert!(routes(count)[count / 2..].iter().any(|net| net.contains(probed[probed.len() - 1])), "{count} routes");
        }
        assert_eq!(probes(&[]), Vec::<Ipv4Addr>::new());
    }

    /// 非法清单项一律拒绝。
    #[test]
    fn the_list_is_checked_before_anything_is_done() {
        let setup = || Setup { mtu: 9000, routes: nets(&["0.0.0.0/1", "128.0.0.0/1"]) };
        assert_eq!(check(&setup()), Ok(()));

        let refused = |change: fn(&mut Setup)| {
            let mut setup = setup();
            change(&mut setup);
            check(&setup).is_err()
        };
        assert!(refused(|s| s.mtu = 100));
        assert!(refused(|s| s.mtu = 65000));
        assert!(refused(|s| s.routes = nets(&["10.0.0.0/8"; MAX_ROUTES + 1])));
        // 不替换默认路由。
        assert!(refused(|s| s.routes.extend(nets(&["0.0.0.0/0"]))));
        // 非法网段无法解析。
        assert!(serde_json::from_str::<Setup>(r#"{"mtu":9000,"routes":["10.0.0.0/8"]}"#).is_ok());
        assert!(serde_json::from_str::<Setup>(r#"{"mtu":9000,"routes":["10.0.0.0/8 metric=1"]}"#).is_err());
    }

    #[test]
    fn only_helper_commands_are_taken_over() {
        // 测试进程的命令行不应触发助手。
        assert_eq!(dispatch(), None);
    }
}
