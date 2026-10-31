//! 用户态协议栈：把 IP 包还原为 TCP 连接并逐条转发。
//!
//! 连接在本机终结，转发的是数据而非 IP 包，避免 TCP-over-TCP；代价是只支持 TCP。
//! 唯一处理的 UDP 是 DNS：改用 TCP 转发给原服务器，应答超出 UDP 限制时返回截断标志让客户端改用 TCP。

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
/// 并发 DNS 查询上限，超出的丢弃（客户端会重发）。
const MAX_QUERIES: usize = 128;
/// 并发连接上限，超出的直接关闭；防止端口扫描在 SSH 上开出大量通道。
const MAX_CONNECTIONS: usize = 1024;
/// 连接最短存活时间：握手完成前关闭会导致应用收不到结束标志而挂起。
const HANDSHAKE_GRACE: Duration = Duration::from_millis(200);
const COPY_BUFFER: usize = 32 * 1024;
const READ_BUFFER: usize = 256 * 1024;

pub trait Duplex: AsyncRead + AsyncWrite + Send + Unpin {}
impl<T: AsyncRead + AsyncWrite + Send + Unpin> Duplex for T {}
pub type Conn = Box<dyn Duplex>;

/// 连接 主机:端口，最后一个参数为本机发起方。
pub type Dialer = Arc<dyn Fn(String, u16, SocketAddr) -> BoxFuture<'static, io::Result<Conn>> + Send + Sync>;

pub struct Tunnel {
    dial: Dialer,
    /// 上行字节数。
    pub up: AtomicU64,
    /// 下行字节数。
    pub down: AtomicU64,
    pub connections: AtomicU64,
    max_connections: usize,
}

impl Tunnel {
    pub fn new(dial: Dialer) -> Self {
        Self { dial, up: AtomicU64::new(0), down: AtomicU64::new(0), connections: AtomicU64::new(0), max_connections: MAX_CONNECTIONS }
    }
}

/// 运行协议栈直到链路断开，返回断开原因。
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

    // tasks 被丢弃时所有任务随之终止。
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
            // 丢弃不支持的包。
            if !carried(&packet) {
                continue;
            }
            // 只有协议栈停止才算出错。
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

// ---- 包过滤 ----

pub(super) const DNS_PORT: u16 = 53;

/// 只接受 IPv4 TCP 和 DNS 查询。
pub fn carried(packet: &[u8]) -> bool {
    if packet.first().map(|byte| byte >> 4) != Some(4) {
        return false;
    }
    let Ok(ip) = Ipv4Packet::new_checked(packet) else {
        return false;
    };
    match ip.next_header() {
        IpProtocol::Tcp => true,
        // 残缺的 UDP 包会让协议栈停止接收，需预先校验。
        IpProtocol::Udp => UdpPacket::new_checked(ip.payload()).is_ok_and(|udp| udp.dst_port() == DNS_PORT),
        _ => false,
    }
}

// ---- TCP 转发 ----

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

/// 连接计数，结束时自动递减。
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

/// 转发一条连接直到结束；连接失败时交由协议栈关闭。permit 为并发名额（见 [`MAX_CONNECTIONS`]）。
async fn forward(tunnel: Arc<Tunnel>, mut local: TcpStream, from: SocketAddr, to: SocketAddr, permit: Option<OwnedSemaphorePermit>) {
    let ready = tokio::time::Instant::now() + HANDSHAKE_GRACE;
    if permit.is_some() {
        carry(&tunnel, &mut local, from, to, ready).await;
    }
    tokio::time::sleep_until(ready).await;
}

/// ready 之前不能关闭 local（见 [`HANDSHAKE_GRACE`]）。
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
        // 不等待 shutdown 返回：协议栈要到连接彻底关闭才完成。
        poll_fn(|cx| {
            let _ = Pin::new(&mut local_tx).poll_shutdown(cx);
            Poll::Ready(())
        })
        .await;
        Ok(())
    };
    // 支持半关闭；任一方向出错则整条连接结束。
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

// ---- DNS 转发 ----

/// 此处的 UDP 包均为 DNS 查询。
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
                        // 以原服务器地址回复。
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

/// 经 TCP 转发 DNS 查询；失败时返回 None，由客户端重试。
async fn ask(tunnel: &Tunnel, query: &[u8], from: SocketAddr, to: SocketAddr) -> Option<Vec<u8>> {
    let exchange = async {
        let mut conn = (tunnel.dial)(to.ip().to_string(), to.port(), from).await.ok()?;
        // TCP DNS 报文带两字节长度前缀。
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
    // 超出 UDP 限制时返回截断应答，客户端改用 TCP 后按普通连接转发。
    if answer.len() > udp_limit(query) {
        return truncated_answer(query);
    }
    Some(answer)
}

const DNS_HEADER: usize = 12;
/// 无 EDNS 时的 UDP 应答上限（RFC 1035）。
const PLAIN_LIMIT: usize = 512;
/// EDNS 时采用的通用安全大小。
const EDNS_LIMIT: usize = 1232;

/// 问题段结束位置；非单问题查询返回 None。
fn question_end(query: &[u8]) -> Option<usize> {
    let header = query.get(..DNS_HEADER)?;
    let is_response = header[2] & 0x80 != 0;
    let questions = u16::from_be_bytes([header[4], header[5]]);
    if is_response || questions != 1 {
        return None;
    }
    // 域名为长度前缀标签序列，以 0 结尾，不压缩。
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
    // 之后为类型和类别各两字节。
    query.get(at..at + 4)?;
    Some(at + 4)
}

/// 截断应答：保留编号和问题，不含记录，客户端会改用 TCP 重试。
fn truncated_answer(query: &[u8]) -> Option<Vec<u8>> {
    let mut answer = query[..question_end(query)?].to_vec();
    // QR=1，TC=1；保留 OPCODE 和 RD。
    answer[2] = 0x80 | 0x02 | (query[2] & 0x79);
    // RA=1，RCODE=0。
    answer[3] = 0x80;
    // 三段记录均为空。
    answer[6..DNS_HEADER].fill(0);
    Some(answer)
}

/// 该查询允许的 UDP 应答长度。
fn udp_limit(query: &[u8]) -> usize {
    let additional = query.get(10..DNS_HEADER).map_or(0, |n| u16::from_be_bytes([n[0], n[1]]));
    if additional == 0 { PLAIN_LIMIT } else { EDNS_LIMIT }
}
