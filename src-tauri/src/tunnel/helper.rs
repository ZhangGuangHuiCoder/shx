//! 提权的助手进程，以及主进程怎么用它。
//!
//! 本程序以 `--tun-helper <地址>` 启动、以管理员身份运行时就是助手。它只做需要特权的三件事：
//! 创建虚拟网卡，照主进程定好的清单加路由，在网卡和主进程之间搬运 IP 包。SSH 连接、凭据和
//! 协议栈都在不提权的主进程里。
//!
//! 主进程请它收尾或者断开了（退出、崩溃），它就撤掉网卡退出；路由都指向这块网卡，随它
//! 一起消失，所以没有别的要还原。它没有窗口也没有日志，出错只能经那条连接告诉主进程。
//!
//! 两边的对话见 `wire.rs`。怎么提权、经什么连接对话，各系统不同，在 `os` 里；这里是共用的流程。

use std::fmt::Display;
use std::io;
use std::net::Ipv4Addr;
use std::time::Duration;

use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tun_rs::{AsyncDevice, DeviceBuilder};

use super::route::{table, Cidr};
use super::wire::{read_json, write_json, Device, Ready, Setup};
use super::{os, MAX_ROUTES, MTU, TUN_ADDR, TUN_PREFIX};
use crate::error::{AppError, Result};
use crate::i18n::tr;

/// 授权之后，助手应当在这段时间内连上来。
pub const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// 主进程等助手建网卡、加路由的时间上限。头一次用还要装网卡的驱动，留得宽一些。
const READY_TIMEOUT: Duration = Duration::from_secs(60);
/// 助手等主进程交代清单的时间上限。
const SETUP_TIMEOUT: Duration = Duration::from_secs(30);
/// 检查路由是否生效时，最多抽查这么多条。
const MAX_PROBES: usize = 8;
const READ_BUFFER: usize = 256 * 1024;
const MAX_PACKET: usize = 65535;

pub fn not_started() -> AppError {
    AppError::msg(tr("助手进程启动失败", "The helper process failed to start"))
}

pub fn denied(detail: impl Display) -> AppError {
    AppError::msg(format!("{}{detail}", tr("未获得管理员权限：", "Administrator privileges were not granted: ")))
}

// ---- 主进程这一头：启动助手，把要做的事交代给它 -------------------------------

/// 和助手之间接好的链路：上面来回跑的是虚拟网卡收发的 IP 包。
pub struct Link {
    /// 虚拟网卡的名字。
    pub device: String,
    /// 路由被别的程序占着之类不致命的问题。
    pub warning: Option<String>,
    pub stream: os::Stream,
}

/// 建好虚拟网卡，把 `routes` 这些网段引进去。会请用户授权；用户不同意时以 `Cancelled` 失败。
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

    // 查路由表会卡一下，不占着异步运行时的线程。
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

/// 没加上路由的网段：发往它们的流量不进隧道，得让用户知道。列出前几个，其余的只报个数。
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

/// 网段里用来抽查路由的一个地址。
fn probe(net: &Cidr) -> Option<Ipv4Addr> {
    // 只含一个地址的网段就查它自己，否则查第一个地址后面的那个。
    let probe = Ipv4Addr::from(u32::from(net.addr) | u32::from(net.prefix < 32));
    // 0.0.0.0/8 不是能发包过去的地址。
    (probe.octets()[0] != 0).then_some(probe)
}

/// 抽查哪些地址：在进隧道的网段里均匀地挑几个。
fn probes(routes: &[Cidr]) -> Vec<Ipv4Addr> {
    let spacing = routes.len().div_ceil(MAX_PROBES).max(1);
    routes.iter().step_by(spacing).filter_map(probe).collect()
}

/// 路由加好之后验一遍：本该进隧道（编号为 `device` 的网卡）的流量要是去了别的网卡，返回
/// 那块网卡的名字。别的 VPN 或代理工具用更细的路由占着同一批地址时会这样。
fn intruder(device: u32, routes: &[Cidr]) -> Option<String> {
    let mut carriers = probes(routes).into_iter().filter_map(table::carrier);
    carriers.find(|(index, _)| *index != device).map(|(_, name)| name)
}

// ---- 助手这一头：以下都以管理员身份运行 ---------------------------------------

