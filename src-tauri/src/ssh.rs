use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;

use parking_lot::{Mutex, RwLock};
use russh::client::{self, KeyboardInteractiveAuthResponse};
use russh::keys::agent::client::{AgentClient, AgentStream};
use russh::keys::{self, HashAlg, PrivateKeyWithHashAlg, PublicKeyOrCertificate};
use russh::{ChannelMsg, Disconnect};
use russh_sftp::client::SftpSession;
use serde::{Deserialize, Serialize};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State};
use tokio::sync::{mpsc, OnceCell};

use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::transfer;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(15);
/// ssh-agent 响应超时，避免卡住连接。
const AGENT_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_KEY_SIZE: u64 = 64 * 1024;
/// 终端输出合批发送的时间窗口。
const TERM_FLUSH: Duration = Duration::from_millis(4);
const TERM_FLUSH_BYTES: usize = 64 * 1024;

enum HostKeyIssue {
    Unknown { fingerprint: String, algorithm: String },
    Changed { fingerprint: String, algorithm: String },
}

impl From<HostKeyIssue> for AppError {
    fn from(issue: HostKeyIssue) -> Self {
        match issue {
            HostKeyIssue::Unknown {
                fingerprint,
                algorithm,
            } => AppError::HostKeyUnknown {
                fingerprint,
                algorithm,
            },
            HostKeyIssue::Changed {
                fingerprint,
                algorithm,
            } => AppError::HostKeyChanged {
                fingerprint,
                algorithm,
            },
        }
    }
}

/// 已信任的主机密钥：算法 → SHA-256 指纹，由前端保管。
pub type KnownHostKeys = HashMap<String, String>;

pub struct ClientHandler {
    known: KnownHostKeys,
    issue: Arc<Mutex<Option<HostKeyIssue>>>,
}

impl client::Handler for ClientHandler {
    type Error = AppError;

