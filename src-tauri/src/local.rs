//! 本地 shell：在本机伪终端里运行系统 shell，会话 id 以 `local:` 开头。
//! 终端命令（term_open / term_write / term_resize）与远程会话共用，前端只需换一种 id。

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{mpsc, Arc, Mutex as StdMutex};

use parking_lot::Mutex;
use portable_pty::{ChildKiller, CommandBuilder, MasterPty, PtySize, native_pty_system};
use tauri::ipc::{Channel, InvokeResponseBody};
use tauri::{AppHandle, Emitter, State};

use crate::error::{AppError, Result};
use crate::ssh::{AppState, SessionClosed};

/// 本地会话 id 前缀。
pub const ID_PREFIX: &str = "local:";

/// 输入输出共用的终端命令。
pub enum TermCmd {
    Write(Vec<u8>),
    Resize(u32, u32),
}

pub fn is_local(id: &str) -> bool {
    id.starts_with(ID_PREFIX)
}

fn local_id() -> String {
    format!("{ID_PREFIX}{}", uuid::Uuid::new_v4())
}

fn clamp_size(v: u32) -> u16 {
    v.clamp(1, u16::MAX as u32) as u16
}

/// 各系统都认定为登录 shell 的参数。
fn shell_args(path: &str) -> Vec<String> {
    let name = std::path::Path::new(path)
        .file_stem()
        .unwrap_or_default()
        .to_string_lossy()
        .to_lowercase();
    match name.as_str() {
        "zsh" => vec!["-l".into()],
        "bash" | "sh" | "dash" | "ksh" => vec!["-l".into()],
        "powershell" | "pwsh" => vec!["-NoLogo".into()],
        _ => Vec::new(),
    }
}

// ─── Session ────────────────────────────────────────────────────────────────

pub struct LocalSession {
    command: Option<CommandBuilder>,
    child: Arc<StdMutex<Option<Box<dyn portable_pty::Child + Send + Sync>>>>,
    killer: Arc<Mutex<Option<Box<dyn ChildKiller + Send + Sync>>>>,
    master: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
    tx: mpsc::Sender<TermCmd>,
    rx: Arc<StdMutex<Option<mpsc::Receiver<TermCmd>>>>,
    channel: Arc<Mutex<Option<Channel<InvokeResponseBody>>>>,
    started: Arc<AtomicUsize>,
    ending: Arc<AtomicUsize>,
}

/// 读写线程之一先结束时负责结束会话，只生效一次。
struct Ending {
    app: AppHandle,
    locals: Locals,
    id: String,
    /// 与 session 共享的 killer 槽；先结束的 Ending 从中取出真实 killer 并 kill。
    killer: Arc<Mutex<Option<Box<dyn ChildKiller + Send + Sync>>>>,
    rx: Arc<StdMutex<Option<mpsc::Receiver<TermCmd>>>>,
    ending: Arc<AtomicUsize>,
}

impl Drop for Ending {
    fn drop(&mut self) {
        if self.ending.swap(1, Ordering::SeqCst) != 0 {
            return;
        }
        if let Some(mut killer) = self.killer.lock().take() {
            let _ = killer.kill();
        }
        // 关闭输入通道，让 writer_loop 退出。
        drop(self.rx.lock().unwrap().take());
        let (app, id) = (self.app.clone(), self.id.clone());
        let locals = self.locals.clone();
        tauri::async_runtime::spawn(async move {
            if locals.remove(&id).is_some() {
                let _ = app.emit("session-closed", SessionClosed { id });
            }
        });
    }
}

// ─── Locals registry ────────────────────────────────────────────────────────

#[derive(Clone, Default)]
pub struct Locals(Arc<parking_lot::RwLock<HashMap<String, Arc<LocalSession>>>>);

impl Locals {
    fn insert(&self, id: String, session: Arc<LocalSession>) {
        self.0.write().insert(id, session);
    }

    fn remove(&self, id: &str) -> Option<Arc<LocalSession>> {
        self.0.write().remove(id)
    }

    fn get(&self, id: &str) -> Result<Arc<LocalSession>> {
        self.0.read().get(id).cloned().ok_or(AppError::NoSession)
    }
}

// ─── Public commands ────────────────────────────────────────────────────────

pub fn send(locals: &Locals, id: &str, cmd: TermCmd) -> Result<()> {
    locals
        .get(id)?
        .tx
        .send(cmd)
        .map_err(|_| AppError::NoSession)
}

/// 关闭本地会话。
pub fn disconnect(app: &AppHandle, locals: &Locals, id: &str) {
    let Ok(session) = locals.get(id) else { return };
    // 作用域结束即触发 Ending::drop：kill 子进程、断输入通道、移出注册表。
    let _ending = Ending {
        app: app.clone(),
        locals: locals.clone(),
        id: id.to_string(),
        killer: session.killer.clone(),
        rx: session.rx.clone(),
        ending: session.ending.clone(),
    };
}

