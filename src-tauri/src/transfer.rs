use std::collections::HashMap;
use std::future::Future;
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use futures::stream::{self, StreamExt, TryStreamExt};
use parking_lot::Mutex;
use russh_sftp::client::fs::Metadata;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, OpenFlags};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncSeekExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::{watch, Semaphore};

use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::sftp::{base_name, join, pack_command, pack_dir, pack_dir_command, unique_path};
use crate::ssh::{sh_quote, AppState, Session};

const MAX_CONCURRENT: usize = 3;
/// 一个任务里同时传输的文件数。文件又多又小时，时间主要花在每个文件的几次往返上，
/// 让这些等待重叠起来能快上几倍。
const FILES_AT_ONCE: usize = 4;
const CHUNK: usize = 128 * 1024;
const PROGRESS_INTERVAL: Duration = Duration::from_millis(200);
const MAX_DEPTH: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    Upload,
    Download,
}

/// 目标文件已存在时的处理方式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Conflict {
    Overwrite,
    Skip,
    Rename,
    /// 续传不完整的文件；大小相同则视为已传完。
    Resume,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransferState {
    Queued,
    /// 打包下载：压缩包正在服务器上生成。
    Packing,
    Running,
    Paused,
    Done,
    Failed,
    Cancelled,
}