    async fn check_server_key(&mut self, key: &PublicKeyOrCertificate) -> Result<bool> {
        let key = key.public_key();
        let algorithm = key.algorithm().as_str().to_string();
        let fingerprint = key.fingerprint(HashAlg::Sha256).to_string();
        let issue = match self.known.get(&algorithm) {
            Some(known) if *known == fingerprint => return Ok(true),
            Some(_) => HostKeyIssue::Changed {
                fingerprint,
                algorithm,
            },
            None => HostKeyIssue::Unknown {
                fingerprint,
                algorithm,
            },
        };
        *self.issue.lock() = Some(issue);
        Ok(false)
    }
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum Auth {
    Password {
        password: String,
    },
    Key {
        /// 私钥文本（OpenSSH、PEM 或 PPK），不是路径。
        content: String,
        passphrase: Option<String>,
    },
    Agent,
}

pub async fn connect(
    host: &str,
    port: u16,
    username: &str,
    auth: Auth,
    known: KnownHostKeys,
) -> Result<Session> {
    let config = Arc::new(client::Config {
        keepalive_interval: Some(Duration::from_secs(30)),
        keepalive_max: 3,
        nodelay: true,
        ..Default::default()
    });
    let issue = Arc::new(Mutex::new(None));
    let handler = ClientHandler {
        known,
        issue: issue.clone(),
    };

    // 自建 TCP 连接以获知实际连接的地址（主机名可能解析出多个）。
    let connecting = async {
        let socket = tokio::net::TcpStream::connect((host, port)).await?;
        let _ = socket.set_nodelay(true);
        let peer = socket.peer_addr()?;
        Ok::<_, AppError>((client::connect_stream(config, socket, handler).await?, peer))
    };
    let (mut handle, peer) = match tokio::time::timeout(CONNECT_TIMEOUT, connecting).await {
        Err(_) => return Err(AppError::msg(tr("连接超时", "Connection timed out"))),
        Ok(Err(e)) => {
            // 主机密钥被拒绝时表现为笼统的密钥交换失败。
            return Err(issue.lock().take().map(Into::into).unwrap_or(e));
        }
        Ok(Ok(connected)) => connected,
    };

    let ok = match auth {
        Auth::Password { password } => {
            if handle
                .authenticate_password(username, &password)
                .await?
                .success()
            {
                true
            } else {
                keyboard_interactive(&mut handle, username, &password).await?
            }
        }
        Auth::Key { content, passphrase } => {
            let passphrase = passphrase.filter(|p| !p.is_empty());
            let key = match keys::decode_secret_key(&content, passphrase.as_deref()) {
                Ok(key) => key,
                Err(keys::Error::KeyIsEncrypted) => return Err(AppError::PassphraseRequired),
                Err(e) if passphrase.is_some() => {
                    return Err(AppError::msg(format!(
                        "{}{e}",
                        tr("私钥口令错误或格式不支持：", "Wrong passphrase or unsupported key format: ")
                    )))
                }
                Err(e) => return Err(AppError::msg(format!("{}{e}", tr("无法解析私钥：", "Could not parse the private key: ")))),
            };
            let hash = handle.best_supported_rsa_hash().await?.flatten();
            handle
                .authenticate_publickey(username, PrivateKeyWithHashAlg::new(Arc::new(key), hash))
                .await?
                .success()
        }
        Auth::Agent => agent_auth(&mut handle, username).await?,
    };

    if !ok {
        let _ = handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
        return Err(AppError::AuthFailed);
    }
    Ok(Session::new(handle, peer))
}

/// 部分服务器只支持 keyboard-interactive，用密码回答所有提示。
async fn keyboard_interactive(
    handle: &mut client::Handle<ClientHandler>,
    username: &str,
    password: &str,
) -> Result<bool> {
    let mut reply = handle
        .authenticate_keyboard_interactive_start(username, None)
        .await?;
    for _ in 0..4 {
        match reply {
            KeyboardInteractiveAuthResponse::Success => return Ok(true),
            KeyboardInteractiveAuthResponse::Failure { .. } => return Ok(false),
            KeyboardInteractiveAuthResponse::InfoRequest { prompts, .. } => {
                let answers = prompts.iter().map(|_| password.to_string()).collect();
                reply = handle
                    .authenticate_keyboard_interactive_respond(answers)
                    .await?;
            }
        }
    }
    Ok(false)
}

type Agent = AgentClient<Box<dyn AgentStream + Send + Unpin>>;

#[cfg(unix)]
async fn connect_agent() -> std::result::Result<Agent, keys::Error> {
    Ok(AgentClient::connect_env().await?.dynamic())
}

/// Windows：先连 OpenSSH agent 的命名管道，失败再试 Pageant。
#[cfg(windows)]
async fn connect_agent() -> std::result::Result<Agent, keys::Error> {
    let pipe = std::env::var("SSH_AUTH_SOCK").unwrap_or_else(|_| r"\\.\pipe\openssh-ssh-agent".to_string());
    match AgentClient::connect_named_pipe(&pipe).await {
        Ok(agent) => Ok(agent.dynamic()),
        Err(_) => Ok(AgentClient::connect_pageant().await?.dynamic()),
    }
}

async fn agent_auth(handle: &mut client::Handle<ClientHandler>, username: &str) -> Result<bool> {
    let connecting = tokio::time::timeout(AGENT_TIMEOUT, connect_agent()).await.map_err(|_| keys::Error::AgentFailure);
    let mut agent = connecting
        .and_then(|connected| connected)
        .map_err(|e| AppError::msg(format!("{}{e}", tr("无法连接 ssh-agent：", "Could not connect to ssh-agent: "))))?;
    let identities = tokio::time::timeout(AGENT_TIMEOUT, agent.request_identities())
        .await
        .map_err(|_| AppError::msg(tr("ssh-agent 无响应", "ssh-agent did not respond")))??;
    // ssh 命令可能直接读取 ~/.ssh 私钥而不经 agent；本程序不读私钥文件，由前端提议 ssh-add。
    if identities.is_empty() {
        return Err(AppError::AgentEmpty);
    }
    let hash = handle.best_supported_rsa_hash().await?.flatten();
    for identity in identities {
        let key = identity.public_key().into_owned();
        let result = handle
            .authenticate_publickey_with(username, key, hash, &mut agent)
            .await;
        if matches!(result, Ok(r) if r.success()) {
            return Ok(true);
        }
    }
    Ok(false)
}

#[cfg(unix)]
const SSH_ADD: &str = "/usr/bin/ssh-add";
#[cfg(windows)]
const SSH_ADD: &str = "ssh-add";

/// 调用 ssh-add 加载默认私钥，返回 agent 中的密钥数。带口令的私钥无法加载；仍为空时报错。
#[tauri::command]
pub async fn agent_add_keys() -> Result<usize> {
    let run = tokio::task::spawn_blocking(|| {
        let mut command = std::process::Command::new(SSH_ADD);
        // macOS：同时存入钥匙串。
        #[cfg(target_os = "macos")]
        command.arg("--apple-use-keychain");
        // 无法询问口令，不弹出外部口令框。
        command.env("SSH_ASKPASS_REQUIRE", "never").stdin(std::process::Stdio::null()).output()
    });
    let output = run
        .await
        .map_err(|e| AppError::msg(e.to_string()))?
        .map_err(|e| AppError::msg(format!("{}{e}", tr("无法运行 ssh-add：", "Could not run ssh-add: "))))?;

    let count = match tokio::time::timeout(AGENT_TIMEOUT, async { connect_agent().await?.request_identities().await }).await {
        Ok(Ok(identities)) => identities.len(),
        _ => 0,
    };
    if count > 0 {
        return Ok(count);
    }
    let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let hint = tr(
        "没能加入任何密钥。设有口令的私钥请在终端运行 ssh-add，或在主机设置中改用私钥认证",
        "No keys were added. For a key with a passphrase, run ssh-add in a terminal, or switch this host to private key authentication",
    );
    Err(AppError::msg(if detail.is_empty() { hint.to_string() } else { format!("{hint}{}{detail}", tr("（", " (")) + tr("）", ")") }))
}

pub struct ExecOutput {
    pub stdout: String,
    pub stderr: String,
    pub code: u32,
}

impl ExecOutput {
    /// 退出码非零时返回含 stderr 的错误。
    pub fn check(self) -> Result<String> {
        if self.code == 0 {
            return Ok(self.stdout);
        }
        let detail = self.stderr.trim();
        Err(AppError::msg(if detail.is_empty() {
            format!("{}{}{}", tr("命令执行失败（退出码 ", "Command failed (exit code "), self.code, tr("）", ")"))
        } else {
            detail.to_string()
        }))
    }
}

/// POSIX shell 单引号转义。
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

enum TermCmd {
    Write(Vec<u8>),
    Resize(u32, u32),
}

/// uid/gid → 名称，从远程 /etc/passwd、/etc/group 读取一次。
#[derive(Default)]
pub struct Names {
    pub users: HashMap<u32, String>,
    pub groups: HashMap<u32, String>,
}

pub struct Session {
    pub handle: client::Handle<ClientHandler>,
    pub peer: SocketAddr,
    sftp: OnceCell<Arc<SftpSession>>,
    pub names: OnceCell<Names>,
    term: Mutex<Option<mpsc::UnboundedSender<TermCmd>>>,
}

impl Session {
    fn new(handle: client::Handle<ClientHandler>, peer: SocketAddr) -> Self {
        Self {
            handle,
            peer,
            sftp: OnceCell::new(),
            names: OnceCell::new(),
            term: Mutex::new(None),
        }
    }

