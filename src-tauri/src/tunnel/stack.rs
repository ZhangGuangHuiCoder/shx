//! 用户态协议栈：把虚拟网卡上的 IP 包还原成一条条 TCP 连接，逐条转发出去。
//!
//! TCP 连接在本机就终结了，转发的是连接里的数据而不是 IP 包，所以没有“TCP 套 TCP”的问题；
//! 代价是只带得了 TCP。隧道不管域名解析，但 DNS 查询要是发往一个进了隧道的地址，也得有
//! 着落，否则域名全都解析不了：它是唯一处理的 UDP，改用 TCP 转发给原来要问的那台服务器。
//! TCP 上的应答可以很长，UDP 里放不下时，按规矩告诉客户端“被截断了”，让它自己改用 TCP 再问。

use std::future::poll_fn;
use std::io;
use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::task::Poll;
use std::time::Duration;

use futures::future::BoxFuture;
use futures::{SinkExt, StreamExt};
use netstack_smoltcp::smoltcp::wire::{IpProtocol, Ipv4Packet, UdpPacket};
use netstack_smoltcp::{StackBuilder, TcpListener, TcpStream, UdpSocket};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt, BufReader};
use tokio::sync::{mpsc, OwnedSemaphorePermit, Semaphore};
use tokio::task::JoinSet;

use super::wire::{read_frame, FrameWriter};
use super::MTU;

const DIAL_TIMEOUT: Duration = Duration::from_secs(20);
const DNS_TIMEOUT: Duration = Duration::from_secs(8);
/// 同时在办的 DNS 查询数；超出的不理会，客户端会重发。
const MAX_QUERIES: usize = 128;
/// 同时转发的连接数；超出的不转发，直接关掉。正常使用远到不了这个数，到了多半是本机有程序
/// 在扫端口：每条连接都要在 SSH 上开一个通道，不能由着它开。
const MAX_CONNECTIONS: usize = 1024;
/// 一条连接从出现到可以关闭，至少要过这么久。协议栈在握手完成之前关不干净连接：
/// 结束标志发不到应用那里，应用会一直挂着。本机的握手转眼就完成，但转发失败得更快时
/// （比如 SSH 连接正断着）得等它一下。
const HANDSHAKE_GRACE: Duration = Duration::from_millis(200);
const COPY_BUFFER: usize = 32 * 1024;
const READ_BUFFER: usize = 256 * 1024;

/// 转发出去的一条连接。
pub trait Duplex: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Duplex for T {}
pub type Conn = Box<dyn Duplex>;

/// 连到 `主机:端口`；最后一个参数是这条连接在本机的发起方。
pub type Dialer = Arc<dyn Fn(String, u16, SocketAddr) -> BoxFuture<'static, io::Result<Conn>> + Send + Sync>;

pub struct Tunnel {
    dial: Dialer,
    /// 应用发出去的字节数。
    pub up: AtomicU64,
    /// 应用收到的字节数。
    pub down: AtomicU64,
    /// 正在转发的连接数。
    pub connections: AtomicU64,
    max_connections: usize,
}

impl Tunnel {
    pub fn new(dial: Dialer) -> Self {
        Self { dial, up: AtomicU64::new(0), down: AtomicU64::new(0), connections: AtomicU64::new(0), max_connections: MAX_CONNECTIONS }
    }
}

/// 运行协议栈：`reader` 上是虚拟网卡收到的 IP 包，`writer` 是发回给它的。
/// 一直运行到这条链路断开，返回断开的原因。
pub async fn run<R, W>(reader: &mut R, writer: &mut FrameWriter<W>, tunnel: Arc<Tunnel>) -> io::Error
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let built = StackBuilder::default().enable_tcp(true).enable_udp(true).mtu(usize::from(MTU)).build();
    let (stack, runner, udp, tcp) = match built {
        Ok((stack, Some(runner), Some(udp), Some(tcp))) => (stack, runner, udp, tcp),
        Ok(_) => return io::Error::other("the network stack is incomplete"),
        Err(e) => return e,
    };
    let (mut to_stack, mut from_stack) = stack.split();

    // 这些任务随本函数结束（`tasks` 被丢弃）而终止，连同它们各自派生的任务。
    let mut tasks = JoinSet::new();
    tasks.spawn(async move {
        let _ = runner.await;
    });
    tasks.spawn(accept(tcp, tunnel.clone()));
    tasks.spawn(datagrams(udp, tunnel.clone()));

    let inbound = async {
        let mut reader = BufReader::with_capacity(READ_BUFFER, reader);
        loop {
            let packet = match read_frame(&mut reader).await {
                Ok(packet) => packet,
                Err(e) => return e,
            };
            // 隧道带不了的包（DNS 以外的 UDP、ICMP、IPv6）就此丢掉。
            if !carried(&packet) {
                continue;
            }
            // 协议栈不认的包只是被丢掉；它自己停了才算出错。
            if let Err(e) = to_stack.send(packet).await {
                if e.kind() == io::ErrorKind::BrokenPipe {
                    return e;
                }
            }
        }
    };
    let outbound = async {
        loop {
            let packet = match from_stack.next().await {
                Some(Ok(packet)) => packet,
                Some(Err(e)) => return e,
                None => return io::Error::other("the network stack stopped"),
            };
            if let Err(e) = writer.send(&packet).await {
                return e;
            }
        }
    };
    tokio::select! {
        e = inbound => e,
        e = outbound => e,
        _ = tasks.join_next() => io::Error::other("the network stack stopped"),
    }
}

