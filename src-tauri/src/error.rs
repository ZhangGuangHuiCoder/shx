use serde::ser::SerializeMap;
use serde::{Serialize, Serializer};

use crate::i18n::tr;

pub type Result<T, E = AppError> = std::result::Result<T, E>;

#[derive(Debug, thiserror::Error)]
pub enum AppError {
    #[error("{0}")]
    Msg(String),
    #[error("{} ({algorithm}) {fingerprint}", tr("未知的主机密钥", "Unknown host key"))]
    HostKeyUnknown {
        fingerprint: String,
        algorithm: String,
    },
    #[error("{}", tr(
        "主机密钥与本地记录不一致：服务器可能已重装，也可能存在中间人攻击",
        "The host key does not match the saved one: the server may have been reinstalled, or this may be a man-in-the-middle attack"
    ))]
    HostKeyChanged {
        fingerprint: String,
        algorithm: String,
    },
    #[error("{}", tr("需要输入私钥口令", "The private key requires a passphrase"))]
    PassphraseRequired,
    #[error("{}", tr("认证失败，请检查用户名、密码或密钥", "Authentication failed; check the username, password or key"))]
    AuthFailed,
    /// agent 为空，前端会提议用 ssh-add 加载（见 `ssh::agent_add_keys`）。
    #[error("{}", tr("ssh-agent 中没有密钥", "ssh-agent holds no keys"))]
    AgentEmpty,
    #[error("{}", tr("会话不存在或已断开", "The session does not exist or is disconnected"))]
    NoSession,
    #[error("{}", tr("已取消", "Cancelled"))]
    Cancelled,
    #[error("SSH: {0}")]
    Ssh(#[from] russh::Error),
    #[error("SFTP: {0}")]
    Sftp(#[from] russh_sftp::client::error::Error),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{label}{0}", label = tr("密钥：", "Key: "))]
    Keys(#[from] russh::keys::Error),
}

impl AppError {
    pub fn msg(s: impl Into<String>) -> Self {
        AppError::Msg(s.into())
    }

    fn kind(&self) -> &'static str {
        match self {
            AppError::HostKeyUnknown { .. } => "hostKeyUnknown",
            AppError::HostKeyChanged { .. } => "hostKeyChanged",
            AppError::PassphraseRequired => "passphraseRequired",
            AppError::AuthFailed => "authFailed",
            AppError::AgentEmpty => "agentEmpty",
            AppError::NoSession => "noSession",
            AppError::Cancelled => "cancelled",
            _ => "error",
        }
    }
}

// 前端按 kind 分支，其余字段供再次询问用户时使用。
impl Serialize for AppError {
    fn serialize<S: Serializer>(&self, s: S) -> std::result::Result<S::Ok, S::Error> {
        let mut m = s.serialize_map(None)?;
        m.serialize_entry("kind", self.kind())?;
        m.serialize_entry("message", &self.to_string())?;
        match self {
            AppError::HostKeyUnknown {
                fingerprint,
                algorithm,
            }
            | AppError::HostKeyChanged {
                fingerprint,
                algorithm,
            } => {
                m.serialize_entry("fingerprint", fingerprint)?;
                m.serialize_entry("algorithm", algorithm)?;
            }
            _ => {}
        }
        m.end()
    }
}