    /// 新开 SFTP 通道，传输各用独立通道以免阻塞浏览。
    pub async fn open_sftp(&self) -> Result<SftpSession> {
        let channel = self.handle.channel_open_session().await?;
        channel.request_subsystem(true, "sftp").await?;
        Ok(SftpSession::new(channel.into_stream()).await?)
    }

    pub async fn sftp(&self) -> Result<Arc<SftpSession>> {
        self.sftp
            .get_or_try_init(|| async { self.open_sftp().await.map(Arc::new) })
            .await
            .cloned()
    }

    pub async fn exec(&self, command: &str) -> Result<ExecOutput> {
        let mut channel = self.handle.channel_open_session().await?;
        channel.exec(true, command).await?;
        let (mut stdout, mut stderr, mut code) = (Vec::new(), Vec::new(), 0);
        while let Some(msg) = channel.wait().await {
            match msg {
                ChannelMsg::Data { data } => stdout.extend_from_slice(&data),
                ChannelMsg::ExtendedData { data, .. } => stderr.extend_from_slice(&data),
                ChannelMsg::ExitStatus { exit_status } => code = exit_status,
                ChannelMsg::ExitSignal { .. } => code = 255,
                _ => {}
            }
        }
        Ok(ExecOutput {
            stdout: String::from_utf8_lossy(&stdout).into_owned(),
            stderr: String::from_utf8_lossy(&stderr).into_owned(),
            code,
        })
    }
}

#[derive(Default)]
pub struct AppState {
    sessions: RwLock<HashMap<String, Arc<Session>>>,
    pub transfers: transfer::Manager,
    pub tunnel: crate::tunnel::Manager,
}

impl AppState {
    pub fn session(&self, id: &str) -> Result<Arc<Session>> {
        self.sessions
            .read()
            .get(id)
            .cloned()
            .ok_or(AppError::NoSession)
    }
}

#[derive(Clone, Serialize)]
struct SessionClosed {
    id: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    host: String,
    port: u16,
    username: String,
    auth: Auth,
    #[serde(default)]
    known_host_keys: KnownHostKeys,
}

/// 建立连接并返回会话 id；后端不保存配置、凭据和主机密钥。
#[tauri::command]
pub async fn ssh_connect(state: State<'_, AppState>, request: ConnectRequest) -> Result<String> {
    let ConnectRequest {
        host,
        port,
        username,
        auth,
        known_host_keys,
    } = request;
    let session = connect(&host, port, &username, auth, known_host_keys).await?;
    let id = uuid::Uuid::new_v4().to_string();
    state.sessions.write().insert(id.clone(), Arc::new(session));
    Ok(id)
}

#[derive(Debug, Serialize)]
pub struct ImportedKey {
    name: String,
    content: String,
    encrypted: bool,
}

fn inspect_key(name: String, content: String) -> Result<ImportedKey> {
    let encrypted = match keys::decode_secret_key(&content, None) {
        Ok(_) => false,
        Err(keys::Error::KeyIsEncrypted) => true,
        Err(e) => return Err(AppError::msg(format!("{}{e}", tr("不是有效的私钥：", "Not a valid private key: ")))),
    };
    Ok(ImportedKey {
        name,
        content,
        encrypted,
    })
}

/// 校验私钥并返回其内容供前端保存。
#[tauri::command]
pub fn key_import(name: Option<String>, path: Option<String>, content: Option<String>) -> Result<ImportedKey> {
    let (default_name, content) = match (path, content) {
        (Some(path), _) => {
            let path = std::path::Path::new(&path);
            if std::fs::metadata(path)?.len() > MAX_KEY_SIZE {
                return Err(AppError::msg(tr("文件过大，不是私钥文件", "The file is too large to be a private key")));
            }
            let file_name = path.file_name().unwrap_or_default().to_string_lossy();
            (file_name.into_owned(), std::fs::read_to_string(path)?)
        }
        (None, Some(content)) => (tr("私钥", "Private key").to_string(), content),
        (None, None) => return Err(AppError::msg(tr("没有提供私钥", "No private key was provided"))),
    };
    let name = name.map(|n| n.trim().to_string()).filter(|n| !n.is_empty());
    inspect_key(name.unwrap_or(default_name), content)
}

#[tauri::command]
pub async fn ssh_disconnect(state: State<'_, AppState>, id: String) -> Result<()> {
    let session = state.sessions.write().remove(&id);
    if let Some(session) = session {
        session.term.lock().take();
        let _ = session
            .handle
            .disconnect(Disconnect::ByApplication, "", "en")
            .await;
    }
    Ok(())
}

/// 启动 shell（指定 keep_alive 时接入 tmux 会话）。输出经 on_data 发送，结束时触发 `session-closed`。
#[tauri::command]
pub async fn term_open(
    app: AppHandle,
    state: State<'_, AppState>,
    id: String,
    cols: u32,
    rows: u32,
    on_data: Channel<InvokeResponseBody>,
    keep_alive: Option<String>,
) -> Result<()> {
    let session = state.session(&id)?;
    let attach = keep_alive.as_deref().map(crate::keepalive::attach_command).transpose()?;
    let mut channel = session.handle.channel_open_session().await?;
    channel
        .request_pty(false, "xterm-256color", cols, rows, 0, 0, &[])
        .await?;
    match attach {
        Some(command) => channel.exec(true, command).await?,
        None => channel.request_shell(true).await?,
    }

    let (tx, mut rx) = mpsc::unbounded_channel();
    *session.term.lock() = Some(tx);

    tokio::spawn(async move {
        let mut pending: Vec<u8> = Vec::new();
        let flush = tokio::time::sleep(TERM_FLUSH);
        tokio::pin!(flush);
        loop {
            tokio::select! {
                cmd = rx.recv() => match cmd {
                    Some(TermCmd::Write(bytes)) => {
                        if channel.data_bytes(bytes).await.is_err() { break; }
                    }
                    Some(TermCmd::Resize(cols, rows)) => {
                        let _ = channel.window_change(cols, rows, 0, 0).await;
                    }
                    None => break,
                },
                msg = channel.wait() => match msg {
                    Some(ChannelMsg::Data { data }) | Some(ChannelMsg::ExtendedData { data, .. }) => {
                        if pending.is_empty() {
                            flush.as_mut().reset(tokio::time::Instant::now() + TERM_FLUSH);
                        }
                        pending.extend_from_slice(&data);
                        if pending.len() >= TERM_FLUSH_BYTES {
                            let _ = on_data.send(InvokeResponseBody::Raw(std::mem::take(&mut pending)));
                        }
                    }
                    Some(ChannelMsg::Close) | None => break,
                    _ => {}
                },
                _ = &mut flush, if !pending.is_empty() => {
                    let _ = on_data.send(InvokeResponseBody::Raw(std::mem::take(&mut pending)));
                }
            }
        }
        if !pending.is_empty() {
            let _ = on_data.send(InvokeResponseBody::Raw(pending));
        }
        let _ = channel.close().await;
        let _ = app.emit("session-closed", SessionClosed { id });
    });
    Ok(())
}

fn term_send(state: &AppState, id: &str, cmd: TermCmd) -> Result<()> {
    let session = state.session(id)?;
    let term = session.term.lock();
    term.as_ref()
        .ok_or(AppError::NoSession)?
        .send(cmd)
        .map_err(|_| AppError::NoSession)
}

#[tauri::command]
pub fn term_write(state: State<'_, AppState>, id: String, data: String) -> Result<()> {
    term_send(&state, &id, TermCmd::Write(data.into_bytes()))
}

#[tauri::command]
pub fn term_resize(state: State<'_, AppState>, id: String, cols: u32, rows: u32) -> Result<()> {
    term_send(&state, &id, TermCmd::Resize(cols, rows))
}

/// 真实 sshd 的端到端测试，默认忽略；需设置 SHX_TEST_PORT/USER/KEY/DIR。
#[cfg(test)]
pub mod tests {
    use super::*;

