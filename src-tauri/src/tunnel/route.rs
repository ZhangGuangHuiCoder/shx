//! 分流路由（仅 IPv4）。
//!
//! [`Rules`] 按最具体规则决定走向；[`plan`] 生成需指向虚拟网卡的网段，[`outrank`] 把与本地网络
//! 重叠的部分切细以盖过直连路由。以上为纯函数，系统路由表操作在 [`table`] 中。

use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// IPv4 网段，主机位清零；跨进程以字符串传递，读入时重新校验。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Cidr {
    pub addr: Ipv4Addr,
    pub prefix: u8,
}

fn mask(prefix: u8) -> u32 {
    u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0)
}

impl Cidr {
    pub fn new(addr: Ipv4Addr, prefix: u8) -> Option<Self> {
        (prefix <= 32).then(|| Self { addr: Ipv4Addr::from(u32::from(addr) & mask(prefix)), prefix })
    }

    pub fn host(addr: Ipv4Addr) -> Self {
        Self { addr, prefix: 32 }
    }

    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & mask(self.prefix) == u32::from(self.addr)
    }

    /// other 完全包含在本网段内。
    fn covers(&self, other: &Cidr) -> bool {
        self.prefix <= other.prefix && self.contains(other.addr)
    }

    /// 对半拆分；单地址无法拆分。
    fn halves(&self) -> Option<(Cidr, Cidr)> {
        let prefix = self.prefix + 1;
        let high = u32::from(self.addr) | 1u32.checked_shl(32u32.checked_sub(u32::from(prefix))?)?;
        Some((Cidr { addr: self.addr, prefix }, Cidr { addr: Ipv4Addr::from(high), prefix }))
    }
}

impl FromStr for Cidr {
    type Err = ();

    fn from_str(s: &str) -> Result<Self, ()> {
        let (addr, prefix) = match s.trim().split_once('/') {
            Some((addr, prefix)) => (addr, Some(prefix)),
            None => (s.trim(), None),
        };
        let addr: Ipv4Addr = addr.parse().map_err(|_| ())?;
        match prefix {
            // 拒绝 parse 会放过的 "+8"、" 8" 等写法。
            Some(prefix) if !prefix.is_empty() && prefix.bytes().all(|b| b.is_ascii_digit()) => {
                Cidr::new(addr, prefix.parse().map_err(|_| ())?).ok_or(())
            }
            Some(_) => Err(()),
            None => Ok(Cidr::host(addr)),
        }
    }
}

impl fmt::Display for Cidr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}/{}", self.addr, self.prefix)
    }
}

impl TryFrom<String> for Cidr {
    type Error = String;

    fn try_from(text: String) -> Result<Self, String> {
        text.parse().map_err(|()| format!("invalid network {text:?}"))
    }
}