impl TransferState {
    fn finished(self) -> bool {
        matches!(self, Self::Done | Self::Failed | Self::Cancelled)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferInfo {
    id: String,
    session_id: String,
    direction: Direction,
    name: String,
    source: String,
    dest_dir: String,
    /// 下载到了本机的哪里，供“打开文件位置”使用；上传没有。
    local: Option<String>,
    state: TransferState,
    total: u64,
    transferred: u64,
    files_total: u32,
    files_done: u32,
    /// 最近一个进度间隔内的速度（字节/秒）。
    speed: u64,
    current: String,
    error: Option<String>,
}

#[derive(Clone, Copy, PartialEq)]
enum Ctl {
    Run,
    Pause,
    Cancel,
}

struct Status {
    state: TransferState,
    current: String,
    error: Option<String>,
}

/// 打包下载的信息：不逐个文件下载，而是先在服务器上打成压缩包，只下载这一个文件。
struct Pack {
    /// 被打包条目所在的文件夹，以及具体是哪些条目。
    dir: String,
    names: Vec<String>,
    /// 压缩包的文件名；扩展名决定格式。
    archive: String,
    level: Option<u8>,
    /// 服务器上为压缩包创建的临时目录（存在期间）。
    holder: Mutex<Option<String>>,
    /// 压缩包完整生成后置位。下载完成之前压缩包一直保留，
    /// 这样中断的下载可以续传，不必重新打包。
    ready: Mutex<bool>,
}

/// 任务对某个文件的处理记录。重新运行时据此接着做，而不是根据目标位置碰巧有什么来判断。
#[derive(Clone)]
struct Written {
    /// 实际写到了哪里：原定位置；该位置被占用时是另一个名字。
    dest: String,
    /// 当时源文件的大小。现在大小不同就说明不是同一个文件了。
    size: u64,
    done: bool,
}

struct Task {
    id: String,
    /// 任务所用的连接；所属标签页重连后会换成新的连接。
    session_id: Mutex<String>,
    direction: Direction,
    source: String,
    dest_dir: String,
    conflict: Conflict,
    pack: Option<Pack>,
    /// 以远程路径为键，记录到目前为止处理过的每个文件。
    written: Mutex<HashMap<String, Written>>,
    status: Mutex<Status>,
    total: AtomicU64,
    transferred: AtomicU64,
    files_total: AtomicU32,
    files_done: AtomicU32,
    speed: AtomicU64,
    ctl: watch::Sender<Ctl>,
}

impl Task {
    fn new(
        session_id: &str,
        direction: Direction,
        source: String,
        dest_dir: &str,
        conflict: Conflict,
    ) -> Self {
        Self {
            id: uuid::Uuid::new_v4().to_string(),
            session_id: Mutex::new(session_id.to_string()),
            direction,
            source,
            dest_dir: dest_dir.to_string(),
            conflict,
            pack: None,
            written: Mutex::new(HashMap::new()),
            status: Mutex::new(Status {
                state: TransferState::Queued,
                current: String::new(),
                error: None,
            }),
            total: AtomicU64::new(0),
            transferred: AtomicU64::new(0),
            files_total: AtomicU32::new(0),
            files_done: AtomicU32::new(0),
            speed: AtomicU64::new(0),
            ctl: watch::channel(Ctl::Run).0,
        }
    }

    /// 把 `dir` 下的 `names` 作为一个压缩包下载，先在服务器上打包。
    fn packed(session_id: &str, pack: Pack, dest_dir: &str, conflict: Conflict) -> Self {
        // 列表里显示压缩包的名字；它在服务器的哪里打包属于任务内部的事。
        let mut task = Self::new(session_id, Direction::Download, pack.archive.clone(), dest_dir, conflict);
        task.pack = Some(pack);
        task
    }

    /// 下载的东西在本机的位置：文件夹是文件夹本身；单个文件是实际写出的那个文件，
    /// 原名被占用时它已经换了名字，不能只靠目标文件夹和名字去拼。
    fn local_path(&self) -> Option<String> {
        if self.direction != Direction::Download {
            return None;
        }
        let dest_dir = Path::new(&self.dest_dir);
        let written = self.written.lock();
        let only = match written.len() {
            1 => written.values().next().filter(|w| Path::new(&w.dest).parent() == Some(dest_dir)),
            _ => None,
        };
        let planned = || dest_dir.join(source_name(self.direction, &self.source)).to_string_lossy().into_owned();
        Some(only.map_or_else(planned, |w| w.dest.clone()))
    }

    fn snapshot(&self) -> TransferInfo {
        let local = self.local_path();
        let status = self.status.lock();
        TransferInfo {
            id: self.id.clone(),
            session_id: self.session_id.lock().clone(),
            direction: self.direction,
            name: source_name(self.direction, &self.source),
            source: self.source.clone(),
            dest_dir: self.dest_dir.clone(),
            local,
            state: status.state,
            total: self.total.load(Ordering::Relaxed),
            transferred: self.transferred.load(Ordering::Relaxed),
            files_total: self.files_total.load(Ordering::Relaxed),
            files_done: self.files_done.load(Ordering::Relaxed),
            speed: self.speed.load(Ordering::Relaxed),
            current: status.current.clone(),
            error: status.error.clone(),
        }
    }

    fn set_state(&self, state: TransferState) {
        self.status.lock().state = state;
    }

    fn add(&self, bytes: u64) {
        self.transferred.fetch_add(bytes, Ordering::Relaxed);
    }

    /// 暂停时阻塞，取消后返回错误。在数据块之间调用。
    async fn checkpoint(&self, ctl: &mut watch::Receiver<Ctl>) -> Result<()> {
        loop {
            let now = *ctl.borrow_and_update();
            match now {
                Ctl::Run => return Ok(()),
                Ctl::Cancel => return Err(AppError::Cancelled),
                Ctl::Pause => {
                    self.set_state(TransferState::Paused);
                    if ctl.changed().await.is_err() {
                        return Err(AppError::Cancelled);
                    }
                    self.set_state(TransferState::Running);
                }
            }
        }
    }
}

/// 传输的条目叫什么：列表里显示它，目标一侧也用这个名字。上传时 `source` 是本机路径，
/// 分隔符随系统而定（Windows 上是反斜杠），交给标准库去拆；下载时是远程路径，总是正斜杠。
fn source_name(direction: Direction, source: &str) -> String {
    let local = || Path::new(source).file_name().map(|name| name.to_string_lossy().into_owned());
    match direction {
        Direction::Upload => local().unwrap_or_else(|| source.to_string()),
        Direction::Download => base_name(source).to_string(),
    }
}

pub struct Manager {
    tasks: Mutex<Vec<Arc<Task>>>,
    permits: Arc<Semaphore>,
}

impl Default for Manager {
    fn default() -> Self {
        Self {
            tasks: Mutex::new(Vec::new()),
            permits: Arc::new(Semaphore::new(MAX_CONCURRENT)),
        }
    }
}

impl Manager {
    fn find(&self, id: &str) -> Result<Arc<Task>> {
        self.tasks
            .lock()
            .iter()
            .find(|t| t.id == id)
            .cloned()
            .ok_or_else(|| AppError::msg(tr("传输任务不存在", "The transfer does not exist")))
    }
}

/// 一个要复制的文件。`local` 和 `remote` 都有值，方向决定哪个是源。
struct Item {
    local: PathBuf,
    remote: String,
    size: u64,
    mode: Option<u32>,
}

#[cfg(unix)]
fn local_mode(meta: &std::fs::Metadata) -> Option<u32> {
    use std::os::unix::fs::PermissionsExt;
    Some(meta.permissions().mode() & 0o777)
}

#[cfg(not(unix))]
fn local_mode(_: &std::fs::Metadata) -> Option<u32> {
    None
}

fn walk_local(
    local: &Path,
    remote: String,
    dirs: &mut Vec<String>,
    files: &mut Vec<Item>,
    depth: usize,
) -> std::io::Result<()> {
    // 跟随符号链接；链接失效就跳过，不让整个上传失败。
    let Ok(meta) = std::fs::metadata(local) else {
        return Ok(());
    };
    if meta.is_file() {
        files.push(Item {
            local: local.to_path_buf(),
            remote,
            size: meta.len(),
            mode: local_mode(&meta),
        });
    } else if meta.is_dir() && depth < MAX_DEPTH {
        for child in std::fs::read_dir(local)? {
            let child = child?;
            let name = child.file_name().to_string_lossy().into_owned();
            walk_local(&child.path(), join(&remote, &name), dirs, files, depth + 1)?;
        }
        dirs.push(remote);
    }
    Ok(())
}

async fn walk_remote(
    sftp: &SftpSession,
    remote: String,
    local: PathBuf,
    dirs: &mut Vec<PathBuf>,
    files: &mut Vec<Item>,
    depth: usize,
) -> Result<()> {
    dirs.push(local.clone());
    for child in sftp.read_dir(&remote).await? {
        let name = child.file_name();
        // 这些名字由服务器给出，绝不能让它们跳出下载目录。
        if name == ".." || name.contains('/') || name.contains('\\') {
            continue;
        }
        let (path, mut meta) = (child.path(), child.metadata());
        if meta.file_type() == FileType::Symlink {
            match sftp.metadata(&path).await {
                // 跳过指向目录的符号链接：跟进去可能无限循环。
                Ok(target) if target.file_type() == FileType::File => meta = target,
                _ => continue,
            }
        }
        match meta.file_type() {
            FileType::Dir if depth < MAX_DEPTH => {
                Box::pin(walk_remote(sftp, path, local.join(&name), dirs, files, depth + 1)).await?
            }
            FileType::File => files.push(Item {
                local: local.join(&name),
                remote: path,
                size: meta.size.unwrap_or(0),
                mode: meta.permissions,
            }),
            _ => {}
        }
    }
    Ok(())
}

fn unique_local(path: &Path) -> PathBuf {
    let stem = path.file_stem().unwrap_or_default().to_string_lossy();
    let ext = path
        .extension()
        .map(|e| format!(".{}", e.to_string_lossy()))
        .unwrap_or_default();
    let mut candidate = path.to_path_buf();
    let mut n = 1;
    while candidate.exists() {
        candidate = path.with_file_name(format!("{stem} ({n}){ext}"));
        n += 1;
    }
    candidate
}

/// 对已有 `have` 字节的目标文件应用冲突策略的结果。
enum Plan {
    Skip,
    /// 从这个偏移开始写入目标（0 = 从头写）。
    Write(u64),
    Rename,
}

fn plan(conflict: Conflict, have: Option<u64>, size: u64) -> Plan {
    match (conflict, have) {
        (_, None) | (Conflict::Overwrite, _) => Plan::Write(0),
        (Conflict::Skip, _) => Plan::Skip,
        (Conflict::Rename, _) => Plan::Rename,
        (Conflict::Resume, Some(have)) if have == size => Plan::Skip,
        (Conflict::Resume, Some(have)) if have < size => Plan::Write(have),
        (Conflict::Resume, _) => Plan::Write(0),
    }
}

/// 本任务在之前的运行中已开始传输的文件，这次从哪里继续：接在目标已有的 `have`
/// 字节之后；如果源文件已不是当时那个（当时 `was` 字节，现在 `size` 字节），
/// 或目标与之对不上，则从头开始。
fn continue_at(have: Option<u64>, was: u64, size: u64) -> u64 {
    match have {
        Some(have) if was == size && have <= size => have,
        _ => 0,
    }
}

/// 任务被取消时完成，否则永不完成。
async fn cancelled(ctl: &mut watch::Receiver<Ctl>) {
    let _ = ctl.wait_for(|c| *c == Ctl::Cancel).await;
    // 通道关闭（任务已不存在）等同于取消：这时 `wait_for` 同样会返回。
}

/// 执行 `work`，除非任务先被取消。
async fn unless_cancelled<T>(ctl: &mut watch::Receiver<Ctl>, work: impl Future<Output = Result<T>>) -> Result<T> {
    tokio::select! {
        out = work => out,
        _ = cancelled(ctl) => Err(AppError::Cancelled),
    }
}

/// 删除打包下载留在服务器上的压缩包（如果有）。
async fn discard(session: &Session, pack: &Pack) {
    *pack.ready.lock() = false;
    let holder = pack.holder.lock().take();
    if let Some(holder) = holder {
        let _ = session.exec(&format!("rm -rf -- {}", sh_quote(&holder))).await;
    }
}

/// 确保打包下载的压缩包已在服务器上，并返回其路径。
/// 之前被中断的运行留下的压缩包直接沿用，否则现在打包。
async fn archive(
    session: &Session,
    sftp: &SftpSession,
    task: &Task,
    pack: &Pack,
    ctl: &mut watch::Receiver<Ctl>,
) -> Result<String> {
    let kept = if *pack.ready.lock() { pack.holder.lock().clone() } else { None };
    if let Some(holder) = kept {
        let path = join(&holder, &pack.archive);
        if sftp.metadata(&path).await.is_ok() {
            return Ok(path);
        }
    }
    // 重新打包得到的是另一个压缩包，旧压缩包已下载的部分不能接着用。
    discard(session, pack).await;
    task.written.lock().clear();
    task.set_state(TransferState::Packing);

    let made = unless_cancelled(ctl, session.exec(&pack_dir_command())).await?.check()?;
    let holder = pack_dir(&made)
        .ok_or_else(|| AppError::msg(tr("无法在服务器上创建临时目录", "Could not create a temporary directory on the server")))?;
    *pack.holder.lock() = Some(holder.clone());
    let command = pack_command(&pack.dir, &pack.names, &holder, &pack.archive, pack.level)?;
    unless_cancelled(ctl, session.exec(&command)).await?.check()?;
    *pack.ready.lock() = true;
    task.set_state(TransferState::Running);
    Ok(join(&holder, &pack.archive))
}

async fn pump(
    task: &Task,
    ctl: &mut watch::Receiver<Ctl>,
    mut reader: impl AsyncRead + Unpin,
    mut writer: impl AsyncWrite + Unpin,
) -> Result<()> {
    let mut buf = vec![0u8; CHUNK];
    loop {
        task.checkpoint(ctl).await?;
        // 连接卡住时读写可能迟迟不返回，取消不能等它。
        let n = unless_cancelled(ctl, async { Ok(reader.read(&mut buf).await?) }).await?;
        if n == 0 {
            break;
        }
        unless_cancelled(ctl, async { Ok(writer.write_all(&buf[..n]).await?) }).await?;
        task.add(n as u64);
    }
    unless_cancelled(ctl, async { Ok(writer.shutdown().await?) }).await
}

/// 列出任务要复制的全部文件，并在目标一侧建好它们所在的目录。
async fn collect(sftp: &SftpSession, task: &Task, source: &str) -> Result<Vec<Item>> {
    let name = source_name(task.direction, source);
    let mut files = Vec::new();
    match task.direction {
        Direction::Upload => {
            let (local, remote) = (PathBuf::from(source), join(&task.dest_dir, &name));
            let (mut dirs, found) = tokio::task::spawn_blocking(move || {
                let (mut dirs, mut files) = (Vec::new(), Vec::new());
                walk_local(&local, remote, &mut dirs, &mut files, 0).map(|_| (dirs, files))
            })
            .await
            .map_err(|e| AppError::msg(e.to_string()))??;
            files = found;
            // 父目录排在子目录前面，按顺序创建不会缺上级目录。
            dirs.sort();
            for dir in dirs {
                if !sftp.try_exists(&dir).await? {
                    sftp.create_dir(&dir).await?;
                }
            }
        }
        Direction::Download => {
            let local = PathBuf::from(&task.dest_dir).join(&name);
            let meta = sftp.metadata(source).await?;
            if meta.file_type() == FileType::Dir {
                let mut dirs = Vec::new();
                walk_remote(sftp, source.to_string(), local, &mut dirs, &mut files, 0).await?;
                for dir in dirs {
                    tokio::fs::create_dir_all(dir).await?;
                }
            } else {
                files.push(Item {
                    local,
                    remote: source.to_string(),
                    size: meta.size.unwrap_or(0),
                    mode: meta.permissions,
                });
            }
        }
    }
    Ok(files)
}

async fn upload_file(
    sftp: &SftpSession,
    task: &Task,
    ctl: &mut watch::Receiver<Ctl>,
    item: &Item,
    conflict: Conflict,
) -> Result<()> {
    let size_of = |path: String| async move { sftp.metadata(&path).await.ok().map(|meta| meta.size.unwrap_or(0)) };
    let earlier = task.written.lock().get(&item.remote).cloned();
    let (remote, offset) = match earlier {
        Some(w) if w.done && w.size == item.size => {
            task.add(item.size);
            return Ok(());
        }
        // 本任务在之前的运行中已开始：接着写它自己写出的文件，
        // 不受针对原有文件的冲突策略影响。
        Some(w) => {
            let offset = continue_at(size_of(w.dest.clone()).await, w.size, item.size);
            (w.dest, offset)
        }
        None => match plan(conflict, size_of(item.remote.clone()).await, item.size) {
            Plan::Skip => {
                task.add(item.size);
                return Ok(());
            }
            Plan::Write(offset) => (item.remote.clone(), offset),
            Plan::Rename => {
                let (dir, name) = item.remote.rsplit_once('/').unwrap_or((".", &item.remote));
                (unique_path(sftp, dir, name).await?, 0)
            }
        },
    };
    let record = |done| Written { dest: remote.clone(), size: item.size, done };
    task.written.lock().insert(item.remote.clone(), record(false));

    let mut src = tokio::fs::File::open(&item.local).await?;
    let flags = if offset > 0 {
        OpenFlags::WRITE
    } else {
        OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE
    };
    let mut dst = sftp.open_with_flags(&remote, flags).await?;
    if offset > 0 {
        src.seek(SeekFrom::Start(offset)).await?;
        dst.seek(SeekFrom::Start(offset)).await?;
        task.add(offset);
    }
    pump(task, ctl, src, dst).await?;

    if let Some(mode) = item.mode {
        let attrs = Metadata {
            permissions: Some(mode),
            ..Metadata::empty()
        };
        let _ = sftp.set_metadata(&remote, attrs).await;
    }
    task.written.lock().insert(item.remote.clone(), record(true));
    Ok(())
}

async fn download_file(
    sftp: &SftpSession,
    task: &Task,
    ctl: &mut watch::Receiver<Ctl>,
    item: &Item,
    conflict: Conflict,
) -> Result<()> {
    let size_of = |path: PathBuf| async move { tokio::fs::metadata(&path).await.ok().map(|meta| meta.len()) };
    let earlier = task.written.lock().get(&item.remote).cloned();
    let (local, offset) = match earlier {
        Some(w) if w.done && w.size == item.size => {
            task.add(item.size);
            return Ok(());
        }
        // 本任务在之前的运行中已开始：接着写它自己写出的文件，
        // 不受针对原有文件的冲突策略影响。
        Some(w) => {
            let local = PathBuf::from(w.dest);
            let offset = continue_at(size_of(local.clone()).await, w.size, item.size);
            (local, offset)
        }
        None => match plan(conflict, size_of(item.local.clone()).await, item.size) {
            Plan::Skip => {
                task.add(item.size);
                return Ok(());
            }
            Plan::Write(offset) => (item.local.clone(), offset),
            Plan::Rename => (unique_local(&item.local), 0),
        },
    };
    let record = |done| Written { dest: local.to_string_lossy().into_owned(), size: item.size, done };
    task.written.lock().insert(item.remote.clone(), record(false));

    let mut src = sftp.open(&item.remote).await?;
    let mut options = tokio::fs::OpenOptions::new();
    options.write(true).create(true).truncate(offset == 0);
    let mut dst = options.open(&local).await?;
    if offset > 0 {
        src.seek(SeekFrom::Start(offset)).await?;
        dst.seek(SeekFrom::Start(offset)).await?;
        task.add(offset);
    }
    pump(task, ctl, src, dst).await?;

    #[cfg(unix)]
    if let Some(mode) = item.mode {
        use std::os::unix::fs::PermissionsExt;
        let perms = std::fs::Permissions::from_mode(mode & 0o777);
        let _ = tokio::fs::set_permissions(&local, perms).await;
    }
    task.written.lock().insert(item.remote.clone(), record(true));
    Ok(())
}

async fn run(session: &Session, task: &Task, permits: &Semaphore) -> Result<()> {
    let mut ctl = task.ctl.subscribe();

    // 等待空位；排队中的任务可以不经启动就被取消。
    let _permit = tokio::select! {
        permit = permits.acquire() => permit.map_err(|_| AppError::Cancelled)?,
        _ = cancelled(&mut ctl) => return Err(AppError::Cancelled),
    };
    task.set_state(TransferState::Running);
    task.checkpoint(&mut ctl).await?;

    let sftp = session.open_sftp().await?;
    let conflict = task.conflict;
    let source = match &task.pack {
        Some(pack) => archive(session, &sftp, task, pack, &mut ctl).await?,
        None => task.source.clone(),
    };
    // 目录树很大时光是列出来就要很久，这一步同样可以取消。
    let files = unless_cancelled(&mut ctl, collect(&sftp, task, &source)).await?;

    task.total
        .store(files.iter().map(|f| f.size).sum(), Ordering::Relaxed);
    task.files_total.store(files.len() as u32, Ordering::Relaxed);

    let sftp = &sftp;
    stream::iter(&files)
        .map(Ok::<_, AppError>)
        .try_for_each_concurrent(FILES_AT_ONCE, |item| async move {
            let mut ctl = task.ctl.subscribe();
            task.checkpoint(&mut ctl).await?;
            task.status.lock().current = base_name(&item.remote).to_string();
            let copied = match task.direction {
                Direction::Upload => upload_file(sftp, task, &mut ctl, item, conflict).await,
                Direction::Download => download_file(sftp, task, &mut ctl, item, conflict).await,
            };
            copied.map_err(|e| match e {
                AppError::Cancelled => e,
                e => AppError::msg(format!("{}: {e}", base_name(&item.remote))),
            })?;
            task.files_done.fetch_add(1, Ordering::Relaxed);
            Ok(())
        })
        .await?;
    let _ = sftp.close().await;
    Ok(())
}

fn spawn(app: AppHandle, session: Arc<Session>, task: Arc<Task>, permits: Arc<Semaphore>) {
    // 不用 `tokio::spawn`：重试会从同步命令走到这里，同步命令在主线程上执行，
    // 不在运行时内，那样会 panic 并让整个应用崩溃。
    tauri::async_runtime::spawn(async move {
        let work = run(&session, &task, &permits);
        tokio::pin!(work);
        let mut tick = tokio::time::interval(PROGRESS_INTERVAL);
        let mut last_emitted = None;
        let (mut last_bytes, mut last_time) = (0u64, Instant::now());

        let result = loop {
            tokio::select! {
                result = &mut work => break result,
                _ = tick.tick() => {
                    let bytes = task.transferred.load(Ordering::Relaxed);
                    let elapsed = last_time.elapsed().as_secs_f64();
                    if elapsed > 0.0 {
                        let rate = (bytes.saturating_sub(last_bytes)) as f64 / elapsed;
                        task.speed.store(rate as u64, Ordering::Relaxed);
                    }
                    (last_bytes, last_time) = (bytes, Instant::now());
                    let snapshot = task.snapshot();
                    if last_emitted.as_ref() != Some(&snapshot) {
                        let _ = app.emit("transfer-update", &snapshot);
                        last_emitted = Some(snapshot);
                    }
                }
            }
        };

        task.speed.store(0, Ordering::Relaxed);
        // 下载完成或被放弃后，打包下载的压缩包就没用了。
        // 失败时保留它，供重试时续传。
        if let Some(pack) = &task.pack {
            if matches!(result, Ok(()) | Err(AppError::Cancelled)) {
                discard(&session, pack).await;
            }
        }
        {
            let mut status = task.status.lock();
            status.current.clear();
            match result {
                Ok(()) => status.state = TransferState::Done,
                Err(AppError::Cancelled) => status.state = TransferState::Cancelled,
                Err(e) => {
                    status.state = TransferState::Failed;
                    status.error = Some(e.to_string());
                }
            }
        }
        let _ = app.emit("transfer-update", task.snapshot());
    });
}

/// 为每个源路径排入一个传输任务。上传时 `sources` 是本地路径、`dest_dir` 是远程目录，
/// 下载时相反。
#[tauri::command]
pub async fn transfer_start(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    direction: Direction,
    sources: Vec<String>,
    dest_dir: String,
    conflict: Conflict,
) -> Result<Vec<TransferInfo>> {
    let session = state.session(&session_id)?;
    let mut started = Vec::new();
    for source in sources {
        let task = Arc::new(Task::new(&session_id, direction, source, &dest_dir, conflict));
        started.push(task.snapshot());
        state.transfers.tasks.lock().push(task.clone());
        spawn(app.clone(), session.clone(), task, state.transfers.permits.clone());
    }
    Ok(started)
}

/// 把远程文件夹 `dir` 下的条目打成一个压缩包后下载，先在服务器上打包：
/// 传输的是一个文件，而不是成千上万个。
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn transfer_pack(
    app: AppHandle,
    state: State<'_, AppState>,
    session_id: String,
    dir: String,
    names: Vec<String>,
    archive: String,
    level: Option<u8>,
    dest_dir: String,
    conflict: Conflict,
) -> Result<TransferInfo> {
    // 请求有问题就现在拒绝，不等到轮到这个任务时才报错。
    pack_command(&dir, &names, "/", &archive, level)?;
    let session = state.session(&session_id)?;
    let pack = Pack { dir, names, archive, level, holder: Mutex::new(None), ready: Mutex::new(false) };
    let task = Arc::new(Task::packed(&session_id, pack, &dest_dir, conflict));
    let started = task.snapshot();
    state.transfers.tasks.lock().push(task.clone());
    spawn(app, session, task, state.transfers.permits.clone());
    Ok(started)
}

/// 把一个连接上的传输任务移交给另一个连接，即标签页重连后得到的新连接。
/// 之后，因连接丢失而中断的任务就可以重试。
#[tauri::command]
pub fn transfer_rebind(state: State<'_, AppState>, from: String, to: String) {
    for task in state.transfers.tasks.lock().iter() {
        let mut session_id = task.session_id.lock();
        if *session_id == from {
            *session_id = to.clone();
        }
    }
}

/// 在连接彻底关闭前调用：删除打包下载为重试留在服务器上的压缩包，因为重试不会再发生。
#[tauri::command]
pub async fn transfer_release(state: State<'_, AppState>, session_id: String) -> Result<()> {
    let Ok(session) = state.session(&session_id) else {
        return Ok(());
    };
    let tasks: Vec<Arc<Task>> = state.transfers.tasks.lock().iter().filter(|t| *t.session_id.lock() == session_id).cloned().collect();
    for task in tasks {
        let finished = task.status.lock().state.finished();
        if let (Some(pack), true) = (&task.pack, finished) {
            discard(&session, pack).await;
        }
    }
    Ok(())
}

#[tauri::command]
pub fn transfer_control(state: State<'_, AppState>, id: String, action: String) -> Result<()> {
    let task = state.transfers.find(&id)?;
    let ctl = match action.as_str() {
        "pause" => Ctl::Pause,
        "resume" => Ctl::Run,
        "cancel" => Ctl::Cancel,
        _ => return Err(AppError::msg(tr("未知操作", "Unknown action"))),
    };
    task.ctl.send_replace(ctl);
    Ok(())
}

/// 重新运行失败或已取消的传输。已完成的文件不再重传，传到一半的文件续传，
/// 其余的按首次运行处理。
#[tauri::command]
pub fn transfer_retry(app: AppHandle, state: State<'_, AppState>, id: String) -> Result<()> {
    let task = state.transfers.find(&id)?;
    let session_id = task.session_id.lock().clone();
    let session = state.session(&session_id)?;
    {
        let mut status = task.status.lock();
        if !status.state.finished() {
            return Ok(());
        }
        status.state = TransferState::Queued;
        status.error = None;
    }
    task.transferred.store(0, Ordering::Relaxed);
    task.files_done.store(0, Ordering::Relaxed);
    task.ctl.send_replace(Ctl::Run);
    spawn(app, session, task, state.transfers.permits.clone());
    Ok(())
}

#[tauri::command]
pub fn transfer_list(state: State<'_, AppState>) -> Vec<TransferInfo> {
    state.transfers.tasks.lock().iter().map(|t| t.snapshot()).collect()
}

/// 从列表中清除已结束的传输。失败的打包下载为重试留在服务器上的压缩包也一并删除。
#[tauri::command]
pub fn transfer_clear(state: State<'_, AppState>) {
    let mut tasks = state.transfers.tasks.lock();
    let (gone, kept): (Vec<_>, Vec<_>) = tasks.drain(..).partition(|t| t.status.lock().state.finished());
    *tasks = kept;
    for task in gone.into_iter().filter(|t| t.pack.is_some()) {
        let session_id = task.session_id.lock().clone();
        if let Ok(session) = state.session(&session_id) {
            tauri::async_runtime::spawn(async move {
                if let Some(pack) = &task.pack {
                    discard(&session, pack).await;
                }
            });
        }
    }
}

/// `names` 中有哪些已存在于本地目录 `dir`。
#[tauri::command]
pub fn local_existing(dir: String, names: Vec<String>) -> Vec<String> {
    let dir = PathBuf::from(dir);
    names.into_iter().filter(|n| dir.join(n).exists()).collect()
}

#[tauri::command]
pub fn default_download_dir() -> Option<String> {
    dirs::download_dir()
        .or_else(dirs::home_dir)
        .map(|p| p.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::tests::{env, session};

    fn pattern(len: usize, seed: u8) -> Vec<u8> {
        (0..len).map(|i| (i as u8).wrapping_mul(31).wrapping_add(seed)).collect()
    }

    async fn go(s: &Session, dir: Direction, source: &str, dest: &str, c: Conflict) -> (Result<()>, TransferInfo) {
        let task = Task::new("test", dir, source.to_string(), dest, c);
        let result = run(s, &task, &Semaphore::new(1)).await;
        (result, task.snapshot())
    }

    #[test]
    fn a_download_knows_where_it_landed() {
        let task = |direction, source: &str| Task::new("test", direction, source.to_string(), "/home/me/Downloads", Conflict::Rename);
        let wrote = |task: &Task, remote: &str, dest: &str| {
            task.written.lock().insert(remote.to_string(), Written { dest: dest.to_string(), size: 1, done: true });
        };
        let local = |task: &Task| task.snapshot().local;

        assert_eq!(local(&task(Direction::Upload, "/home/me/a.txt")), None);
        // 还没写出任何文件时，是打算放的位置。
        let file = task(Direction::Download, "/var/log/app.log");
        assert_eq!(local(&file).as_deref(), Some("/home/me/Downloads/app.log"));
        // 原名被占用、换了名字：以实际写出的为准。
        wrote(&file, "/var/log/app.log", "/home/me/Downloads/app (1).log");
        assert_eq!(local(&file).as_deref(), Some("/home/me/Downloads/app (1).log"));

        // 文件夹：位置是文件夹本身，不是里面的文件，哪怕只有一个。
        let dir = task(Direction::Download, "/var/www/site/");
        wrote(&dir, "/var/www/site/index.html", "/home/me/Downloads/site/index.html");
        assert_eq!(local(&dir).as_deref(), Some("/home/me/Downloads/site"));
        wrote(&dir, "/var/www/site/app.js", "/home/me/Downloads/site/app.js");
        assert_eq!(local(&dir).as_deref(), Some("/home/me/Downloads/site"));
    }

    #[test]
    fn an_item_is_named_after_the_last_part_of_its_path() {
        use Direction::*;
        assert_eq!(source_name(Download, "/var/www/site/"), "site");
        assert_eq!(source_name(Download, "site.tar.gz"), "site.tar.gz");
        assert_eq!(source_name(Upload, "/Users/me/My Site/"), "My Site");
        assert_eq!(source_name(Upload, "/Users/me/notes.txt"), "notes.txt");
        // 本机路径按本机的规则拆：Windows 上是反斜杠和盘符。
        #[cfg(windows)]
        assert_eq!(source_name(Upload, r"C:\Users\me\notes.txt"), "notes.txt");
        // 远程路径里的反斜杠只是文件名的一部分。
        assert_eq!(source_name(Download, r"/tmp/a\b"), r"a\b");
    }

    #[test]
    fn conflict_policy() {
        use Conflict::*;
        assert!(matches!(plan(Skip, None, 10), Plan::Write(0)));
        assert!(matches!(plan(Overwrite, Some(4), 10), Plan::Write(0)));
        assert!(matches!(plan(Skip, Some(4), 10), Plan::Skip));
        assert!(matches!(plan(Rename, Some(4), 10), Plan::Rename));
        assert!(matches!(plan(Resume, Some(4), 10), Plan::Write(4)));
        assert!(matches!(plan(Resume, Some(10), 10), Plan::Skip));
        assert!(matches!(plan(Resume, Some(11), 10), Plan::Write(0)));
    }

    #[test]
    fn a_file_begun_earlier_is_continued_only_if_it_still_fits() {
        // 接在已有内容之后继续。
        assert_eq!(continue_at(Some(4), 10, 10), 4);
        assert_eq!(continue_at(Some(0), 10, 10), 0);
        // 已经全部传完（刚传完就中断了）：没有要发送的内容。
        assert_eq!(continue_at(Some(10), 10, 10), 10);
        // 目标不存在、比源文件长，或源文件已不是原来那个：从头开始。
        assert_eq!(continue_at(None, 10, 10), 0);
        assert_eq!(continue_at(Some(11), 10, 10), 0);
        assert_eq!(continue_at(Some(4), 10, 12), 0);
    }

    /// 单个测试在两端使用的临时区域。
    struct Area {
        remote: String,
        local: PathBuf,
    }

    impl Area {
        async fn new(s: &Session) -> Self {
            let id = uuid::Uuid::new_v4();
            let area = Area { remote: join(&env("SHX_TEST_DIR"), &format!("retry-{id}")), local: std::env::temp_dir().join(format!("shx-retry-{id}")) };
            s.sftp().await.unwrap().create_dir(&area.remote).await.unwrap();
            std::fs::create_dir_all(&area.local).unwrap();
            area
        }

        async fn remove(self, s: &Session) {
            s.exec(&format!("rm -rf -- {}", sh_quote(&self.remote))).await.unwrap().check().unwrap();
            std::fs::remove_dir_all(self.local).unwrap();
        }
    }

    async fn put(s: &Session, path: &str, data: &[u8]) {
        let flags = OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE;
        let mut f = s.sftp().await.unwrap().open_with_flags(path, flags).await.unwrap();
        f.write_all(data).await.unwrap();
        f.shutdown().await.unwrap();
    }

    async fn again(s: &Session, task: &Task) -> TransferInfo {
        task.transferred.store(0, Ordering::Relaxed);
        task.files_done.store(0, Ordering::Relaxed);
        run(s, task, &Semaphore::new(1)).await.unwrap();
        task.snapshot()
    }

    #[tokio::test]
    #[ignore]
    async fn a_retried_download_continues_its_own_file_and_spares_the_one_that_was_there() {
        let s = session().await;
        let area = Area::new(&s).await;
        let data = pattern(CHUNK * 3 + 17, 7);
        let source = join(&area.remote, "big.bin");
        put(&s, &source, &data).await;
        // 下载文件夹里已有一个同名的其他文件，恰好还更短。
        let theirs = b"not ours, keep".to_vec();
        std::fs::write(area.local.join("big.bin"), &theirs).unwrap();
        let ours = area.local.join("big (1).bin");
        let dest = area.local.to_str().unwrap();

        // “保留两者”：下载的文件放在它旁边；任务再运行一次也不会多出文件。
        let task = Task::new("test", Direction::Download, source.clone(), dest, Conflict::Rename);
        let info = again(&s, &task).await;
        assert_eq!((info.transferred, info.files_done), (data.len() as u64, 1));
        assert_eq!(std::fs::read(&ours).unwrap(), data);
        let info = again(&s, &task).await;
        assert_eq!(info.transferred, data.len() as u64);
        assert!(!area.local.join("big (2).bin").exists());
        assert_eq!(std::fs::read(area.local.join("big.bin")).unwrap(), theirs);

        // 中途中断：重试时补全的是任务自己写了一半的文件，而不是另一个。
        std::fs::write(&ours, &data[..CHUNK + 5]).unwrap();
        task.written.lock().get_mut(&source).unwrap().done = false;
        let info = again(&s, &task).await;
        assert_eq!(info.transferred, data.len() as u64);
        assert_eq!(std::fs::read(&ours).unwrap(), data);
        assert_eq!(std::fs::read(area.local.join("big.bin")).unwrap(), theirs);

        // 期间源文件变了：已下载的部分作废，从头开始。
        let newer = pattern(CHUNK + 300, 9);
        put(&s, &source, &newer).await;
        again(&s, &task).await;
        assert_eq!(std::fs::read(&ours).unwrap(), newer);

        // “跳过”在重试时同样跳过：不会往原有的文件后面追加。
        let task = Task::new("test", Direction::Download, source, dest, Conflict::Skip);
        again(&s, &task).await;
        again(&s, &task).await;
        assert_eq!(std::fs::read(area.local.join("big.bin")).unwrap(), theirs);

        area.remove(&s).await;
    }

    #[tokio::test]
    #[ignore]
    async fn a_retried_upload_continues_its_own_file_and_spares_the_one_that_was_there() {
        let s = session().await;
        let sftp = s.sftp().await.unwrap();
        let area = Area::new(&s).await;
        let data = pattern(CHUNK * 2 + 99, 5);
        let source = area.local.join("big.bin");
        std::fs::write(&source, &data).unwrap();
        let theirs = b"already on the server".to_vec();
        put(&s, &join(&area.remote, "big.bin"), &theirs).await;
        let ours = join(&area.remote, "big (1).bin");

        let task = Task::new("test", Direction::Upload, source.to_string_lossy().into_owned(), &area.remote, Conflict::Rename);
        again(&s, &task).await;
        assert_eq!(sftp.read(&ours).await.unwrap(), data);

        put(&s, &ours, &data[..CHUNK - 1]).await;
        task.written.lock().values_mut().for_each(|w| w.done = false);
        let info = again(&s, &task).await;
        assert_eq!(info.transferred, data.len() as u64);
        assert_eq!(sftp.read(&ours).await.unwrap(), data);
        assert_eq!(sftp.read(join(&area.remote, "big.bin")).await.unwrap(), theirs);
        assert!(!sftp.try_exists(join(&area.remote, "big (2).bin")).await.unwrap());

        area.remove(&s).await;
    }

    #[tokio::test]
    #[ignore]
    async fn a_packed_download_keeps_its_archive_until_it_is_through() {
        let s = session().await;
        let sftp = s.sftp().await.unwrap();
        let area = Area::new(&s).await;
        sftp.create_dir(join(&area.remote, "site")).await.unwrap();
        put(&s, &join(&area.remote, "site/index.html"), &pattern(CHUNK * 2 + 1, 3)).await;
        put(&s, &join(&area.remote, "notes.txt"), b"hello").await;
        let local = area.local.join("site.tar");
        let listing = || String::from_utf8(std::process::Command::new("tar").arg("-tf").arg(&local).output().unwrap().stdout).unwrap();

        let pack = Pack {
            dir: area.remote.clone(),
            names: vec!["site".into(), "notes.txt".into()],
            archive: "site.tar".into(),
            level: None,
            holder: Mutex::new(None),
            ready: Mutex::new(false),
        };
        let task = Task::packed("test", pack, area.local.to_str().unwrap(), Conflict::Overwrite);
        let pack = task.pack.as_ref().unwrap();
        let info = again(&s, &task).await;
        assert_eq!((info.name.as_str(), info.files_done), ("site.tar", 1));
        assert!(listing().contains("site/index.html") && listing().contains("notes.txt"), "{}", listing());
        let whole = std::fs::read(&local).unwrap();

        // 压缩包仍在服务器上，独占一个目录：重试需要它。
        let holder = pack.holder.lock().clone().unwrap();
        let in_holder = || async { sftp.read_dir(&holder).await.map(|d| d.count()).unwrap_or(0) };
        assert_eq!(in_holder().await, 1);

        // 中途中断：重试时接着用同一个压缩包。
        std::fs::write(&local, &whole[..CHUNK + 5]).unwrap();
        task.written.lock().values_mut().for_each(|w| w.done = false);
        again(&s, &task).await;
        assert_eq!(pack.holder.lock().clone().unwrap(), holder);
        assert_eq!(std::fs::read(&local).unwrap(), whole);

        // 服务器上的压缩包没了：重新打包，并且不在旧压缩包已下载的部分上续传。
        s.exec(&format!("rm -rf -- {}", sh_quote(&holder))).await.unwrap().check().unwrap();
        std::fs::write(&local, b"left over from the old archive").unwrap();
        task.written.lock().values_mut().for_each(|w| w.done = false);
        again(&s, &task).await;
        let fresh = pack.holder.lock().clone().unwrap();
        assert_ne!(fresh, holder);
        assert!(listing().contains("site/index.html"), "{}", listing());

        // 用完之后，服务器上不留任何东西。
        discard(&s, pack).await;
        assert!(!sftp.try_exists(&fresh).await.unwrap());
        assert!(pack.holder.lock().is_none() && !*pack.ready.lock());

        // 开始前就取消：根本不会打包。
        task.ctl.send_replace(Ctl::Cancel);
        assert!(matches!(run(&s, &task, &Semaphore::new(1)).await, Err(AppError::Cancelled)));
        assert!(pack.holder.lock().is_none());

        area.remove(&s).await;
    }

    #[tokio::test]
    #[ignore]
    async fn round_trips_a_directory_tree() {
        let s = session().await;
        let sftp = s.sftp().await.unwrap();
        let id = uuid::Uuid::new_v4();
        let local = std::env::temp_dir().join(format!("shx-up-{id}"));
        let back = std::env::temp_dir().join(format!("shx-down-{id}"));
        let remote = join(&env("SHX_TEST_DIR"), &format!("xfer-{id}"));
        sftp.create_dir(&remote).await.unwrap();
        std::fs::create_dir_all(&back).unwrap();

        // 文件大小横跨数据块边界；文件名用来检验引号处理和 Unicode。
        let tree = local.join("tree");
        let files: Vec<(&str, Vec<u8>)> = vec![
            ("empty", vec![]),
            ("small.txt", b"hello".to_vec()),
            ("sub/big.bin", pattern(CHUNK * 3 + 17, 1)),
            ("sub/deep/exact.bin", pattern(CHUNK, 2)),
            ("sub/it's 中文 $x.txt", pattern(1000, 3)),
        ];
        for (name, data) in &files {
            let path = tree.join(name);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, data).unwrap();
        }
        std::fs::create_dir_all(tree.join("empty-dir")).unwrap();
        let total: u64 = files.iter().map(|(_, d)| d.len() as u64).sum();

        let (result, info) = go(&s, Direction::Upload, tree.to_str().unwrap(), &remote, Conflict::Overwrite).await;
        result.unwrap();
        assert_eq!((info.total, info.transferred, info.files_total, info.files_done), (total, total, 5, 5));
        let remote_tree = join(&remote, "tree");
        for (name, data) in &files {
            assert_eq!(&sftp.read(join(&remote_tree, name)).await.unwrap(), data, "{name}");
        }
        assert!(sftp.try_exists(join(&remote_tree, "empty-dir")).await.unwrap());

        let (result, info) = go(&s, Direction::Download, &remote_tree, back.to_str().unwrap(), Conflict::Overwrite).await;
        result.unwrap();
        assert_eq!((info.total, info.transferred, info.files_done), (total, total, 5));
        for (name, data) in &files {
            assert_eq!(&std::fs::read(back.join("tree").join(name)).unwrap(), data, "{name}");
        }
        assert!(back.join("tree/empty-dir").is_dir());

        // 续传：被截断的目标会补全，完整的目标保持不动。
        let big_remote = join(&remote_tree, "sub/big.bin");
        let big = &files[2].1;
        let flags = OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE;
        let mut f = sftp.open_with_flags(&big_remote, flags).await.unwrap();
        f.write_all(&big[..CHUNK + 5]).await.unwrap();
        f.shutdown().await.unwrap();
        let (result, info) = go(&s, Direction::Upload, tree.to_str().unwrap(), &remote, Conflict::Resume).await;
        result.unwrap();
        assert_eq!(info.transferred, total);
        assert_eq!(&sftp.read(&big_remote).await.unwrap(), big);

        let big_local = back.join("tree/sub/big.bin");
        std::fs::write(&big_local, &big[..100]).unwrap();
        let (result, _) = go(&s, Direction::Download, &remote_tree, back.to_str().unwrap(), Conflict::Resume).await;
        result.unwrap();
        assert_eq!(&std::fs::read(&big_local).unwrap(), big);

        // “跳过”保留已有文件；“重命名”写到它旁边。
        let small = tree.join("small.txt");
        std::fs::write(&small, b"changed!").unwrap();
        go(&s, Direction::Upload, small.to_str().unwrap(), &remote_tree, Conflict::Skip).await.0.unwrap();
        assert_eq!(sftp.read(join(&remote_tree, "small.txt")).await.unwrap(), b"hello");
        go(&s, Direction::Upload, small.to_str().unwrap(), &remote_tree, Conflict::Rename).await.0.unwrap();
        assert_eq!(sftp.read(join(&remote_tree, "small (1).txt")).await.unwrap(), b"changed!");
        assert_eq!(sftp.read(join(&remote_tree, "small.txt")).await.unwrap(), b"hello");
        go(&s, Direction::Download, &join(&remote_tree, "small (1).txt"), back.join("tree").to_str().unwrap(), Conflict::Rename).await.0.unwrap();
        go(&s, Direction::Download, &join(&remote_tree, "small.txt"), back.join("tree").to_str().unwrap(), Conflict::Rename).await.0.unwrap();
        assert_eq!(std::fs::read(back.join("tree/small (1).txt")).unwrap(), b"changed!");
        assert_eq!(std::fs::read(back.join("tree/small (2).txt")).unwrap(), b"hello");

        // 被取消的任务直接停止，不碰目标。
        let task = Task::new("test", Direction::Upload, small.to_string_lossy().into_owned(), &remote, Conflict::Overwrite);
        task.ctl.send_replace(Ctl::Cancel);
        assert!(matches!(run(&s, &task, &Semaphore::new(1)).await, Err(AppError::Cancelled)));
        assert!(!sftp.try_exists(join(&remote, "small.txt")).await.unwrap());

        // 源不存在时带原因失败，而不是卡住。
        let (result, _) = go(&s, Direction::Download, &join(&remote, "nope"), back.to_str().unwrap(), Conflict::Overwrite).await;
        assert!(result.is_err());

        s.exec(&format!("rm -rf -- {}", crate::ssh::sh_quote(&remote))).await.unwrap().check().unwrap();
        std::fs::remove_dir_all(local).unwrap();
        std::fs::remove_dir_all(back).unwrap();
    }
}