    pub fn env(name: &str) -> String {
        std::env::var(name).unwrap_or_else(|_| panic!("{name} is not set"))
    }

    fn key(suffix: &str, passphrase: Option<&str>) -> Auth {
        Auth::Key {
            content: std::fs::read_to_string(format!("{}{suffix}", env("SHX_TEST_KEY"))).unwrap(),
            passphrase: passphrase.map(Into::into),
        }
    }

    async fn dial(auth: Auth, known: &KnownHostKeys) -> Result<Session> {
        let port = env("SHX_TEST_PORT").parse().unwrap();
        connect("127.0.0.1", port, &env("SHX_TEST_USER"), auth, known.clone()).await
    }

    async fn trusted() -> KnownHostKeys {
        match dial(key("", None), &KnownHostKeys::new()).await {
            Err(AppError::HostKeyUnknown {
                fingerprint,
                algorithm,
            }) => KnownHostKeys::from([(algorithm, fingerprint)]),
            _ => panic!("an unknown host key must be rejected"),
        }
    }

    pub async fn session() -> Arc<Session> {
        Arc::new(dial(key("", None), &trusted().await).await.unwrap())
    }

    #[tokio::test]
    #[ignore]
    async fn host_key_must_be_known_to_connect() {
        let known = trusted().await;
        let (algorithm, fingerprint) = known.iter().next().unwrap();
        assert!(fingerprint.starts_with("SHA256:"));
        dial(key("", None), &known).await.unwrap();

        // 已知的是另一类型的密钥，不能作保。
        let other = KnownHostKeys::from([("ssh-rsa".to_string(), fingerprint.clone())]);
        assert!(matches!(
            dial(key("", None), &other).await,
            Err(AppError::HostKeyUnknown { .. })
        ));

        // 同类型不同密钥报告为已变更。
        let stale = KnownHostKeys::from([(algorithm.clone(), "SHA256:someone-else".to_string())]);
        assert!(matches!(
            dial(key("", None), &stale).await,
            Err(AppError::HostKeyChanged { fingerprint: f, algorithm: a }) if f == *fingerprint && a == *algorithm
        ));
    }