impl From<Cidr> for String {
    fn from(net: Cidr) -> String {
        net.to_string()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Via {
    Tunnel,
    Direct,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    pub net: Cidr,
    pub via: Via,
}

/// 规则集：由前缀最长的规则决定，与顺序无关；同网段以后写的为准；未匹配时走 default。
#[derive(Clone, Debug, PartialEq)]
pub struct Rules {
    pub default: Via,
    pub list: Vec<Rule>,
}

impl Rules {
    /// 覆盖整个 net 的最具体规则。
    pub fn ruling(&self, net: &Cidr) -> Option<&Rule> {
        self.list.iter().filter(|rule| rule.net.covers(net)).max_by_key(|rule| rule.net.prefix)
    }

    fn via(&self, net: &Cidr) -> Via {
        self.ruling(net).map_or(self.default, |rule| rule.via)
    }

    /// 去掉冗余规则（被同网段覆盖或与上级走向相同），避免路由被无谓切碎。
    fn effective(&self) -> Rules {
        let last = |(at, rule): &(usize, &Rule)| !self.list[at + 1..].iter().any(|later| later.net == rule.net);
        let outer = |rule: &Rule| self.list.iter().filter(|other| other.net.covers(&rule.net) && other.net != rule.net).max_by_key(|other| other.net.prefix);
        let differs = |(_, rule): &(usize, &Rule)| outer(rule).map_or(self.default, |outer| outer.via) != rule.via;
        let list = self.list.iter().enumerate().filter(last).filter(differs).map(|(_, rule)| *rule).collect();
        Rules { default: self.default, list }
    }
}

/// 永不进隧道的地址：回环、链路本地、组播和广播。
const LOCAL: [&str; 4] = ["127.0.0.0/8", "169.254.0.0/16", "224.0.0.0/4", "255.255.255.255/32"];

const EVERYTHING: Cidr = Cidr { addr: Ipv4Addr::UNSPECIFIED, prefix: 0 };

fn nets<'a>(list: &'a [&'a str]) -> impl Iterator<Item = Cidr> + 'a {
    list.iter().filter_map(|net| net.parse().ok())
}

/// 永不进隧道的地址：[`LOCAL`]、SSH 服务器（否则形成环路）、本机地址和广播地址。
pub fn never(server: IpAddr, local: &Local) -> Vec<Cidr> {
    // IPv6 服务器本就不经隧道。
    let server = match server {
        IpAddr::V4(ip) => Some(Cidr::host(ip)),
        IpAddr::V6(_) => None,
    };
    nets(&LOCAL).chain(server).chain(local.own()).collect()
}

/// 用尽量大的网段覆盖 net 中需进隧道的部分。
fn settle(net: Cidr, rules: &Rules, never: &[Cidr], kept: &mut Vec<Cidr>) {
    if never.iter().any(|hole| hole.covers(&net)) {
        return;
    }
    // 内部有不同走向时对半拆分递归处理。
    let inside = |other: &Cidr| net.covers(other) && *other != net;
    match net.halves().filter(|_| never.iter().any(inside) || rules.list.iter().any(|rule| inside(&rule.net))) {
        Some((low, high)) => {
            settle(low, rules, never, kept);
            settle(high, rules, never, kept);
        }
        None if rules.via(&net) == Via::Tunnel => kept.push(net),
        None => {}
    }
}

/// 需进隧道的网段（规则结果减去 never），各加一条指向网卡的路由。
///
/// 结果不含 0.0.0.0/0，因此不触碰默认路由，撤销后即可复原。
pub fn plan(rules: &Rules, never: &[Cidr]) -> Vec<Cidr> {
    let mut kept = Vec::new();
    settle(EVERYTHING, &rules.effective(), never, &mut kept);
    kept
}

/// 把隧道网段切得比直连路由更具体以胜出；单地址无法再切时由助手替换系统路由（见 [`table::Takeover`]）。
pub fn outrank(routes: Vec<Cidr>, shadows: &[Cidr]) -> Vec<Cidr> {
    fn cut(net: Cidr, shadows: &[Cidr], kept: &mut Vec<Cidr>) {
        match net.halves().filter(|_| shadows.iter().any(|shadow| net.covers(shadow))) {
            Some((low, high)) => {
                cut(low, shadows, kept);
                cut(high, shadows, kept);
            }
            None => kept.push(net),
        }
    }
    let mut kept = Vec::new();
    for net in routes {
        cut(net, shadows, &mut kept);
    }
    kept
}

/// 本机直连网络（含虚拟机/容器内部网络）。
#[derive(Clone, Debug, PartialEq)]
pub struct Lan {
    pub interface: u32,
    pub addr: Ipv4Addr,
    pub net: Cidr,
    /// 是否为上网所用网络（含默认网关）；否则为虚拟机/容器的内部网络。
    pub uplink: bool,
}

impl Lan {
    /// 掩码非法、单地址或链路本地地址时返回 None。
    pub fn new(interface: u32, addr: Ipv4Addr, netmask: Ipv4Addr) -> Option<Self> {
        let prefix = u32::from(netmask).leading_ones();
        if u32::from(netmask).count_ones() != prefix || !(1..=31).contains(&prefix) || addr.is_link_local() {
            return None;
        }
        Some(Self { interface, addr, net: Cidr::new(addr, prefix as u8)?, uplink: false })
    }

    /// 广播地址；/31 及更小的网段没有。
    fn broadcast(&self) -> Option<Ipv4Addr> {
        (self.net.prefix < 31).then(|| Ipv4Addr::from(u32::from(self.net.addr) | !mask(self.net.prefix)))
    }
}

/// 本机直连网络，与隧道网段重叠时需被盖过。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Local {
    pub lans: Vec<Lan>,
    /// macOS 为网关单独添加的主机路由（地址、网卡编号）。
    pub pinned: Vec<(Ipv4Addr, u32)>,
}

impl Local {
    /// 本机地址和广播地址。
    fn own(&self) -> impl Iterator<Item = Cidr> + '_ {
        self.lans.iter().flat_map(|lan| std::iter::once(lan.addr).chain(lan.broadcast())).map(Cidr::host)
    }

    pub fn shadows(&self) -> Vec<Cidr> {
        self.lans.iter().map(|lan| lan.net).chain(self.pinned.iter().map(|(addr, _)| Cidr::host(*addr))).collect()
    }

    pub fn on_lan(&self, ip: Ipv4Addr) -> bool {
        self.lans.iter().any(|lan| lan.net.contains(ip))
    }
}

/// 系统路由表，使用跨平台的 route_manager。
#[cfg(any(target_os = "macos", windows))]
pub mod table {
    use std::io;
    use std::net::{IpAddr, Ipv4Addr};

