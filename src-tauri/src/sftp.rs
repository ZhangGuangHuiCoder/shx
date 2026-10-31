use std::collections::HashMap;
use std::time::Duration;

use futures::future::join_all;
use russh_sftp::client::fs::Metadata;
use russh_sftp::client::SftpSession;
use russh_sftp::protocol::{FileType, OpenFlags};
use serde::Serialize;
use tauri::State;
use tokio::io::AsyncWriteExt;

use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::local;
use crate::localfs;
use crate::ssh::{sh_quote, AppState, Names, Session};

// 本地 shell 会话（id 以 `local:` 开头）没有 SFTP，各命令据此转给 localfs，
// 前端照旧用同一套 api，无需区分会话类型。

/// 编辑器可打开的最大文件。
pub(crate) const MAX_TEXT_SIZE: u64 = 4 * 1024 * 1024;
pub(crate) const SEARCH_LIMIT: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    /// "file" | "dir" | "link" | "other"
    pub kind: &'static str,
    /// 目录或指向目录的链接。
    pub is_dir: bool,
    pub size: u64,
    pub mtime: Option<u32>,
    pub mode: Option<u32>,
    pub uid: Option<u32>,
    pub gid: Option<u32>,
    pub owner: Option<String>,
    pub group: Option<String>,
    pub link_target: Option<String>,
}

#[derive(Serialize)]
pub struct Listing {
    pub(crate) path: String,
    pub(crate) entries: Vec<FileEntry>,
}

pub fn join(dir: &str, name: &str) -> String {
    if dir.ends_with('/') {
        format!("{dir}{name}")
    } else {
        format!("{dir}/{name}")
    }
}

pub fn base_name(path: &str) -> &str {
    path.trim_end_matches('/').rsplit('/').next().unwrap_or(path)
}

pub(crate) fn parse_id_file(text: &str, id_field: usize) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|line| {
            let mut fields = line.split(':');
            let name = fields.next()?;
            let id = fields.nth(id_field - 1)?.parse().ok()?;
            Some((id, name.to_string()))
        })
        .collect()
}

async fn read_small(sftp: &SftpSession, path: &str) -> Option<String> {
    let read = async {
        let size = sftp.metadata(path).await.ok()?.size?;
        if size > 1024 * 1024 {
            return None;
        }
        String::from_utf8(sftp.read(path).await.ok()?).ok()
    };
    tokio::time::timeout(Duration::from_secs(3), read)
        .await
        .ok()
        .flatten()
}

async fn names<'a>(session: &'a Session, sftp: &SftpSession) -> &'a Names {
    session
        .names
        .get_or_init(|| async {
            let (passwd, group) =
                tokio::join!(read_small(sftp, "/etc/passwd"), read_small(sftp, "/etc/group"));
            Names {
                users: passwd.map(|t| parse_id_file(&t, 2)).unwrap_or_default(),
                groups: group.map(|t| parse_id_file(&t, 2)).unwrap_or_default(),
            }
        })
        .await
}

fn entry(name: String, path: String, meta: &Metadata, names: &Names) -> FileEntry {
    let kind = match meta.file_type() {
        FileType::Dir => "dir",
        FileType::File => "file",
        FileType::Symlink => "link",
        FileType::Other => "other",
    };
    FileEntry {
        name,
        path,
        kind,
        is_dir: kind == "dir",
        size: meta.size.unwrap_or(0),
        mtime: meta.mtime,
        mode: meta.permissions,
        uid: meta.uid,
        gid: meta.gid,
        owner: meta.uid.and_then(|id| names.users.get(&id).cloned()),
        group: meta.gid.and_then(|id| names.groups.get(&id).cloned()),
        link_target: None,
    }
}

/// 解析符号链接目标及其是否为目录。
async fn resolve_link(sftp: &SftpSession, e: &mut FileEntry) {
    if e.kind != "link" {
        return;
    }
    let (target, meta) = tokio::join!(sftp.read_link(&e.path), sftp.metadata(&e.path));
    e.link_target = target.ok();
    if let Ok(meta) = meta {
        e.is_dir = meta.file_type() == FileType::Dir;
        e.size = meta.size.unwrap_or(e.size);
    }
}

#[tauri::command]
pub async fn sftp_list(state: State<'_, AppState>, id: String, path: String) -> Result<Listing> {
    if local::is_local(&id) {
        return localfs::list(path).await;
    }
    list(&*state.session(&id)?, &path).await
}

