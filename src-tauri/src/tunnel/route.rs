//! 路由：网段的写法，哪些网段进隧道（纯函数），以及系统路由表的查询和添加（[`table`]）。
//! 隧道只管 IPv4。

use std::fmt;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;

use serde::{Deserialize, Serialize};

/// 一个网段：`10.0.0.0/8`；不带前缀长度的是单个地址。主机位总是清零的。
/// 在进程之间传递时写成字符串，读进来的一律重新校验。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Cidr {
    pub addr: Ipv4Addr,
    pub prefix: u8,
}

/// 高 `prefix` 位为 1 的掩码。
fn mask(prefix: u8) -> u32 {
    u32::MAX.checked_shl(32 - u32::from(prefix)).unwrap_or(0)
}

impl Cidr {
    pub fn new(addr: Ipv4Addr, prefix: u8) -> Option<Self> {
        (prefix <= 32).then(|| Self { addr: Ipv4Addr::from(u32::from(addr) & mask(prefix)), prefix })
    }

    /// 只含这一个地址的网段。
    pub fn host(addr: Ipv4Addr) -> Self {
        Self { addr, prefix: 32 }
    }

    pub fn contains(&self, ip: Ipv4Addr) -> bool {
        u32::from(ip) & mask(self.prefix) == u32::from(self.addr)
    }

    /// `other` 整个落在这个网段里。
    fn covers(&self, other: &Cidr) -> bool {
        self.prefix <= other.prefix && self.contains(other.addr)
    }