    #[tokio::test]
    #[ignore]
    async fn encrypted_key_asks_for_passphrase() {
        let known = trusted().await;
        for missing in [None, Some("")] {
            assert!(matches!(
                dial(key("_enc", missing), &known).await,
                Err(AppError::PassphraseRequired)
            ));
        }
        assert!(matches!(
            dial(key("_enc", Some("nope")), &known).await,
            Err(AppError::Msg(_))
        ));
        dial(key("_enc", Some("secret")), &known).await.unwrap();
    }

    #[tokio::test]
    #[ignore]
    async fn import_reports_whether_a_key_is_encrypted() {
        let path = env("SHX_TEST_KEY");
        let plain = key_import(None, Some(path.clone()), None).unwrap();
        assert_eq!((plain.name.as_str(), plain.encrypted), ("client", false));
        assert_eq!(plain.content, std::fs::read_to_string(&path).unwrap());

        let locked = key_import(Some("  mine ".into()), Some(format!("{path}_enc")), None).unwrap();
        assert_eq!((locked.name.as_str(), locked.encrypted), ("mine", true));

        let pasted = key_import(None, None, Some(plain.content.clone())).unwrap();
        assert_eq!((pasted.name.as_str(), pasted.encrypted), ("私钥", false));
    }

    #[test]
    fn import_rejects_what_is_not_a_private_key() {
        assert!(key_import(None, None, Some("ssh-ed25519 AAAAC3Nza public-key".into())).is_err());
        assert!(key_import(None, None, Some(String::new())).is_err());
        assert!(key_import(None, None, None).is_err());
        assert!(key_import(None, Some("/nonexistent/shx/key".into()), None).is_err());
    }