async fn list(session: &Session, path: &str) -> Result<Listing> {
    let sftp = session.sftp().await?;
    let path = sftp.canonicalize(if path.is_empty() { "." } else { path }).await?;
    let names = names(session, &sftp).await;
    let mut entries: Vec<FileEntry> = sftp
        .read_dir(&path)
        .await?
        .map(|e| {
            let name = e.file_name();
            entry(name.clone(), join(&path, &name), &e.metadata(), names)
        })
        .collect();
    join_all(entries.iter_mut().map(|e| resolve_link(&sftp, e))).await;
    Ok(Listing { path, entries })
}

#[tauri::command]
pub async fn sftp_stat(state: State<'_, AppState>, id: String, path: String) -> Result<FileEntry> {
    if local::is_local(&id) {
        return localfs::stat(path).await;
    }
    let session = state.session(&id)?;
    let sftp = session.sftp().await?;
    let meta = sftp.symlink_metadata(&path).await?;
    let names = names(&session, &sftp).await;
    let mut e = entry(base_name(&path).to_string(), path, &meta, names);
    resolve_link(&sftp, &mut e).await;
    Ok(e)
}

#[tauri::command]
pub async fn sftp_mkdir(state: State<'_, AppState>, id: String, path: String) -> Result<()> {
    if local::is_local(&id) {
        return localfs::mkdir(path).await;
    }
    Ok(state.session(&id)?.sftp().await?.create_dir(path).await?)
}

#[tauri::command]
pub async fn sftp_create_file(state: State<'_, AppState>, id: String, path: String) -> Result<()> {
    if local::is_local(&id) {
        return localfs::create_file(path).await;
    }
    let sftp = state.session(&id)?.sftp().await?;
    let flags = OpenFlags::CREATE | OpenFlags::EXCLUDE | OpenFlags::WRITE;
    let mut file = sftp.open_with_flags(path, flags).await?;
    file.shutdown().await?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_rename(
    state: State<'_, AppState>,
    id: String,
    from: String,
    to: String,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::rename(from, to).await;
    }
    let sftp = state.session(&id)?.sftp().await?;
    if sftp.try_exists(&to).await? {
        return Err(AppError::msg(format!("{} {}", base_name(&to), tr("已存在", "already exists"))));
    }
    Ok(sftp.rename(from, to).await?)
}

fn quote_all(paths: &[String]) -> String {
    paths.iter().map(|p| sh_quote(p)).collect::<Vec<_>>().join(" ")
}

async fn remove_recursive(sftp: &SftpSession, path: String) -> Result<()> {
    if sftp.symlink_metadata(&path).await?.file_type() != FileType::Dir {
        return Ok(sftp.remove_file(path).await?);
    }
    for child in sftp.read_dir(&path).await? {
        Box::pin(remove_recursive(sftp, child.path())).await?;
    }
    Ok(sftp.remove_dir(path).await?)
}

#[tauri::command]
pub async fn sftp_remove(state: State<'_, AppState>, id: String, paths: Vec<String>) -> Result<()> {
    if local::is_local(&id) {
        return localfs::remove(paths).await;
    }
    let session = state.session(&id)?;
    // 大目录用 rm -rf 更快；无 shell 时回退 SFTP。
    let rm = session.exec(&format!("rm -rf -- {}", quote_all(&paths))).await;
    if matches!(rm, Ok(out) if out.code == 0) {
        return Ok(());
    }
    let sftp = session.sftp().await?;
    for path in paths {
        if sftp.try_exists(&path).await.unwrap_or(true) {
            remove_recursive(&sftp, path).await?;
        }
    }
    Ok(())
}

/// 生成不冲突的名字：a.txt → a (1).txt。
pub async fn unique_path(sftp: &SftpSession, dir: &str, name: &str) -> Result<String> {
    let (stem, ext) = match name.rfind('.') {
        Some(i) if i > 0 => name.split_at(i),
        _ => (name, ""),
    };
    let mut candidate = join(dir, name);
    let mut n = 1;
    while sftp.try_exists(&candidate).await? {
        candidate = join(dir, &format!("{stem} ({n}){ext}"));
        n += 1;
    }
    Ok(candidate)
}