    /// 对半分开的两个网段；只含一个地址的网段分不了。
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
            // 不接受 "+8"、" 8" 之类 `parse` 会放过的写法。
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

/// 从来不进隧道的地址：本机自己的、只在本链路上有意义的，以及组播（SSH 带不了）。
const LOCAL: [&str; 3] = ["127.0.0.0/8", "169.254.0.0/16", "224.0.0.0/4"];

/// 全局模式下进隧道的网段。不用 `0.0.0.0/0`：那会和系统的默认路由争同一个位置；
/// 拆成两半后默认路由原封不动，隧道一撤就恢复原样。
const EVERYTHING: [&str; 2] = ["0.0.0.0/1", "128.0.0.0/1"];

fn nets<'a>(list: &'a [&'a str]) -> impl Iterator<Item = Cidr> + 'a {
    list.iter().filter_map(|net| net.parse().ok())
}

/// 从 `net` 里挖掉 `exclude` 里的网段，剩下的部分用尽量大的网段拼出来，放进 `kept`。
fn carve(net: Cidr, exclude: &[Cidr], kept: &mut Vec<Cidr>) {
    // 整个都在要挖掉的范围里：一点不剩。
    if exclude.iter().any(|hole| hole.covers(&net)) {
        return;
    }
    // 里面有要挖掉的一块：对半分开，两半各自再看。没有的话整个保留。
    match net.halves().filter(|_| exclude.iter().any(|hole| net.covers(hole))) {
        Some((low, high)) => {
            carve(low, exclude, kept);
            carve(high, exclude, kept);
        }
        None => kept.push(net),
    }
}

/// 进隧道的网段：每个都要加一条指向虚拟网卡的路由。
///
/// `include` 为 `None` 是全局模式，全部地址进隧道；否则只有列出的网段。不该进隧道的——
/// `bypass`、SSH 服务器自己（`server`；承载隧道的连接要是也进了隧道，就成了死循环）、
/// 本机和本链路的地址——从里面挖掉。没被隧道的路由盖住的地址，系统照原样处理，所以不必
/// 知道它们原来怎么走，也没有别的路由要加、要还原。
pub fn plan(include: Option<&[Cidr]>, bypass: &[Cidr], server: IpAddr) -> Vec<Cidr> {
    let wanted: Vec<Cidr> = match include {
        Some(include) => include.to_vec(),
        None => nets(&EVERYTHING).collect(),
    };
    // 服务器是 IPv6 地址的话，连它的连接本来就不归隧道管。
    let server = match server {
        IpAddr::V4(ip) => Some(Cidr::host(ip)),
        IpAddr::V6(_) => None,
    };
    let exclude: Vec<Cidr> = bypass.iter().copied().chain(server).chain(nets(&LOCAL)).collect();

    let mut kept = Vec::new();
    for net in wanted {
        carve(net, &exclude, &mut kept);
    }
    let mut seen = std::collections::HashSet::new();
    kept.retain(|net| seen.insert(*net));
    kept
}

/// 系统的路由表。用的是 tun-rs 配套的 `route_manager`：一套写法，各个系统通用，
/// 不必去调各系统自己的命令、再去读它们的输出。
#[cfg(any(target_os = "macos", windows))]
pub mod table {
    use std::io;
    use std::net::{IpAddr, Ipv4Addr};

    use route_manager::{Route, RouteManager};

    use super::Cidr;

    /// 发往 `ip` 的包现在从哪块网卡出去：它的编号和名字。没有路由时返回 None。
    pub fn carrier(ip: Ipv4Addr) -> Option<(u32, String)> {
        let ip = IpAddr::V4(ip);
        let mut manager = RouteManager::new().ok()?;
        // Windows 能直接问系统“这个地址走哪条路由”。
        #[cfg(windows)]
        let route = manager.find_route(&ip).ok()??;
        // 别的系统上自己在路由表里找最具体的那条。macOS 有一类只对绑在某块网卡上的连接才生效的
        // 路由（每块网卡各有一条这样的“默认路由”），对普通的连接不作数。
        #[cfg(not(windows))]
        let route = manager.list().ok()?.into_iter().filter(|route| !route.if_scope() && route.contains(&ip)).max_by_key(Route::prefix)?;
        let index = route.if_index()?;
        Some((index, route.if_name().cloned().unwrap_or_else(|| index.to_string())))
    }

    /// 加一条路由：发往 `net` 的包从编号为 `interface` 的网卡出去。需要管理员权限。
    /// 这样的路由不用删：网卡没了，它们也就没了。
    pub fn add(net: &Cidr, interface: u32) -> io::Result<()> {
        RouteManager::new()?.add(&Route::new(IpAddr::V4(net.addr), net.prefix).with_if_index(interface))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn net(s: &str) -> Cidr {
        s.parse().unwrap()
    }

    fn list(nets: &[&str]) -> Vec<Cidr> {
        nets.iter().map(|s| net(s)).collect()
    }

    fn ip(s: &str) -> Ipv4Addr {
        s.parse().unwrap()
    }

    fn strings(nets: &[Cidr]) -> Vec<String> {
        nets.iter().map(Cidr::to_string).collect()
    }

    #[test]
    fn networks_are_parsed_and_normalized() {
        assert_eq!(net("10.1.2.3/8").to_string(), "10.0.0.0/8");
        assert_eq!(net(" 192.168.1.7 ").to_string(), "192.168.1.7/32");
        assert_eq!(net("0.0.0.0/0").to_string(), "0.0.0.0/0");
        assert_eq!(net("203.0.113.9/32").to_string(), "203.0.113.9/32");
        for bad in ["", "10.0.0.0/33", "10.0.0/8", "10.0.0.0/", "10.0.0.0/+8", "10.0.0.0/8/8", "example.com", "10.0.0.0/-1"] {
            assert!(bad.parse::<Cidr>().is_err(), "{bad:?}");
        }
        // 隧道不管 IPv6。
        for v6 in ["::1", "fd00::/8", "::/0", "2001:db8::9/128"] {
            assert!(v6.parse::<Cidr>().is_err(), "{v6:?}");
        }
    }

    #[test]
    fn a_network_knows_its_addresses() {
        assert!(net("10.0.0.0/8").contains(ip("10.255.0.1")));
        assert!(!net("10.0.0.0/8").contains(ip("11.0.0.1")));
        assert!(net("0.0.0.0/0").contains(ip("8.8.8.8")));
        assert!(net("128.0.0.0/1").contains(ip("203.0.113.9")));
        assert!(net("192.168.1.7/32").contains(ip("192.168.1.7")) && !net("192.168.1.7/32").contains(ip("192.168.1.6")));

        assert!(net("10.0.0.0/8").covers(&net("10.1.0.0/16")) && net("10.0.0.0/8").covers(&net("10.0.0.0/8")));
        assert!(!net("10.1.0.0/16").covers(&net("10.0.0.0/8")) && !net("10.0.0.0/8").covers(&net("11.0.0.0/8")));
    }

    #[test]
    fn a_network_splits_into_two_halves() {
        assert_eq!(net("10.0.0.0/8").halves(), Some((net("10.0.0.0/9"), net("10.128.0.0/9"))));
        assert_eq!(net("0.0.0.0/0").halves(), Some((net("0.0.0.0/1"), net("128.0.0.0/1"))));
        assert_eq!(net("192.168.1.6/31").halves(), Some((net("192.168.1.6/32"), net("192.168.1.7/32"))));
        assert_eq!(net("192.168.1.7/32").halves(), None);
    }

    #[test]
    fn what_bypasses_is_cut_out_of_the_tunnel() {
        let carved = |from: &str, holes: &[&str]| {
            let mut kept = Vec::new();
            carve(net(from), &list(holes), &mut kept);
            strings(&kept)
        };
        // 从 10.0.0.0/8 里挖掉一个 /10：剩下两块，一块不多一块不少。
        assert_eq!(carved("10.0.0.0/8", &["10.64.0.0/10"]), ["10.0.0.0/10", "10.128.0.0/9"]);
        // 挖掉单个地址：一路对半分到底，每一层留下不含它的那一半。
        assert_eq!(carved("192.168.1.0/29", &["192.168.1.5"]), ["192.168.1.0/30", "192.168.1.4/32", "192.168.1.6/31"]);
        // 不相干的、把整个网段都盖住的。
        assert_eq!(carved("10.0.0.0/8", &["172.16.0.0/12"]), ["10.0.0.0/8"]);
        assert_eq!(carved("10.1.0.0/16", &["10.0.0.0/8"]), Vec::<String>::new());
        assert_eq!(carved("10.1.2.3/32", &["10.1.2.3"]), Vec::<String>::new());
    }

    /// 每个网段的头、尾，以及紧挨在它外面的两个地址：出错最容易出在这些边界上。
    fn edges(nets: &[Cidr]) -> Vec<Ipv4Addr> {
        let around = |net: &Cidr| {
            let first = u32::from(net.addr);
            let last = first | !mask(net.prefix);
            [first, last, first.wrapping_sub(1), last.wrapping_add(1)].map(Ipv4Addr::from)
        };
        nets.iter().flat_map(around).collect()
    }

    /// 进隧道的正是“想要的减去不要的”：拿一批地址逐个核对，同时确认各个网段互不重叠。
    fn assert_exact(routes: &[Cidr], wanted: &[Cidr], unwanted: &[Cidr], samples: &[Ipv4Addr]) {
        for &sample in samples {
            let expected = wanted.iter().any(|net| net.contains(sample)) && !unwanted.iter().any(|net| net.contains(sample));
            let covering = routes.iter().filter(|net| net.contains(sample)).count();
            assert_eq!(covering, usize::from(expected), "{sample}");
        }
    }

    #[test]
    fn global_mode_takes_everything_but_what_bypasses() {
        let server = ip("203.0.113.9");
        let bypass = list(&["10.0.0.0/8", "172.16.0.0/12", "192.168.0.0/16"]);
        let unwanted: Vec<Cidr> = bypass.iter().copied().chain(nets(&LOCAL)).chain([Cidr::host(server)]).collect();
        let mut samples = edges(&unwanted);
        samples.extend(["1.1.1.1", "8.8.8.8", "100.64.0.1", "203.0.113.8", "203.0.113.10", "223.5.5.5", "255.255.255.255"].map(ip));

        let routes = plan(None, &bypass, server.into());
        assert_exact(&routes, &list(&["0.0.0.0/0"]), &unwanted, &samples);
        // 没有哪条路由会去争系统默认路由的位置。
        assert!(routes.iter().all(|net| net.prefix >= 1));
        assert!(routes.len() < 100, "{} routes", routes.len());

        // 服务器是 IPv6 地址：没有它要挖。
        let v6_server = plan(None, &[], "2001:db8::9".parse().unwrap());
        assert_exact(&v6_server, &list(&["0.0.0.0/0"]), &nets(&LOCAL).collect::<Vec<_>>(), &samples);
        assert!(v6_server.iter().any(|net| net.contains(server)));
    }

    #[test]
    fn split_mode_takes_only_the_listed_networks() {
        let outside: IpAddr = "203.0.113.9".parse().unwrap();
        // 服务器不在其中，没有什么要挖的；写重的只算一次。
        let split = plan(Some(&list(&["10.0.0.0/8", "10.0.0.0/8", "172.16.0.0/12"])), &[], outside);
        assert_eq!(strings(&split), ["10.0.0.0/8", "172.16.0.0/12"]);

        // 服务器自己在要进隧道的网段里：单独挖掉。
        let server = ip("10.1.2.3");
        let inside = plan(Some(&list(&["10.0.0.0/8"])), &[], server.into());
        assert_eq!(inside.len(), 24);
        let mut samples = edges(&[Cidr::host(server), net("10.0.0.0/8")]);
        samples.extend(["10.1.2.2", "10.1.2.4", "10.200.0.1", "11.0.0.1", "8.8.8.8"].map(ip));
        assert_exact(&inside, &list(&["10.0.0.0/8"]), &[Cidr::host(server)], &samples);

        // 同一个网段写在两边：绕开；嵌套着写的，挖掉里面那块。
        assert_eq!(strings(&plan(Some(&list(&["10.0.0.0/8", "172.16.0.0/12"])), &list(&["10.0.0.0/8"]), outside)), ["172.16.0.0/12"]);
        assert_eq!(strings(&plan(Some(&list(&["10.0.0.0/8"])), &list(&["10.128.0.0/9"]), outside)), ["10.0.0.0/9"]);
        // 全都绕开了：没有什么要进隧道。
        assert_eq!(plan(Some(&list(&["10.1.0.0/16"])), &list(&["10.0.0.0/8"]), outside), vec![]);
    }

    #[test]
    fn networks_cross_process_boundaries_as_checked_strings() {
        assert_eq!(serde_json::to_string(&list(&["10.1.2.3/8", "192.168.1.7"])).unwrap(), r#"["10.0.0.0/8","192.168.1.7/32"]"#);
        assert_eq!(serde_json::from_str::<Vec<Cidr>>(r#"["10.1.2.3/8"]"#).unwrap(), list(&["10.0.0.0/8"]));
        // 这些值会成为以管理员身份执行的操作的参数：不合规矩的一律读不进来。
        for bad in ["default", "-interface", "10.0.0.0/8 metric=1", "203.0.113.9; reboot", "fd00::/8"] {
            assert!(serde_json::from_value::<Cidr>(bad.into()).is_err(), "{bad:?}");
        }
    }

    /// 在真实系统上查，不改任何东西。
    #[cfg(any(target_os = "macos", windows))]
    #[test]
    fn the_real_routing_table_can_be_read() {
        // 本机地址总有路由，走的是回环网卡。
        let (index, name) = table::carrier(Ipv4Addr::LOCALHOST).unwrap();
        assert!(index > 0);
        #[cfg(target_os = "macos")]
        assert_eq!(name, "lo0");
        assert!(!name.is_empty());

        // 能上网的话，公网地址也有去向。
        if let Some((index, name)) = table::carrier(Ipv4Addr::new(1, 1, 1, 1)) {
            assert!(index > 0 && !name.is_empty());
        }
    }
}
