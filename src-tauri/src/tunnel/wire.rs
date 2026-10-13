//! 主进程和提权助手之间的约定。两边经一个本地连接对话，上面的每条消息都带两字节的长度前缀：
//! 先是主进程发给助手的 [`Setup`]，再是助手回复的 [`Ready`]，此后双向都是原始的 IP 包。
//! 主进程发来长度为零的一条，是请助手收尾：有的系统上这种连接没有“我说完了”的信号。

use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::route::Cidr;

/// 助手要做的全部事情。不需要特权就能定下来的，主进程都定好了；助手只负责照着做。
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Setup {
    pub mtu: u16,
    /// 进隧道的网段：每个加一条指向虚拟网卡的路由。
    pub routes: Vec<Cidr>,
}

/// 建好的虚拟网卡。
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Device {
    /// 给人看的名字。
    pub name: String,
    /// 系统给它的编号，路由表里用这个认网卡。
    pub index: u32,
    /// 没加上路由的网段：发往它们的流量不进隧道。
    pub skipped: Vec<Cidr>,
}

/// 助手准备好之后的回复。
#[derive(Serialize, Deserialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub enum Ready {
    Device(Device),
    Error(String),
}

pub async fn read_frame<R: AsyncRead + Unpin>(reader: &mut R) -> io::Result<Vec<u8>> {
    let len = usize::from(reader.read_u16().await?);
    let mut frame = vec![0u8; len];
    reader.read_exact(&mut frame).await?;
    Ok(frame)
}

/// 往连接上写消息的一头：长度和内容拼在一起写出。写到一半被打断（调用它的任务被取消）
/// 也不会把连接写乱：没写完的那条记在这里，下一次先把它写完。
pub struct FrameWriter<W> {
    writer: W,
    /// 正在写的那条消息（连同长度前缀），以及已经写出去多少。
    frame: Vec<u8>,
    written: usize,
}

impl<W: AsyncWrite + Unpin> FrameWriter<W> {
    pub fn new(writer: W) -> Self {
        Self { writer, frame: Vec::new(), written: 0 }
    }

    async fn finish(&mut self) -> io::Result<()> {
        while self.written < self.frame.len() {
            match self.writer.write(&self.frame[self.written..]).await? {
                0 => return Err(io::ErrorKind::WriteZero.into()),
                n => self.written += n,
            }
        }
        Ok(())
    }

    pub async fn send(&mut self, payload: &[u8]) -> io::Result<()> {
        let len = u16::try_from(payload.len()).map_err(|_| io::Error::new(io::ErrorKind::InvalidInput, "frame too long"))?;
        self.finish().await?;
        self.frame.clear();
        self.frame.extend_from_slice(&len.to_be_bytes());
        self.frame.extend_from_slice(payload);
        self.written = 0;
        self.finish().await
    }

    /// 请对面收尾（空的一条），并表示这边不再写了。
    pub async fn close(&mut self) -> io::Result<()> {
        self.send(&[]).await?;
        self.writer.shutdown().await
    }
}

pub async fn read_json<R: AsyncRead + Unpin, T: for<'de> Deserialize<'de>>(reader: &mut R) -> io::Result<T> {
    serde_json::from_slice(&read_frame(reader).await?).map_err(io::Error::other)
}