#[tauri::command]
pub async fn sftp_move(
    state: State<'_, AppState>,
    id: String,
    paths: Vec<String>,
    dest_dir: String,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::move_paths(paths, dest_dir).await;
    }
    let session = state.session(&id)?;
    let sftp = session.sftp().await?;
    for path in paths {
        let dest = join(&dest_dir, base_name(&path));
        if dest == path {
            continue;
        }
        if sftp.try_exists(&dest).await? {
            return Err(AppError::msg(format!(
                "{} {}",
                base_name(&path),
                tr("已存在于目标目录", "already exists in the destination")
            )));
        }
        // SFTP rename 不能跨文件系统，改用 mv。
        if sftp.rename(&path, &dest).await.is_err() {
            let cmd = format!("mv -- {} {}", sh_quote(&path), sh_quote(&dest));
            session.exec(&cmd).await?.check()?;
        }
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_copy(
    state: State<'_, AppState>,
    id: String,
    paths: Vec<String>,
    dest_dir: String,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::copy(paths, dest_dir).await;
    }
    let session = state.session(&id)?;
    let sftp = session.sftp().await?;
    for path in paths {
        let dest = unique_path(&sftp, &dest_dir, base_name(&path)).await?;
        let cmd = format!("cp -a -- {} {}", sh_quote(&path), sh_quote(&dest));
        session.exec(&cmd).await?.check()?;
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_chmod(
    state: State<'_, AppState>,
    id: String,
    paths: Vec<String>,
    mode: u32,
    recursive: bool,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::chmod(paths, mode, recursive).await;
    }
    let session = state.session(&id)?;
    let mode = mode & 0o7777;
    if recursive {
        let cmd = format!("chmod -R {mode:o} -- {}", quote_all(&paths));
        session.exec(&cmd).await?.check()?;
        return Ok(());
    }
    let sftp = session.sftp().await?;
    for path in paths {
        let attrs = Metadata {
            permissions: Some(mode),
            ..Metadata::empty()
        };
        sftp.set_metadata(path, attrs).await?;
    }
    Ok(())
}

#[tauri::command]
pub async fn sftp_chown(
    state: State<'_, AppState>,
    id: String,
    paths: Vec<String>,
    owner: String,
    group: String,
    recursive: bool,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::chown(paths, owner, group, recursive).await;
    }
    let spec = owner_spec(&owner, &group)?;
    let flag = if recursive { "-R " } else { "" };
    let cmd = format!("chown {flag}-- {spec} {}", quote_all(&paths));
    state.session(&id)?.exec(&cmd).await?.check()?;
    Ok(())
}

/// chown 的“所有者[:组]”参数；名称只允许安全字符，且不能以 - 开头（否则会被当作选项）。
pub(crate) fn owner_spec(owner: &str, group: &str) -> Result<String> {
    let valid = |s: &str| !s.starts_with('-') && s.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c));
    if owner.is_empty() || !valid(owner) || !valid(group) {
        return Err(AppError::msg(tr("所有者或所属组名称无效", "Invalid owner or group name")));
    }
    Ok(if group.is_empty() { owner.to_string() } else { format!("{owner}:{group}") })
}

#[derive(Debug, Serialize)]
pub struct TextFile {
    pub(crate) content: String,
    /// 不可写时编辑器只读打开。
    pub(crate) writable: bool,
}

/// 以写方式试开（不创建不截断）探测可写性，文件不变。
async fn can_write(sftp: &SftpSession, path: &str) -> bool {
    match sftp.open_with_flags(path, OpenFlags::WRITE).await {
        Ok(mut file) => {
            let _ = file.shutdown().await;
            true
        }
        // 只有明确拒绝才视为只读，探测出错不阻止保存。
        Err(russh_sftp::client::error::Error::Status(_)) => false,
        Err(_) => true,
    }
}

#[tauri::command]
pub async fn sftp_read_text(state: State<'_, AppState>, id: String, path: String) -> Result<TextFile> {
    if local::is_local(&id) {
        return localfs::read_text(path).await;
    }
    let sftp = state.session(&id)?.sftp().await?;
    read_text(&sftp, &path).await
}

/// 拒绝有损解码，否则保存会损坏文件。
pub(crate) fn decode_text(bytes: Vec<u8>) -> Result<String> {
    String::from_utf8(bytes).map_err(|_| AppError::msg(tr(
        "不是 UTF-8 文本文件，无法在内置编辑器中打开",
        "Not a UTF-8 text file; it cannot be opened in the built-in editor",
    )))
}

async fn read_text(sftp: &SftpSession, path: &str) -> Result<TextFile> {
    let size = sftp.metadata(path).await?.size.unwrap_or(0);
    if size > MAX_TEXT_SIZE {
        return Err(AppError::msg(tr(
            "文件超过 4 MB，请下载后编辑",
            "The file exceeds 4 MB; download it to edit",
        )));
    }
    let content = decode_text(sftp.read(path).await?)?;
    Ok(TextFile { content, writable: can_write(sftp, path).await })
}