/// 建立本地会话，返回会话 id；shell 由随后的 term_open 启动。
#[tauri::command]
pub fn local_connect(state: State<'_, AppState>) -> Result<String> {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| {
        #[cfg(windows)]
        { "powershell.exe".to_string() }
        #[cfg(not(windows))]
        { "/bin/sh".to_string() }
    });
    let home = crate::localfs::home()?;
    let user = std::env::var("USER")
        .or_else(|_| std::env::var("USERNAME"))
        .unwrap_or_else(|_| home.file_name().unwrap_or_default().to_string_lossy().into_owned());

    let mut cmd = CommandBuilder::new(&shell);
    for arg in shell_args(&shell) {
        cmd.arg(arg);
    }
    cmd.cwd(&home);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("USER", &user);

    let (tx, rx) = mpsc::channel();
    let session = Arc::new(LocalSession {
        command: Some(cmd),
        child: Arc::new(StdMutex::new(None)),
        killer: Arc::new(Mutex::new(None)),
        master: Arc::new(Mutex::new(None)),
        tx,
        rx: Arc::new(StdMutex::new(Some(rx))),
        channel: Arc::new(Mutex::new(None)),
        started: Arc::new(AtomicUsize::new(0)),
        ending: Arc::new(AtomicUsize::new(0)),
    });
    let id = local_id();
    state.locals.insert(id.clone(), session);
    Ok(id)
}

/// 启动伪终端与读写线程；由 ssh::term_open 把本地 id 转来。
pub fn open(
    app: &AppHandle,
    locals: &Locals,
    id: &str,
    cols: u32,
    rows: u32,
    on_data: Channel<InvokeResponseBody>,
) -> Result<()> {
    let session = locals.get(id)?;
    if session.started.swap(1, Ordering::SeqCst) != 0 {
        return Ok(());
    }

    let system = native_pty_system();
    let pair = system
        .openpty(PtySize {
            rows: clamp_size(rows),
            cols: clamp_size(cols),
            pixel_width: 0,
            pixel_height: 0,
        })
        .map_err(|e| AppError::msg(e.to_string()))?;

    let command = session.command.clone().ok_or(AppError::NoSession)?;
    let child = pair
        .slave
        .spawn_command(command)
        .map_err(|e| AppError::msg(e.to_string()))?;
    let killer = child.clone_killer();
    *session.child.lock().unwrap() = Some(child);
    *session.killer.lock() = Some(killer);

    let reader = pair
        .master
        .try_clone_reader()
        .map_err(|e| AppError::msg(e.to_string()))?;
    let writer = pair
        .master
        .take_writer()
        .map_err(|e| AppError::msg(e.to_string()))?;
    let master = pair.master;
    *session.master.lock() = Some(master);
    *session.channel.lock() = Some(on_data);

    let ending = || Ending {
        app: app.clone(),
        locals: locals.clone(),
        id: id.to_string(),
        killer: session.killer.clone(),
        rx: session.rx.clone(),
        ending: session.ending.clone(),
    };

    // 输入线程（阻塞接收 TermCmd，写入 PTY）。
    {
        let ending = ending();
        let rx = session.rx.clone();
        let master_ref = session.master.clone();
        std::thread::spawn(move || {
            writer_loop(rx, writer, master_ref);
            drop(ending);
        });
    }

    // 输出线程（阻塞读 PTY，经 Channel 送到前端）。
    {
        let ending = ending();
        let channel = session.channel.clone();
        std::thread::spawn(move || {
            reader_loop(reader, channel);
            drop(ending);
        });
    }
    Ok(())
}

// ─── Writer loop ────────────────────────────────────────────────────────────

fn writer_loop(
    rx_slot: Arc<StdMutex<Option<mpsc::Receiver<TermCmd>>>>,
    mut writer: Box<dyn Write + Send>,
    master_slot: Arc<Mutex<Option<Box<dyn MasterPty + Send>>>>,
) {
    let rx = rx_slot.lock().unwrap().take();
    let Some(rx) = rx else { return };
    while let Ok(cmd) = rx.recv() {
        match cmd {
            TermCmd::Write(bytes) => {
                if writer.write_all(&bytes).and_then(|_| writer.flush()).is_err() {
                    return;
                }
            }
            TermCmd::Resize(cols, rows) => {
                if let Some(m) = master_slot.lock().as_ref() {
                    let _ = m.resize(PtySize {
                        rows: clamp_size(rows),
                        cols: clamp_size(cols),
                        pixel_width: 0,
                        pixel_height: 0,
                    });
                }
            }
        }
    }
}

// ─── Reader loop ────────────────────────────────────────────────────────────

fn reader_loop(mut reader: Box<dyn Read + Send>, channel: Arc<Mutex<Option<Channel<InvokeResponseBody>>>>) {
    let mut buf = [0u8; 8192];
    loop {
        match reader.read(&mut buf) {
            Ok(0) | Err(_) => break,
            Ok(n) => {
                if let Some(ch) = channel.lock().as_ref() {
                    let _ = ch.send(InvokeResponseBody::Raw(buf[..n].to_vec()));
                }
            }
        }
    }
}