    #[test]
    fn connect_request_matches_what_the_frontend_sends() {
        let request: ConnectRequest = serde_json::from_value(serde_json::json!({
            "host": "example.com",
            "port": 2222,
            "username": "root",
            "auth": { "type": "key", "content": "KEY", "passphrase": null },
            "knownHostKeys": { "ssh-ed25519": "SHA256:abc" },
        }))
        .unwrap();
        assert_eq!((request.host.as_str(), request.port), ("example.com", 2222));
        assert!(matches!(request.auth, Auth::Key { content, passphrase: None } if content == "KEY"));
        assert_eq!(request.known_host_keys["ssh-ed25519"], "SHA256:abc");

        for (auth, expect_password) in [
            (serde_json::json!({ "type": "password", "password": "pw" }), true),
            (serde_json::json!({ "type": "agent" }), false),
        ] {
            let request: ConnectRequest = serde_json::from_value(serde_json::json!({
                "host": "h", "port": 22, "username": "u", "auth": auth,
            }))
            .unwrap();
            assert_eq!(matches!(request.auth, Auth::Password { .. }), expect_password);
            assert!(request.known_host_keys.is_empty());
        }
    }

    #[tokio::test]
    #[ignore]
    async fn exec_reports_output_and_exit_code() {
        let s = session().await;
        let out = s.exec("printf hello; printf oops >&2; exit 3").await.unwrap();
        assert_eq!((out.stdout.as_str(), out.stderr.as_str(), out.code), ("hello", "oops", 3));
        assert_eq!(out.check().unwrap_err().to_string(), "oops");

        let tricky = "it's a \"test\" $HOME `id`";
        let echoed = s.exec(&format!("printf %s {}", sh_quote(tricky))).await.unwrap();
        assert_eq!(echoed.stdout, tricky);
    }
}