// ---- 进协议栈之前：哪些包带得了 ---------------------------------------------

const DNS_PORT: u16 = 53;

/// 隧道带得了的包：IPv4 的 TCP，以及 DNS 查询。别的 UDP、ICMP、IPv6 都带不了。
pub fn carried(packet: &[u8]) -> bool {
    if packet.first().map(|byte| byte >> 4) != Some(4) {
        return false;
    }
    let Ok(ip) = Ipv4Packet::new_checked(packet) else {
        return false;
    };
    match ip.next_header() {
        IpProtocol::Tcp => true,
        // 协议栈遇到残缺的 UDP 包会停止接收，所以在这里先验一遍。
        IpProtocol::Udp => UdpPacket::new_checked(ip.payload()).is_ok_and(|udp| udp.dst_port() == DNS_PORT),
        _ => false,
    }
}

// ---- TCP：逐条连接转发 -------------------------------------------------------

/// 应用每发起一条 TCP 连接，协议栈就在这里交出一条。
async fn accept(mut listener: TcpListener, tunnel: Arc<Tunnel>) {
    let limit = Arc::new(Semaphore::new(tunnel.max_connections));
    let mut connections = JoinSet::new();
    loop {
        tokio::select! {
            next = listener.next() => match next {
                Some((stream, from, to)) => {
                    let permit = limit.clone().try_acquire_owned().ok();
                    connections.spawn(forward(tunnel.clone(), stream, from, to, permit));
                }
                None => break,
            },
            Some(_) = connections.join_next() => {}
        }
    }
}

/// 正在转发的连接数，随连接结束自动减回去。
struct Open<'a>(&'a AtomicU64);

impl<'a> Open<'a> {
    fn new(count: &'a AtomicU64) -> Self {
        count.fetch_add(1, Ordering::Relaxed);
        Self(count)
    }
}

impl Drop for Open<'_> {
    fn drop(&mut self) {
        self.0.fetch_sub(1, Ordering::Relaxed);
    }
}

/// 把应用的一条连接（`from` → `to`）转发出去，直到它结束。连不上时直接放手：协议栈会把它关掉。
/// `permit` 是它占的名额（见 [`MAX_CONNECTIONS`]），随它结束而归还；没占到的不转发。
async fn forward(tunnel: Arc<Tunnel>, mut local: TcpStream, from: SocketAddr, to: SocketAddr, permit: Option<OwnedSemaphorePermit>) {
    let ready = tokio::time::Instant::now() + HANDSHAKE_GRACE;
    if permit.is_some() {
        carry(&tunnel, &mut local, from, to, ready).await;
    }
    tokio::time::sleep_until(ready).await;
}

/// 转发的本体。`ready` 之前不能关闭 `local`（见 [`HANDSHAKE_GRACE`]）。
async fn carry(tunnel: &Tunnel, local: &mut TcpStream, from: SocketAddr, to: SocketAddr, ready: tokio::time::Instant) {
    let dialing = (tunnel.dial)(to.ip().to_string(), to.port(), from);
    let Ok(Ok(remote)) = tokio::time::timeout(DIAL_TIMEOUT, dialing).await else {
        return;
    };
    let _open = Open::new(&tunnel.connections);

    let (mut local_rx, mut local_tx) = tokio::io::split(local);
    let (mut remote_rx, mut remote_tx) = tokio::io::split(remote);
    let up = async {
        copy(&mut local_rx, &mut remote_tx, &tunnel.up).await?;
        remote_tx.shutdown().await
    };
    let down = async {
        copy(&mut remote_rx, &mut local_tx, &tunnel.down).await?;
        tokio::time::sleep_until(ready).await;
        // 只把“对方说完了”转达给应用，不等它返回：协议栈的 shutdown 要到连接彻底关闭才完成。
        poll_fn(|cx| {
            let _ = Pin::new(&mut local_tx).poll_shutdown(cx);
            Poll::Ready(())
        })
        .await;
        Ok(())
    };
    // 一个方向说完了，另一个方向照常继续；任何一边出错则整条连接作废。
    let _ = tokio::try_join!(up, down);
}

async fn copy<R, W>(from: &mut R, to: &mut W, count: &AtomicU64) -> io::Result<()>
where
    R: AsyncRead + Unpin,
    W: AsyncWrite + Unpin,
{
    let mut buffer = vec![0u8; COPY_BUFFER];
    loop {
        let n = from.read(&mut buffer).await?;
        if n == 0 {
            return Ok(());
        }
        to.write_all(&buffer[..n]).await?;
        count.fetch_add(n as u64, Ordering::Relaxed);
    }
}

// ---- DNS：唯一处理的 UDP，改用 TCP 转发 ---------------------------------------

/// 交到这里的 UDP 包都是 DNS 查询（其余的在进协议栈之前就丢掉了）。
async fn datagrams(socket: UdpSocket, tunnel: Arc<Tunnel>) {
    let (mut incoming, mut outgoing) = socket.split();
    let (answer_tx, mut answers) = mpsc::channel(64);
    let limit = Arc::new(Semaphore::new(MAX_QUERIES));
    let mut queries = JoinSet::new();
    loop {
        tokio::select! {
            message = incoming.next() => {
                let Some((query, from, to)) = message else { break };
                let Ok(permit) = limit.clone().try_acquire_owned() else { continue };
                let (tunnel, answer_tx) = (tunnel.clone(), answer_tx.clone());
                queries.spawn(async move {
                    let _permit = permit;
                    if let Some(answer) = ask(&tunnel, &query, from, to).await {
                        // 应答从被问的那个地址发回去。
                        let _ = answer_tx.send((answer, to, from)).await;
                    }
                });
            }
            Some(answer) = answers.recv() => {
                let _ = outgoing.send(answer).await;
            }
            Some(_) = queries.join_next() => {}
        }
    }
}