    use getifaddrs::{Address, InterfaceFlags};
    use route_manager::{Route, RouteManager};

    use super::{Cidr, Lan, Local};

    /// 查询 ip 的出口网卡（编号、名称），无路由时返回 None。
    pub fn carrier(ip: Ipv4Addr) -> Option<(u32, String)> {
        let ip = IpAddr::V4(ip);
        let mut manager = RouteManager::new().ok()?;
        // Windows 可直接查询最佳路由。
        #[cfg(windows)]
        let route = manager.find_route(&ip).ok()??;
        // 其他系统自行查找最具体路由，忽略 macOS 绑定网卡（ifscope）的路由。
        #[cfg(not(windows))]
        let route = manager.list().ok()?.into_iter().filter(|route| !route.if_scope() && route.contains(&ip)).max_by_key(Route::prefix)?;
        let index = route.if_index()?;
        Some((index, route.if_name().cloned().unwrap_or_else(|| index.to_string())))
    }

    /// 需管理员权限；网卡撤除后路由随之消失，无需删除。
    pub fn add(net: &Cidr, interface: u32) -> io::Result<()> {
        RouteManager::new()?.add(&Route::new(IpAddr::V4(net.addr), net.prefix).with_if_index(interface))
    }

    /// 只读查询，失败时视为没有。
    pub fn local() -> Local {
        let mut local = Local::default();
        let Ok(interfaces) = getifaddrs::getifaddrs() else { return local };
        for interface in interfaces {
            // 排除回环和点对点网卡（如其他 VPN）。
            let (wanted, unwanted) = (InterfaceFlags::UP | InterfaceFlags::BROADCAST, InterfaceFlags::LOOPBACK | InterfaceFlags::POINTTOPOINT);
            if !interface.flags.contains(wanted) || interface.flags.intersects(unwanted) {
                continue;
            }
            let (Some(index), Address::V4(v4)) = (interface.index, &interface.address) else { continue };
            local.lans.extend(v4.netmask.and_then(|netmask| Lan::new(index, v4.address, netmask)));
        }
        // 查不到路由表时，视为都不是上网网络（全部留在本地）。
        let routes = RouteManager::new().and_then(|mut manager| manager.list()).unwrap_or_default();
        let gateways = gateways(&routes);
        for lan in &mut local.lans {
            lan.uplink = gateways.iter().any(|(gateway, interface)| lan.interface == *interface && lan.net.contains(*gateway) && lan.addr != *gateway);
        }
        #[cfg(target_os = "macos")]
        {
            local.pinned = pinned(&routes, &local.lans);
        }
        local
    }