#[tauri::command]
pub async fn sftp_write_text(
    state: State<'_, AppState>,
    id: String,
    path: String,
    content: String,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::write_text(path, content).await;
    }
    let sftp = state.session(&id)?.sftp().await?;
    let flags = OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE;
    let mut file = sftp.open_with_flags(path, flags).await?;
    file.write_all(content.as_bytes()).await?;
    file.shutdown().await?;
    Ok(())
}

#[tauri::command]
pub async fn sftp_du(state: State<'_, AppState>, id: String, path: String) -> Result<u64> {
    if local::is_local(&id) {
        return localfs::du(path).await;
    }
    let out = state
        .session(&id)?
        .exec(&format!("du -sk -- {}", sh_quote(&path)))
        .await?;
    // 部分条目不可读时 du 非零退出，但仍输出总计。
    out.stdout
        .split_whitespace()
        .next()
        .and_then(|kb| kb.parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .ok_or_else(|| AppError::msg(out.stderr.trim().to_string()))
}

/// 打包格式，由文件名决定。
#[derive(Clone, Copy, Debug, PartialEq)]
enum Format {
    Zip,
    Tar,
    Gzip,
    Bzip2,
    Xz,
}

fn format_of(archive: &str) -> Result<Format> {
    let lower = archive.to_lowercase();
    let ends = |exts: &[&str]| exts.iter().any(|ext| lower.ends_with(ext));
    Ok(if ends(&[".zip"]) {
        Format::Zip
    } else if ends(&[".tar.gz", ".tgz"]) {
        Format::Gzip
    } else if ends(&[".tar.bz2", ".tbz2"]) {
        Format::Bzip2
    } else if ends(&[".tar.xz", ".txz"]) {
        Format::Xz
    } else if ends(&[".tar"]) {
        Format::Tar
    } else {
        return Err(AppError::msg(tr(
            "仅支持 .tar.gz、.tar.bz2、.tar.xz、.tar、.zip",
            "Only .tar.gz, .tar.bz2, .tar.xz, .tar and .zip are supported",
        )));
    })
}

/// 打包命令前缀，后接压缩包和条目。level 为 0–9，None 时用工具默认值及最通用的写法。
fn packer(format: Format, level: Option<u8>) -> String {
    let through = |tool: &str, level: u8| format!("tar --use-compress-program='{tool} -{level}' -cf");
    match (format, level.map(|l| l.min(9))) {
        (Format::Tar, _) => "tar -cf".into(),
        (Format::Zip, None) => "zip -r -q".into(),
        (Format::Zip, Some(l)) => format!("zip -r -q -{l}"),
        (Format::Gzip, None) => "tar -czf".into(),
        (Format::Bzip2, None) => "tar -cjf".into(),
        (Format::Xz, None) => "tar -cJf".into(),
        // gzip/bzip2 级别从 1 开始。
        (Format::Gzip, Some(l)) => through("gzip", l.max(1)),
        (Format::Bzip2, Some(l)) => through("bzip2", l.max(1)),
        (Format::Xz, Some(l)) => through("xz", l),
    }
}

/// 条目加 "./" 前缀，避免以 - 开头被当作选项。
fn items(names: &[String]) -> String {
    names.iter().map(|n| sh_quote(&format!("./{n}"))).collect::<Vec<_>>().join(" ")
}

pub(crate) fn compress_command(dir: &str, names: &[String], archive: &str, level: Option<u8>, remove_source: bool) -> Result<String> {
    let format = format_of(archive)?;
    let target = sh_quote(&format!("./{archive}"));
    let mut cmd = format!("cd {} && ", sh_quote(dir));
    // zip 会追加到已有压缩包，先删除。
    if format == Format::Zip {
        cmd += &format!("rm -f {target} && ");
    }
    cmd += &format!("{} {target} {}", packer(format, level), items(names));
    if remove_source {
        // 打包成功后才删除源，且不删除压缩包本身。
        let sources: Vec<String> = names.iter().filter(|n| n.as_str() != archive).cloned().collect();
        if !sources.is_empty() {
            cmd += &format!(" && rm -rf {}", items(&sources));
        }
    }
    Ok(cmd)
}

/// 在同目录下打包，格式由扩展名决定；可选打包后删除源。
#[tauri::command]
pub async fn sftp_compress(
    state: State<'_, AppState>,
    id: String,
    dir: String,
    names: Vec<String>,
    archive: String,
    level: Option<u8>,
    remove_source: Option<bool>,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::compress(dir, names, archive, level, remove_source.unwrap_or(false)).await;
    }
    let cmd = compress_command(&dir, &names, &archive, level, remove_source.unwrap_or(false))?;
    state.session(&id)?.exec(&cmd).await?.check()?;
    Ok(())
}