/// 把一个 DNS 查询改用 TCP 转发给它原来要问的服务器（`to`），返回应答；
/// 问不到时返回 None，客户端自己会重试。
async fn ask(tunnel: &Tunnel, query: &[u8], from: SocketAddr, to: SocketAddr) -> Option<Vec<u8>> {
    let exchange = async {
        let mut conn = (tunnel.dial)(to.ip().to_string(), to.port(), from).await.ok()?;
        // TCP 上的 DNS 报文前面带两字节的长度。
        let mut message = Vec::with_capacity(query.len() + 2);
        message.extend_from_slice(&u16::try_from(query.len()).ok()?.to_be_bytes());
        message.extend_from_slice(query);
        conn.write_all(&message).await.ok()?;
        let len = conn.read_u16().await.ok()?;
        let mut answer = vec![0u8; usize::from(len)];
        conn.read_exact(&mut answer).await.ok()?;
        Some(answer)
    };
    let answer = tokio::time::timeout(DNS_TIMEOUT, exchange).await.ok()??;
    // UDP 里放不下的应答：告诉客户端改用 TCP 来问——那是一条普通的连接，照常转发。
    if answer.len() > udp_limit(query) {
        return truncated_answer(query);
    }
    Some(answer)
}

const DNS_HEADER: usize = 12;
/// 没有声明能收多大应答的查询，UDP 应答不能超过这个长度（RFC 1035）。
const PLAIN_LIMIT: usize = 512;
/// 带 EDNS 的查询声明的长度各不相同，这是公认都收得下的大小。
const EDNS_LIMIT: usize = 1232;

/// 查询里那个问题结束的位置。不是只带一个问题的普通查询时返回 None。
fn question_end(query: &[u8]) -> Option<usize> {
    let header = query.get(..DNS_HEADER)?;
    let is_response = header[2] & 0x80 != 0;
    let questions = u16::from_be_bytes([header[4], header[5]]);
    if is_response || questions != 1 {
        return None;
    }
    // 域名是一串带长度前缀的标签，以长度 0 结束；问题里的域名不压缩。
    let mut at = DNS_HEADER;
    loop {
        let len = usize::from(*query.get(at)?);
        at += 1;
        if len == 0 {
            break;
        }
        if len > 63 {
            return None;
        }
        at += len;
    }
    // 后面是类型和类别，各两个字节。
    query.get(at..at + 4)?;
    Some(at + 4)
}

/// “应答太长，UDP 里放不下”：照抄查询的编号和问题，不带任何记录。客户端收到后会改用 TCP 再问一遍。
fn truncated_answer(query: &[u8]) -> Option<Vec<u8>> {
    let mut answer = query[..question_end(query)?].to_vec();
    // QR=1（这是应答），TC=1（被截断了）；保留操作码和 RD。
    answer[2] = 0x80 | 0x02 | (query[2] & 0x79);
    // RA=1，RCODE=0（没有错误）。
    answer[3] = 0x80;
    // 应答、授权、附加三段都是空的。
    answer[6..DNS_HEADER].fill(0);
    Some(answer)
}

/// 这个查询的 UDP 应答最长可以多长。
fn udp_limit(query: &[u8]) -> usize {
    let additional = query.get(10..DNS_HEADER).map_or(0, |n| u16::from_be_bytes([n[0], n[1]]));
    if additional == 0 { PLAIN_LIMIT } else { EDNS_LIMIT }
}

#[cfg(test)]
mod packet_tests {
    use super::*;
    use netstack_smoltcp::smoltcp::phy::ChecksumCapabilities;
    use netstack_smoltcp::smoltcp::wire::{Ipv4Address, Ipv4Repr, TcpControl, TcpPacket, TcpRepr, TcpSeqNumber, UdpRepr};

    const FROM: Ipv4Address = Ipv4Address::new(198, 19, 255, 253);
    const TO: Ipv4Address = Ipv4Address::new(93, 184, 216, 34);

    /// 一个 IPv4 包：`protocol` 的头部和内容由 `fill` 填进 `len` 个字节里。
    fn v4(protocol: IpProtocol, len: usize, fill: impl FnOnce(&mut [u8])) -> Vec<u8> {
        let ip = Ipv4Repr { src_addr: FROM, dst_addr: TO, next_header: protocol, payload_len: len, hop_limit: 64 };
        let mut buffer = vec![0u8; ip.buffer_len() + len];
        ip.emit(&mut Ipv4Packet::new_unchecked(&mut buffer[..]), &ChecksumCapabilities::default());
        fill(&mut buffer[ip.buffer_len()..]);
        buffer
    }

