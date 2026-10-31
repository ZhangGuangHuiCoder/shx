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