/// 用 sh 执行，不受用户登录 shell 影响。
fn sh(script: &str) -> String {
    format!("sh -c {}", sh_quote(script))
}

/// 创建独立的临时目录存放压缩包，删除时只删压缩包。用 /var/tmp：/tmp 常位于内存中。
pub fn pack_dir_command() -> String {
    sh(r#"mktemp -d "${TMPDIR:-/var/tmp}/shx-pack.XXXXXX" 2>/dev/null || mktemp -d"#)
}

/// 只取最后一行并严格校验格式（登录脚本可能有输出，且该路径之后会被删除）。
pub fn pack_dir(output: &str) -> Option<String> {
    let line = output.lines().map(str::trim).rfind(|line| !line.is_empty())?;
    let name = line.rsplit('/').next()?;
    (line.starts_with('/') && (name.starts_with("shx-pack.") || name.starts_with("tmp."))).then(|| line.to_string())
}

/// 打包到 holder 目录，压缩包不完整时失败。
pub fn pack_command(dir: &str, names: &[String], holder: &str, archive: &str, level: Option<u8>) -> Result<String> {
    if names.is_empty() || archive.is_empty() || archive.contains('/') {
        return Err(AppError::msg(tr("没有可打包的内容", "Nothing to pack")));
    }
    let format = format_of(archive)?;
    let target = sh_quote(&join(holder, archive));
    // GNU tar 在文件读取期间变化时以 1 退出，压缩包仍可用；其他工具的 1 是真失败。
    let lenient = if format == Format::Zip {
        ""
    } else {
        "\nif [ \"$code\" -eq 1 ] && tar --version 2>/dev/null | grep -q 'GNU tar'; then code=0; fi"
    };
    Ok(sh(&format!(
        "cd {} && {} {target} {}\ncode=$?{lenient}\n[ \"$code\" -eq 0 ] && [ -s {target} ]",
        sh_quote(dir),
        packer(format, level),
        items(names),
    )))
}

#[tauri::command]
pub async fn sftp_extract(
    state: State<'_, AppState>,
    id: String,
    path: String,
    dest_dir: String,
) -> Result<()> {
    if local::is_local(&id) {
        return localfs::extract(path, dest_dir).await;
    }
    let cmd = extract_command(&path, &dest_dir)?;
    state.session(&id)?.exec(&cmd).await?.check()?;
    Ok(())
}

/// 解压命令，格式由扩展名决定；本地会话直接在本机执行同一套命令。
pub(crate) fn extract_command(path: &str, dest_dir: &str) -> Result<String> {
    let lower = path.to_lowercase();
    let (file, dest) = (sh_quote(path), sh_quote(dest_dir));
    if lower.ends_with(".zip") {
        Ok(format!("unzip -o -q {file} -d {dest}"))
    } else if [".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz"]
        .iter()
        .any(|ext| lower.ends_with(ext))
    {
        Ok(format!("tar -xf {file} -C {dest}"))
    } else if lower.ends_with(".gz") {
        Ok(format!("gunzip -k -f {file}"))
    } else {
        Err(AppError::msg(tr("不支持的压缩格式", "Unsupported archive format")))
    }
}

/// 按文件名搜索（不区分大小写），最多 SEARCH_LIMIT 条。
#[tauri::command]
pub async fn sftp_search(
    state: State<'_, AppState>,
    id: String,
    dir: String,
    pattern: String,
) -> Result<Vec<FileEntry>> {
    if local::is_local(&id) {
        return localfs::search(dir, &pattern).await;
    }
    let session = state.session(&id)?;
    let cmd = format!(
        "find {} -iname {} 2>/dev/null | head -n {SEARCH_LIMIT}",
        sh_quote(&dir),
        sh_quote(&format!("*{pattern}*"))
    );
    let out = session.exec(&cmd).await?.stdout;
    let sftp = session.sftp().await?;
    let names = names(&session, &sftp).await;
    let (sftp, dir) = (&sftp, dir.as_str());
    let found = join_all(out.lines().filter(|p| *p != dir).map(|path| {
        async move {
            let meta = sftp.symlink_metadata(path).await.ok()?;
            // 显示相对路径。
            let name = path.strip_prefix(dir).unwrap_or(path).trim_start_matches('/');
            Some(entry(name.to_string(), path.to_string(), &meta, names))
        }
    }))
    .await;
    Ok(found.into_iter().flatten().collect())
}