    /// 默认路由的网关及网卡。
    fn gateways<'a>(routes: impl IntoIterator<Item = &'a Route>) -> Vec<(Ipv4Addr, u32)> {
        let gateway = |route: &Route| match (route.prefix(), route.gateway(), route.if_index()) {
            (0, Some(IpAddr::V4(gateway)), Some(interface)) if !gateway.is_unspecified() => Some((gateway, interface)),
            _ => None,
        };
        routes.into_iter().filter_map(gateway).collect()
    }

    /// macOS 为直连网关添加的主机路由；隧道要接管该地址须先替换它。
    #[cfg(target_os = "macos")]
    fn pinned(routes: &[Route], lans: &[Lan]) -> Vec<(Ipv4Addr, u32)> {
        // 忽略 ifscope 路由。
        let unscoped = || routes.iter().filter(|route| !route.if_scope());
        let direct = |(addr, interface): &(Ipv4Addr, u32)| {
            let on_lan = lans.iter().any(|lan| lan.interface == *interface && lan.net.contains(*addr) && lan.addr != *addr);
            let own_route = |route: &Route| route.prefix() == 32 && route.destination() == IpAddr::V4(*addr) && route.gateway().is_none() && route.if_index() == Some(*interface);
            on_lan && unscoped().any(own_route)
        };
        let mut pinned: Vec<(Ipv4Addr, u32)> = gateways(unscoped()).into_iter().filter(direct).collect();
        pinned.dedup();
        pinned
    }

    /// 接管本地网络时对路由表的改动，drop 时还原。Windows 无需改动；macOS 需处理两处：
    ///
    /// - 网关也经路由表查找，隧道路由盖住本地网段后默认路由会找不到网关。
    ///   因此为本地网段添加 ifscope 路由，保证网关查找不受影响。
    /// - 网关主机路由（[`Local::pinned`]）与隧道路由冲突时，删除它腾位，关闭时放回。
    #[derive(Default)]
    pub struct Takeover {
        #[cfg(target_os = "macos")]
        pinned: Vec<(Ipv4Addr, u32)>,
        #[cfg(target_os = "macos")]
        scoped: Vec<Route>,
        #[cfg(target_os = "macos")]
        displaced: Vec<(Ipv4Addr, u32)>,
    }

    impl Takeover {
        /// 加隧道路由前的准备。
        pub fn prepare(local: &Local, routes: &[Cidr]) -> Self {
            #[cfg(target_os = "macos")]
            {
                let overlapped = |lan: &&Lan| routes.iter().any(|net| lan.net.covers(net) || net.covers(&lan.net));
                let scoped = |lan: &Lan| Route::new(IpAddr::V4(lan.net.addr), lan.net.prefix).with_if_index(lan.interface).with_if_scope(true);
                // 已存在的路由加不上，也无需还原。
                let added = |route: &Route| RouteManager::new().and_then(|mut manager| manager.add(route)).is_ok();
                Self { pinned: local.pinned.clone(), scoped: local.lans.iter().filter(overlapped).map(scoped).filter(added).collect(), displaced: Vec::new() }
            }
            #[cfg(not(target_os = "macos"))]
            {
                let _ = (local, routes);
                Self::default()
            }
        }

        /// 路由添加失败时，若被网关主机路由占位则删除它；返回 true 表示可重试。
        pub fn make_room(&mut self, net: &Cidr) -> bool {
            #[cfg(target_os = "macos")]
            if let Some(pin) = self.pinned.iter().find(|(addr, _)| *net == Cidr::host(*addr)) {
                let route = Route::new(IpAddr::V4(pin.0), 32);
                if RouteManager::new().and_then(|mut manager| manager.delete(&route)).is_ok() {
                    self.displaced.push(*pin);
                    return true;
                }
            }
            let _ = net;
            false
        }
    }

