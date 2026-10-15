//! 主进程与助手的协议：消息带两字节长度前缀。
//! 先 [`Setup`] 再 [`Ready`]，之后双向传输 IP 包；空消息表示请求收尾（部分平台连接无半关闭信号）。

use std::io;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};

use super::route::Cidr;

/// 助手要执行的清单，由主进程预先确定。
#[derive(Serialize, Deserialize, Debug, Default, PartialEq)]
pub struct Setup {
    pub mtu: u16,
    /// 需加路由的网段。
    pub routes: Vec<Cidr>,
}

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq)]
pub struct Device {
    /// 显示名。
    pub name: String,
    /// 系统接口编号，路由表据此识别网卡。
    pub index: u32,
    /// 未能添加路由的网段。
    pub skipped: Vec<Cidr>,
}

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

/// 消息写入端。写到一半被取消时记下未写完的部分，下次先写完，保证消息完整。
pub struct FrameWriter<W> {
    writer: W,
    /// 待写消息（含长度前缀）及已写字节数。
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

    /// 发送空消息请求收尾。
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

        // 消息中途断开应报错，而非返回半条。
        tokio::io::AsyncWriteExt::write_all(&mut a, &[0, 9, 1, 2]).await.unwrap();
        drop(a);
        assert_eq!(read_frame(&mut b).await.unwrap_err().kind(), io::ErrorKind::UnexpectedEof);

        // 超出长度前缀范围时拒绝，不截断。
        let (mut c, _d) = tokio::io::duplex(16);
        assert!(FrameWriter::new(&mut c).send(&vec![0u8; 70_000]).await.is_err());
    }

    /// 限量放行的写入端，模拟写入中断。
    struct Trickle {
        written: Vec<u8>,
        /// 剩余放行次数，用完后挂起。
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
        // 写入两次后卡住并取消任务。
        let interrupted = tokio::time::timeout(std::time::Duration::from_millis(20), frames.send(&[1, 2, 3, 4, 5, 6, 7, 8])).await;
        assert!(interrupted.is_err());
        assert_eq!(frames.writer.written, [0, 8, 1, 2, 3, 4]);

        // 恢复后先补完上一条，对端读到的消息仍完整。
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