    fn tcp_syn() -> Vec<u8> {
        let tcp = TcpRepr {
            src_port: 50000,
            dst_port: 443,
            control: TcpControl::Syn,
            seq_number: TcpSeqNumber(1000),
            ack_number: None,
            window_len: 65535,
            window_scale: None,
            max_seg_size: None,
            sack_permitted: false,
            sack_ranges: [None, None, None],
            timestamp: None,
            payload: &[],
        };
        v4(IpProtocol::Tcp, tcp.buffer_len(), |buffer| {
            tcp.emit(&mut TcpPacket::new_unchecked(buffer), &FROM.into(), &TO.into(), &ChecksumCapabilities::default());
        })
    }

    fn udp(dst_port: u16, payload: &[u8]) -> Vec<u8> {
        let udp = UdpRepr { src_port: 40000, dst_port };
        v4(IpProtocol::Udp, udp.header_len() + payload.len(), |buffer| {
            let fill = |body: &mut [u8]| body.copy_from_slice(payload);
            udp.emit(&mut UdpPacket::new_unchecked(buffer), &FROM.into(), &TO.into(), payload.len(), fill, &ChecksumCapabilities::default());
        })
    }

    #[test]
    fn tcp_and_dns_reach_the_stack_and_the_rest_is_dropped() {
        assert!(carried(&tcp_syn()));
        assert!(carried(&udp(53, b"query")));
        // QUIC 之类：SSH 带不了 UDP。
        assert!(!carried(&udp(443, b"quic")));
        // ping。
        assert!(!carried(&v4(IpProtocol::Icmp, 8, |buffer| buffer[0] = 8)));

        // 残缺的 UDP 包不能交给协议栈。
        let mut short = udp(53, b"query");
        short.truncate(24);
        short[3] = 24;
        assert!(!carried(&short));
        for junk in [&[][..], &[0x45], &[0x00; 40]] {
            assert!(!carried(junk));
        }
    }

    #[test]
    fn ipv6_is_not_carried() {
        // 一个像模像样的 IPv6 包头：版本 6，载荷是 TCP。
        let mut v6 = vec![0u8; 60];
        v6[0] = 0x60;
        v6[5] = 20;
        v6[6] = 6;
        assert!(!carried(&v6));
    }
}

#[cfg(test)]
mod dns_tests {
    use super::*;

    /// `name` 的 `kind` 类型查询，编号 0x1234，要求递归。
    pub fn query(name: &str, kind: u16, edns: bool) -> Vec<u8> {
        let mut q = vec![0x12, 0x34, 0x01, 0x00, 0, 1, 0, 0, 0, 0, 0, u8::from(edns)];
        for label in name.split('.') {
            q.push(label.len() as u8);
            q.extend_from_slice(label.as_bytes());
        }
        q.push(0);
        q.extend_from_slice(&kind.to_be_bytes());
        q.extend_from_slice(&[0, 1]);
        if edns {
            // OPT 记录：根域名，类型 41，声明能收 4096 字节。
            q.extend_from_slice(&[0, 0, 41, 0x10, 0x00, 0, 0, 0, 0, 0, 0]);
        }
        q
    }

    #[test]
    fn a_truncated_answer_echoes_the_question_and_nothing_else() {
        let q = query("example.com", 16, true);
        let end = q.len() - 11;
        let answer = truncated_answer(&q).unwrap();
        assert_eq!(answer.len(), end, "the OPT record is dropped");
        assert_eq!(&answer[..2], &[0x12, 0x34]);
        assert_eq!(answer[2], 0x83, "a response, truncated, recursion still desired");
        assert_eq!(answer[3], 0x80, "recursion available, no error");
        assert_eq!(&answer[4..DNS_HEADER], &[0, 1, 0, 0, 0, 0, 0, 0]);
        assert_eq!(&answer[DNS_HEADER..], &q[DNS_HEADER..end]);
    }

    #[test]
    fn what_is_not_a_plain_query_gets_no_made_up_answer() {
        let plain = query("example.com", 1, false);
        assert!(truncated_answer(&plain).is_some());
        // 应答、没有问题、残缺的报文、超长的标签。
        let mut response = plain.clone();
        response[2] |= 0x80;
        assert_eq!(truncated_answer(&response), None);
        let mut no_question = plain.clone();
        no_question[5] = 0;
        assert_eq!(truncated_answer(&no_question), None);
        for cut in [0, 5, DNS_HEADER, DNS_HEADER + 3, plain.len() - 1] {
            assert_eq!(truncated_answer(&plain[..cut]), None, "cut at {cut}");
        }
        let mut long_label = plain.clone();
        long_label[DNS_HEADER] = 64;
        assert_eq!(truncated_answer(&long_label), None);
    }

    #[test]
    fn udp_answers_are_limited_by_what_the_query_allows() {
        assert_eq!(udp_limit(&query("example.com", 1, false)), 512);
        assert_eq!(udp_limit(&query("example.com", 1, true)), 1232);
        assert_eq!(udp_limit(&[]), 512);
    }
}

/// 不经 SSH、也不需要真的虚拟网卡的端到端测试：测试里另起一个 TCP/IP 协议栈扮演操作系统，
/// 包经一条内存里的链路和被测的协议栈往来；“转发出去”的连接直接连到本机的测试服务。
#[cfg(test)]
mod tests {
    use std::collections::{HashMap, VecDeque};
    use std::net::IpAddr;