pub async fn write_json<W: AsyncWrite + Unpin, T: Serialize>(writer: &mut W, message: &T) -> io::Result<()> {
    let payload = serde_json::to_vec(message).map_err(io::Error::other)?;
    FrameWriter::new(writer).send(&payload).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nets(list: &[&str]) -> Vec<Cidr> {
        list.iter().map(|net| net.parse().unwrap()).collect()
    }

    #[tokio::test]
    async fn messages_and_packets_share_one_framing() {
        let (mut a, mut b) = tokio::io::duplex(64 * 1024);
        let setup = Setup { mtu: 9000, routes: nets(&["0.0.0.0/1", "128.0.0.0/1"]) };
        let device = Device { name: "utun7".into(), index: 23, skipped: nets(&["10.0.0.0/8"]) };
        write_json(&mut a, &setup).await.unwrap();
        write_json(&mut a, &Ready::Device(device.clone())).await.unwrap();
        let mut frames = FrameWriter::new(&mut a);
        frames.send(&[0x45, 0, 0, 20]).await.unwrap();
        frames.send(&[]).await.unwrap();

        assert_eq!(read_json::<_, Setup>(&mut b).await.unwrap(), setup);
        assert_eq!(read_json::<_, Ready>(&mut b).await.unwrap(), Ready::Device(device));
        assert_eq!(read_frame(&mut b).await.unwrap(), [0x45, 0, 0, 20]);
        assert_eq!(read_frame(&mut b).await.unwrap(), [0u8; 0]);

        // 对端在一条消息中途断开：读到的是错误，不是半条消息。
        tokio::io::AsyncWriteExt::write_all(&mut a, &[0, 9, 1, 2]).await.unwrap();
        drop(a);
        assert_eq!(read_frame(&mut b).await.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);

        // 超过长度前缀能表示的范围：拒绝，而不是悄悄截断。
        let (mut c, _d) = tokio::io::duplex(16);
        assert!(FrameWriter::new(&mut c).send(&vec![0u8; 70_000]).await.is_err());
    }

    /// 一次只收得下几个字节、可以数着放行的写入端：模拟写到一半被打断。
    struct Trickle {
        written: Vec<u8>,
        /// 还放行多少次写入；用完之后写入一直挂起。
        allowed: usize,
    }

    impl AsyncWrite for Trickle {
        fn poll_write(mut self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>, buf: &[u8]) -> std::task::Poll<io::Result<usize>> {
            if self.allowed == 0 {
                return std::task::Poll::Pending;
            }
            self.allowed -= 1;
            let n = buf.len().min(3);
            self.written.extend_from_slice(&buf[..n]);
            std::task::Poll::Ready(Ok(n))
        }

        fn poll_flush(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }

        fn poll_shutdown(self: std::pin::Pin<&mut Self>, _: &mut std::task::Context<'_>) -> std::task::Poll<io::Result<()>> {
            std::task::Poll::Ready(Ok(()))
        }
    }

    #[tokio::test]
    async fn an_interrupted_message_is_finished_before_the_next_one() {
        let mut frames = FrameWriter::new(Trickle { written: Vec::new(), allowed: 2 });
        // 写了两次（六个字节）之后卡住，写它的任务被取消。
        let interrupted = tokio::time::timeout(std::time::Duration::from_millis(20), frames.send(&[1, 2, 3, 4, 5, 6, 7, 8])).await;
        assert!(interrupted.is_err());
        assert_eq!(frames.writer.written, [0, 8, 1, 2, 3, 4]);

        // 连接通了之后，先把那条写完，再写后面的：对面读到的仍是完整的一条条消息。
        frames.writer.allowed = usize::MAX;
        frames.close().await.unwrap();
        let mut reader = &frames.writer.written[..];
        assert_eq!(read_frame(&mut reader).await.unwrap(), [1, 2, 3, 4, 5, 6, 7, 8]);
        assert_eq!(read_frame(&mut reader).await.unwrap(), [0u8; 0]);
        assert!(reader.is_empty());
    }

    #[test]
    fn the_wire_format_is_what_both_sides_expect() {
        assert_eq!(serde_json::to_string(&Ready::Error("no".into())).unwrap(), r#"{"error":"no"}"#);
        let device = Ready::Device(Device { name: "shx".into(), index: 23, skipped: nets(&["10.9.0.0/16"]) });
        assert_eq!(serde_json::to_string(&device).unwrap(), r#"{"device":{"name":"shx","index":23,"skipped":["10.9.0.0/16"]}}"#);
        let setup = Setup { mtu: 9000, routes: nets(&["10.0.0.0/8"]) };
        assert_eq!(serde_json::to_string(&setup).unwrap(), r#"{"mtu":9000,"routes":["10.0.0.0/8"]}"#);
    }
}