/// 命令行是在叫助手干活时，干完并返回退出码；否则返回 None，照常启动应用。
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

    let (device, described) = match check(&setup).and_then(|()| bring_up(&setup)) {
        Ok(up) => up,
        Err(error) => return write_json(&mut stream, &Ready::Error(error)).await,
    };
    let result = match write_json(&mut stream, &Ready::Device(described)).await {
        Ok(()) => pump(&device, &mut stream).await,
        Err(e) => Err(e),
    };
    // 网卡一撤，指向它的路由全都跟着消失。
    drop(device);
    // 连接留到最后才关：主进程读到它关闭时，一切已经还原。
    drop(stream);
    result
}

/// 清单来自不提权的进程，而它的每一项都会变成以管理员身份执行的操作的参数：逐项校验。
/// 网段的写法在读进来时已经验过（见 [`Cidr`]）。
fn check(setup: &Setup) -> std::result::Result<(), String> {
    if !(576..=MTU).contains(&setup.mtu) {
        return Err(format!("invalid MTU {}", setup.mtu));
    }
    if setup.routes.len() > MAX_ROUTES {
        return Err("too many routes".into());
    }
    // 整个地址空间作为一条路由，会和系统的默认路由争同一个位置：隧道一撤未必恢复得了原样。
    if let Some(all) = setup.routes.iter().find(|net| net.prefix == 0) {
        return Err(format!("refusing to replace the default route ({all})"));
    }
    Ok(())
}

/// 建网卡，加路由。
fn bring_up(setup: &Setup) -> std::result::Result<(AsyncDevice, Device), String> {
    let builder = os::configure(DeviceBuilder::new().ipv4(TUN_ADDR, TUN_PREFIX, None).mtu(setup.mtu))?;
    let device = builder.build_async().map_err(|e| format!("create the network interface: {e}"))?;
    let name = device.name().map_err(|e| format!("name the network interface: {e}"))?;
    let index = device.if_index().map_err(|e| format!("find the network interface: {e}"))?;

    // 加不上的路由不致命：别的程序占着同一个网段时会这样。记下是哪些，由主进程提醒用户。
    // 一条都加不上就是另一回事了。
    let mut skipped = Vec::new();
    let mut last = None;
    for net in &setup.routes {
        if let Err(error) = table::add(net, index) {
            skipped.push(*net);
            last = Some(error);
        }
    }
    if let Some(error) = last.filter(|_| skipped.len() == setup.routes.len()) {
        return Err(format!("add routes: {error}"));
    }
    Ok((device, Device { name, index, skipped }))
}

/// 在网卡和主进程之间搬运 IP 包，直到主进程请它收尾、断开，或者收到终止信号。
async fn pump<S>(device: &AsyncDevice, stream: &mut S) -> io::Result<()>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    let (reader, mut writer) = tokio::io::split(stream);
    let outgoing = async {
        // 前两个字节留给长度，一个包一次写出。
        let mut frame = vec![0u8; 2 + MAX_PACKET];
        loop {
            let n = device.recv(&mut frame[2..]).await?;
            // 空的一条在这条连接上另有含义，不能发。
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
            // 空的一条：主进程请这边收尾。
            if n == 0 {
                return Ok(());
            }
            reader.read_exact(&mut packet[..n]).await?;
            // 网卡一时收不下的包就丢掉：IP 本来就允许丢包，上层会重传。
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
        // 太多时只列前几个，个数照实报。
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
            // 不是只盯着开头的那几条：最后抽查的那个地址落在后半截的网段里。
            assert!(routes(count)[count / 2..].iter().any(|net| net.contains(probed[probed.len() - 1])), "{count} routes");
        }
        assert_eq!(probes(&[]), Vec::<Ipv4Addr>::new());
    }

    /// 清单里的每个值都会成为以管理员身份执行的操作的参数：不合规矩的一律拒绝。
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
        // 不替换系统的默认路由。
        assert!(refused(|s| s.routes.extend(nets(&["0.0.0.0/0"]))));
        // 写法不对的网段读都读不进来。
        assert!(serde_json::from_str::<Setup>(r#"{"mtu":9000,"routes":["10.0.0.0/8"]}"#).is_ok());
        assert!(serde_json::from_str::<Setup>(r#"{"mtu":9000,"routes":["10.0.0.0/8 metric=1"]}"#).is_err());
    }

    #[test]
    fn only_helper_commands_are_taken_over() {
        // 测试程序自己的命令行不是给助手的：照常运行。
        assert_eq!(dispatch(), None);
    }
}