    use netstack_smoltcp::smoltcp::iface::{Config, Interface, SocketHandle, SocketSet};
    use netstack_smoltcp::smoltcp::phy::{Device, DeviceCapabilities, Medium, RxToken, TxToken};
    use netstack_smoltcp::smoltcp::socket::{tcp, udp};
    use netstack_smoltcp::smoltcp::time::Instant;
    use netstack_smoltcp::smoltcp::wire::{HardwareAddress, IpAddress, IpCidr, IpEndpoint};
    use parking_lot::Mutex;
    use tokio::io::{DuplexStream, WriteHalf};
    use tokio::net::{TcpListener as LocalListener, TcpStream as LocalStream};

    use super::*;
    use super::dns_tests::query;
    use crate::tunnel::{TUN_ADDR, TUN_PREFIX};

    #[derive(Default)]
    struct Queue {
        rx: VecDeque<Vec<u8>>,
        tx: VecDeque<Vec<u8>>,
    }

    struct Rx(Vec<u8>);
    struct Tx<'a>(&'a mut VecDeque<Vec<u8>>);

    impl RxToken for Rx {
        fn consume<R, F: FnOnce(&[u8]) -> R>(self, f: F) -> R {
            f(&self.0)
        }
    }

    impl TxToken for Tx<'_> {
        fn consume<R, F: FnOnce(&mut [u8]) -> R>(self, len: usize, f: F) -> R {
            let mut frame = vec![0u8; len];
            let result = f(&mut frame);
            self.0.push_back(frame);
            result
        }
    }

    impl Device for Queue {
        type RxToken<'a> = Rx;
        type TxToken<'a> = Tx<'a>;

        fn receive(&mut self, _: Instant) -> Option<(Rx, Tx<'_>)> {
            let frame = self.rx.pop_front()?;
            Some((Rx(frame), Tx(&mut self.tx)))
        }

        fn transmit(&mut self, _: Instant) -> Option<Tx<'_>> {
            Some(Tx(&mut self.tx))
        }

        fn capabilities(&self) -> DeviceCapabilities {
            let mut caps = DeviceCapabilities::default();
            caps.medium = Medium::Ip;
            caps.max_transmission_unit = usize::from(MTU);
            caps
        }
    }

    /// 扮演操作系统的那一头：地址和路由照虚拟网卡的样子配置。
    struct Host {
        iface: Interface,
        device: Queue,
        sockets: SocketSet<'static>,
        incoming: mpsc::UnboundedReceiver<Vec<u8>>,
        writer: WriteHalf<DuplexStream>,
        next_port: u16,
    }

    impl Host {
        fn start(tunnel: Arc<Tunnel>) -> Self {
            let (ours, theirs) = tokio::io::duplex(1024 * 1024);
            tokio::spawn(async move {
                let (mut reader, writer) = tokio::io::split(theirs);
                run(&mut reader, &mut FrameWriter::new(writer), tunnel).await;
            });
            let (mut reader, writer) = tokio::io::split(ours);
            let (frames, incoming) = mpsc::unbounded_channel();
            tokio::spawn(async move {
                while let Ok(frame) = read_frame(&mut reader).await {
                    if frames.send(frame).is_err() {
                        break;
                    }
                }
            });

            let mut device = Queue::default();
            let mut iface = Interface::new(Config::new(HardwareAddress::Ip), &mut device, Instant::now());
            iface.update_ip_addrs(|addrs| {
                addrs.push(IpCidr::new(IpAddress::Ipv4(TUN_ADDR), TUN_PREFIX)).unwrap();
            });
            iface.routes_mut().add_default_ipv4_route("198.19.255.254".parse().unwrap()).unwrap();
            Self { iface, device, sockets: SocketSet::new(vec![]), incoming, writer, next_port: 49152 }
        }

        /// 收发一轮包。
        async fn step(&mut self) {
            while let Ok(frame) = self.incoming.try_recv() {
                self.device.rx.push_back(frame);
            }
            self.iface.poll(Instant::now(), &mut self.device, &mut self.sockets);
            let mut frames = FrameWriter::new(&mut self.writer);
            while let Some(frame) = self.device.tx.pop_front() {
                frames.send(&frame).await.unwrap();
            }
            tokio::time::sleep(Duration::from_millis(1)).await;
        }

        async fn until(&mut self, what: &str, mut done: impl FnMut(&mut Self) -> bool) {
            let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
            while !done(self) {
                assert!(tokio::time::Instant::now() < deadline, "timed out waiting until {what}");
                self.step().await;
            }
        }

        /// 让一段时间过去，期间照常收发包。
        async fn idle(&mut self, time: Duration) {
            let end = tokio::time::Instant::now() + time;
            while tokio::time::Instant::now() < end {
                self.step().await;
            }
        }

        fn port(&mut self) -> u16 {
            self.next_port += 1;
            self.next_port
        }

        fn connect(&mut self, addr: &str, port: u16) -> SocketHandle {
            let buffer = || tcp::SocketBuffer::new(vec![0u8; 256 * 1024]);
            let handle = self.sockets.add(tcp::Socket::new(buffer(), buffer()));
            let local = self.port();
            let remote = IpEndpoint::new(addr.parse::<IpAddr>().unwrap().into(), port);
            self.sockets.get_mut::<tcp::Socket>(handle).connect(self.iface.context(), remote, local).unwrap();
            handle
        }

        fn tcp(&mut self, handle: SocketHandle) -> &mut tcp::Socket<'static> {
            self.sockets.get_mut(handle)
        }

        async fn open(&mut self, addr: &str, port: u16) -> SocketHandle {
            let handle = self.connect(addr, port);
            self.until("the connection opens", |host| host.tcp(handle).may_send()).await;
            handle
        }

        async fn send(&mut self, handle: SocketHandle, data: &[u8]) {
            let mut sent = 0;
            self.until("the data is sent", |host| {
                sent += host.tcp(handle).send_slice(&data[sent..]).unwrap();
                sent == data.len()
            })
            .await;
        }

        async fn recv(&mut self, handle: SocketHandle, len: usize) -> Vec<u8> {
            let mut data = vec![0u8; len];
            let mut got = 0;
            self.until("the data arrives", |host| {
                let socket = host.tcp(handle);
                if socket.can_recv() {
                    got += socket.recv_slice(&mut data[got..]).unwrap();
                }
                got == len
            })
            .await;
            data
        }

        /// 发一个 UDP 包，等它的回包。
        async fn ask(&mut self, to: (IpAddr, u16), payload: &[u8]) -> (Vec<u8>, IpEndpoint) {
            let handle = self.udp_send(to, payload);
            let mut answer = None;
            self.until("the answer arrives", |host| {
                answer = host.sockets.get_mut::<udp::Socket>(handle).recv().ok().map(|(data, meta)| (data.to_vec(), meta.endpoint));
                answer.is_some()
            })
            .await;
            answer.unwrap()
        }

        fn udp_send(&mut self, to: (IpAddr, u16), payload: &[u8]) -> SocketHandle {
            let buffer = || udp::PacketBuffer::new(vec![udp::PacketMetadata::EMPTY; 8], vec![0u8; 16 * 1024]);
            let mut socket = udp::Socket::new(buffer(), buffer());
            socket.bind(self.port()).unwrap();
            socket.send_slice(payload, IpEndpoint::new(to.0.into(), to.1)).unwrap();
            self.sockets.add(socket)
        }
    }

    type Dialed = Arc<Mutex<Vec<(String, u16)>>>;

    /// 一条隧道，它“转发出去”的连接按目标端口连到本机的测试服务上；没有对应服务的端口连不上。
    fn tunnel(services: &[(u16, SocketAddr)]) -> (Arc<Tunnel>, Dialed) {
        let services: HashMap<u16, SocketAddr> = services.iter().copied().collect();
        let dialed = Dialed::default();
        let record = dialed.clone();
        let dial: Dialer = Arc::new(move |host, port, _from| {
            record.lock().push((host, port));
            let service = services.get(&port).copied();
            Box::pin(async move {
                let service = service.ok_or(io::ErrorKind::ConnectionRefused)?;
                Ok(Box::new(LocalStream::connect(service).await?) as Conn)
            })
        });
        (Arc::new(Tunnel::new(dial)), dialed)
    }

    /// 收到什么就原样发回去。
    async fn echo_server() -> SocketAddr {
        let listener = LocalListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                tokio::spawn(async move {
                    let (mut reader, mut writer) = stream.split();
                    let _ = tokio::io::copy(&mut reader, &mut writer).await;
                });
            }
        });
        addr
    }

    /// 说一句话就挂断。
    async fn greeting_server(greeting: &'static [u8]) -> SocketAddr {
        let listener = LocalListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                let _ = stream.write_all(greeting).await;
            }
        });
        addr
    }

    const DNS_PORT: u16 = 53;

    /// TCP 上的 DNS 服务器：应答是把查询标成应答，再补到至少 `size` 字节。返回地址和被问的次数。
    async fn dns_server(size: usize) -> (SocketAddr, Arc<std::sync::atomic::AtomicUsize>) {
        let listener = LocalListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let asked = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let count = asked.clone();
        tokio::spawn(async move {
            while let Ok((mut stream, _)) = listener.accept().await {
                count.fetch_add(1, Ordering::Relaxed);
                tokio::spawn(async move {
                    let Ok(mut answer) = read_frame(&mut stream).await else { return };
                    answer[2] |= 0x80;
                    answer.resize(answer.len().max(size), 0xab);
                    let _ = FrameWriter::new(&mut stream).send(&answer).await;
                });
            }
        });
        (addr, asked)
    }

    fn dialed(log: &Dialed) -> Vec<(String, u16)> {
        log.lock().clone()
    }

    #[tokio::test]
    async fn a_connection_is_carried_end_to_end() {
        let echo = echo_server().await;
        let (tunnel, log) = tunnel(&[(7, echo)]);
        let mut host = Host::start(tunnel.clone());

        let conn = host.open("93.184.216.34", 7).await;
        host.send(conn, b"hello").await;
        assert_eq!(host.recv(conn, 5).await, b"hello");
        // 转发的目标是应用要连的那个地址，不是隧道自己的什么地址。
        assert_eq!(dialed(&log), [("93.184.216.34".to_string(), 7)]);
        assert_eq!(tunnel.connections.load(Ordering::Relaxed), 1);

        // 大量数据，两个方向同时走：快慢由两头的窗口互相约束，不丢、不乱。
        let data: Vec<u8> = (0..3 * 1024 * 1024).map(|i| (i % 251) as u8).collect();
        let (mut sent, mut received) = (0, Vec::with_capacity(data.len()));
        host.until("the bulk data comes back", |host| {
            let socket = host.tcp(conn);
            if sent < data.len() {
                sent += socket.send_slice(&data[sent..]).unwrap();
            }
            while socket.can_recv() {
                socket.recv(|chunk| (chunk.len(), received.extend_from_slice(chunk))).unwrap();
            }
            received.len() == data.len()
        })
        .await;
        assert!(received == data, "the data came back changed");
        let total = (data.len() + 5) as u64;
        assert_eq!((tunnel.up.load(Ordering::Relaxed), tunnel.down.load(Ordering::Relaxed)), (total, total));

        // 应用关掉连接：对面读到结束，随后整条连接收尾。
        host.tcp(conn).close();
        host.until("the connection winds down", |host| {
            matches!(host.tcp(conn).state(), tcp::State::TimeWait | tcp::State::Closed) && tunnel.connections.load(Ordering::Relaxed) == 0
        })
        .await;
    }

    #[tokio::test]
    async fn several_connections_run_side_by_side() {
        let echo = echo_server().await;
        let (tunnel, log) = tunnel(&[(7, echo)]);
        let mut host = Host::start(tunnel.clone());

        let mut conns = Vec::new();
        for i in 0..20 {
            conns.push(host.connect(&format!("10.1.{i}.9"), 7));
        }
        host.until("all connections open", |host| conns.iter().all(|conn| host.tcp(*conn).may_send())).await;
        for (i, conn) in conns.iter().enumerate() {
            host.send(*conn, format!("connection {i:02}").as_bytes()).await;
        }
        for (i, conn) in conns.iter().enumerate() {
            assert_eq!(host.recv(*conn, 13).await, format!("connection {i:02}").as_bytes());
        }
        assert_eq!(tunnel.connections.load(Ordering::Relaxed), 20);
        let mut targets: Vec<String> = dialed(&log).into_iter().map(|(host, _)| host).collect();
        targets.sort();
        targets.dedup();
        assert_eq!(targets.len(), 20, "each connection goes to its own destination");
    }

    #[tokio::test]
    async fn connections_beyond_the_limit_are_closed_without_being_forwarded() {
        let echo = echo_server().await;
        let (tunnel, log) = tunnel(&[(7, echo)]);
        let tunnel = Arc::new(Tunnel { max_connections: 2, ..Arc::try_unwrap(tunnel).ok().unwrap() });
        let mut host = Host::start(tunnel.clone());

        let first = host.open("10.1.0.1", 7).await;
        let _second = host.open("10.1.0.2", 7).await;
        host.until("both are being forwarded", |_| tunnel.connections.load(Ordering::Relaxed) == 2).await;

        // 名额用完了：第三条不去连，直接关掉。
        let third = host.connect("10.1.0.3", 7);
        host.until("the extra connection is closed", |host| matches!(host.tcp(third).state(), tcp::State::CloseWait | tcp::State::Closed)).await;
        assert_eq!(dialed(&log).len(), 2);

        // 有连接结束，名额就还回来了。
        host.tcp(first).close();
        host.until("a slot is free again", |_| tunnel.connections.load(Ordering::Relaxed) == 1).await;
        let fourth = host.open("10.1.0.4", 7).await;
        host.send(fourth, b"hello").await;
        assert_eq!(host.recv(fourth, 5).await, b"hello");
        assert_eq!(dialed(&log).len(), 3);
    }

    #[tokio::test]
    async fn the_other_side_hanging_up_reaches_the_application() {
        let greeter = greeting_server(b"bye").await;
        let (tunnel, _) = tunnel(&[(9, greeter)]);
        let mut host = Host::start(tunnel.clone());

        let conn = host.open("93.184.216.34", 9).await;
        assert_eq!(host.recv(conn, 3).await, b"bye");
        // 对面说完就挂了：应用这头读到结束，但还可以把自己的话说完。
        host.until("the end of the stream is seen", |host| host.tcp(conn).state() == tcp::State::CloseWait).await;
        host.tcp(conn).close();
        host.until("the connection is gone", |host| {
            host.tcp(conn).state() == tcp::State::Closed && tunnel.connections.load(Ordering::Relaxed) == 0
        })
        .await;
    }

    #[tokio::test]
    async fn a_destination_that_cannot_be_reached_ends_the_connection() {
        let (tunnel, log) = tunnel(&[]);
        let mut host = Host::start(tunnel.clone());

        let conn = host.connect("93.184.216.34", 81);
        // 应用不必等到超时：连接马上就被关掉了。
        host.until("the connection is closed", |host| matches!(host.tcp(conn).state(), tcp::State::CloseWait | tcp::State::Closed)).await;
        assert_eq!(dialed(&log), [("93.184.216.34".to_string(), 81)]);
        assert_eq!(tunnel.connections.load(Ordering::Relaxed), 0);
    }

    #[tokio::test]
    async fn dns_queries_are_passed_on_over_tcp_to_the_server_they_were_meant_for() {
        let (dns, asked) = dns_server(0).await;
        let (tunnel, log) = tunnel(&[(DNS_PORT, dns)]);
        let mut host = Host::start(tunnel);

        let question = query("example.com", 1, true);
        let (answer, from) = host.ask(("8.8.8.8".parse().unwrap(), DNS_PORT), &question).await;
        // 应答来自被问的那台服务器，内容是它给的；隧道没有换成别的服务器，也没有改动内容。
        assert_eq!(from.to_string(), "8.8.8.8:53");
        assert_eq!((answer[2] & 0x80, &answer[..2], &answer[3..]), (0x80, &question[..2], &question[3..]));
        assert_eq!(dialed(&log), [("8.8.8.8".to_string(), DNS_PORT)]);

        // 问 IPv6 地址的查询也一样照转：域名解析不归隧道管。
        let (answer, from) = host.ask(("119.29.29.29".parse().unwrap(), DNS_PORT), &query("example.com", 28, false)).await;
        assert_eq!((from.to_string().as_str(), answer[2] & 0x80), ("119.29.29.29:53", 0x80));
        assert_eq!(asked.load(Ordering::Relaxed), 2);
    }

    #[tokio::test]
    async fn long_dns_answers_are_fetched_again_over_tcp() {
        let (dns, _) = dns_server(2000).await;
        let (tunnel, log) = tunnel(&[(DNS_PORT, dns)]);
        let mut host = Host::start(tunnel);

        // UDP 里放不下：告诉客户端改用 TCP。
        let question = query("example.com", 16, false);
        let (answer, _) = host.ask(("8.8.8.8".parse().unwrap(), DNS_PORT), &question).await;
        assert_eq!(Some(answer), truncated_answer(&question));

        // 客户端于是用 TCP 来问同一台服务器：那是一条普通的连接。
        let conn = host.open("8.8.8.8", DNS_PORT).await;
        let mut message = (question.len() as u16).to_be_bytes().to_vec();
        message.extend_from_slice(&question);
        host.send(conn, &message).await;
        let full = host.recv(conn, 2 + 2000).await;
        assert_eq!(&full[..2], &2000u16.to_be_bytes());
        assert_eq!(dialed(&log), [("8.8.8.8".to_string(), DNS_PORT), ("8.8.8.8".to_string(), DNS_PORT)]);
    }

    #[tokio::test]
    async fn what_the_tunnel_cannot_carry_is_dropped_without_harm() {
        let echo = echo_server().await;
        let (tunnel, log) = tunnel(&[(7, echo)]);
        let mut host = Host::start(tunnel);

        // DNS 以外的 UDP（QUIC 之类）：没有回音，也不会被转发。
        host.udp_send(("93.184.216.34".parse().unwrap(), 443), b"not dns");
        host.idle(Duration::from_millis(200)).await;
        assert_eq!(dialed(&log), []);

        // 隧道照常工作。
        let conn = host.open("93.184.216.34", 7).await;
        host.send(conn, b"still here").await;
        assert_eq!(host.recv(conn, 10).await, b"still here");
    }

    /// 经真实的 sshd 转发。默认忽略；需要的环境变量见 `ssh.rs` 的测试。
    #[tokio::test]
    #[ignore]
    async fn connections_travel_through_a_real_ssh_server() {
        let session = crate::ssh::tests::session().await;
        assert!(session.peer.ip().is_loopback(), "the test server is on this machine");
        let slot = Arc::new(parking_lot::RwLock::new(Some(session.clone())));
        let tunnel = Arc::new(Tunnel::new(crate::tunnel::ssh_dialer(slot.clone())));
        let mut host = Host::start(tunnel.clone());

        // 目标是 sshd 那台机器上的回显服务：连接从 sshd 那里发起。
        let echo = echo_server().await;
        let conn = host.open("127.0.0.1", echo.port()).await;
        let data: Vec<u8> = (0..1024 * 1024).map(|i| (i % 249) as u8).collect();
        let (mut sent, mut received) = (0, Vec::with_capacity(data.len()));
        host.until("the data comes back through ssh", |host| {
            let socket = host.tcp(conn);
            if sent < data.len() {
                sent += socket.send_slice(&data[sent..]).unwrap();
            }
            while socket.can_recv() {
                socket.recv(|chunk| (chunk.len(), received.extend_from_slice(chunk))).unwrap();
            }
            received.len() == data.len()
        })
        .await;
        assert!(received == data);

        // 没有人在听的端口：sshd 连不上，应用的连接随即被关掉。
        let closed = |host: &mut Host, conn| matches!(host.tcp(conn).state(), tcp::State::CloseWait | tcp::State::Closed);
        let refused = host.connect("127.0.0.1", 1);
        host.until("the refused connection is closed", |host| closed(host, refused)).await;

        // SSH 连接断着的时候，新连接马上被关掉而不是挂着；接上之后恢复。
        *slot.write() = None;
        let orphan = host.connect("127.0.0.1", echo.port());
        host.until("the orphaned connection is closed", |host| closed(host, orphan)).await;
        *slot.write() = Some(session.clone());
        let again = host.open("127.0.0.1", echo.port()).await;
        host.send(again, b"back").await;
        assert_eq!(host.recv(again, 4).await, b"back");

        // 连接断开后看得出来：隧道靠这个发现该重连了。
        assert!(!session.handle.is_closed());
        session.handle.disconnect(russh::Disconnect::ByApplication, "", "en").await.unwrap();
        host.until("the lost connection is noticed", |_| session.handle.is_closed()).await;
    }

    #[tokio::test]
    async fn the_stack_stops_when_the_link_goes_away() {
        let (tunnel, _) = tunnel(&[]);
        let (ours, theirs) = tokio::io::duplex(64 * 1024);
        let running = tokio::spawn(async move {
            let (mut reader, writer) = tokio::io::split(theirs);
            run(&mut reader, &mut FrameWriter::new(writer), tunnel).await
        });
        drop(ours);
        let error = tokio::time::timeout(Duration::from_secs(5), running).await.unwrap().unwrap();
        assert_eq!(error.kind(), io::ErrorKind::UnexpectedEof);
    }
}