    /// 须在撤网卡后 drop，删除的路由才能放回。
    impl Drop for Takeover {
        fn drop(&mut self) {
            #[cfg(target_os = "macos")]
            {
                for (addr, interface) in self.displaced.drain(..) {
                    let _ = pin::add(addr, interface);
                }
                for route in self.scoped.drain(..) {
                    let _ = RouteManager::new().and_then(|mut manager| manager.delete(&route));
                }
            }
        }
    }

    /// 网关主机路由（可克隆、派生邻居表项）route_manager 无法添加，需手写路由消息。
    #[cfg(target_os = "macos")]
    mod pin {
        use std::io::{self, Write};
        use std::net::Ipv4Addr;
        use std::os::fd::FromRawFd;
        use std::os::unix::net::UnixStream;

        // 取自 <net/route.h>、<sys/socket.h>。
        const RTM_VERSION: u8 = 5;
        const RTM_ADD: u8 = 1;
        const RTF_UP: i32 = 0x1;
        const RTF_CLONING: i32 = 0x100;
        const RTF_STATIC: i32 = 0x800;
        const RTA_DST: i32 = 0x1;
        const RTA_GATEWAY: i32 = 0x2;
        const RTA_NETMASK: i32 = 0x4;
        const PF_ROUTE: i32 = 17;
        const SOCK_RAW: i32 = 3;
        const AF_INET: u8 = 2;
        const AF_LINK: u8 = 18;
        const HEADER: usize = 92;
        const INET: usize = 16;
        const LINK: usize = 20;

        extern "C" {
            fn socket(domain: i32, kind: i32, protocol: i32) -> i32;
        }

        /// RTM_ADD 消息：addr/32 直接从 interface 网卡发出。
        pub(super) fn message(addr: Ipv4Addr, interface: u32) -> io::Result<Vec<u8>> {
            let interface = u16::try_from(interface).map_err(|_| io::ErrorKind::InvalidInput)?;
            let mut message = vec![0u8; HEADER + INET + LINK + INET];
            let len = message.len() as u16;
            // rt_msghdr 头部：长度、版本、类型、标志与地址掩码。
            message[..2].copy_from_slice(&len.to_ne_bytes());
            message[2] = RTM_VERSION;
            message[3] = RTM_ADD;
            message[8..12].copy_from_slice(&(RTF_UP | RTF_STATIC | RTF_CLONING).to_ne_bytes());
            message[12..16].copy_from_slice(&(RTA_DST | RTA_GATEWAY | RTA_NETMASK).to_ne_bytes());
            message[20..24].copy_from_slice(&1i32.to_ne_bytes());

            let inet = |at: usize, ip: Ipv4Addr, message: &mut [u8]| {
                message[at] = INET as u8;
                message[at + 1] = AF_INET;
                message[at + 4..at + 8].copy_from_slice(&ip.octets());
            };
            inet(HEADER, addr, &mut message);
            // 网关位置为只含编号的 sockaddr_dl。
            let link = HEADER + INET;
            message[link] = LINK as u8;
            message[link + 1] = AF_LINK;
            message[link + 2..link + 4].copy_from_slice(&interface.to_ne_bytes());
            inet(link + LINK, Ipv4Addr::BROADCAST, &mut message);
            Ok(message)
        }

        pub(super) fn add(addr: Ipv4Addr, interface: u32) -> io::Result<()> {
            let message = message(addr, interface)?;
            // SAFETY: 参数均为整数。
            let fd = unsafe { socket(PF_ROUTE, SOCK_RAW, 0) };
            if fd < 0 {
                return Err(io::Error::last_os_error());
            }
            // SAFETY: fd 新打开且独占，由 UnixStream 负责关闭。
            let mut socket = unsafe { UnixStream::from_raw_fd(fd) };
            // 系统拒绝时写入失败并返回错误。
            socket.write_all(&message)
        }
    }
}
