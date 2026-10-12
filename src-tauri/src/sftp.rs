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
use crate::ssh::{sh_quote, AppState, Names, Session};

/// 内置编辑器能打开的最大文件。
const MAX_TEXT_SIZE: u64 = 4 * 1024 * 1024;
const SEARCH_LIMIT: usize = 500;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FileEntry {
    pub name: String,
    pub path: String,
    /// "file" | "dir" | "link" | "other"
    pub kind: &'static str,
    /// 目录，或指向目录的符号链接。
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
    path: String,
    entries: Vec<FileEntry>,
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

fn parse_id_file(text: &str, id_field: usize) -> HashMap<u32, String> {
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

/// 补全符号链接的指向，以及目标是否为目录。
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
    Ok(state.session(&id)?.sftp().await?.create_dir(path).await?)
}

#[tauri::command]
pub async fn sftp_create_file(state: State<'_, AppState>, id: String, path: String) -> Result<()> {
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
    let session = state.session(&id)?;
    // 目录树很大时 `rm -rf` 快得多；没有 shell 时退回 SFTP。
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

/// 在 `dir` 中选一个不冲突的名字：`a.txt` → `a (1).txt`。
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
        // SFTP 的 rename 不能跨文件系统，`mv` 可以。
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
    let valid = |s: &str| s.chars().all(|c| c.is_ascii_alphanumeric() || "_.-".contains(c));
    if owner.is_empty() || !valid(&owner) || !valid(&group) {
        return Err(AppError::msg(tr("所有者或所属组名称无效", "Invalid owner or group name")));
    }
    let spec = if group.is_empty() { owner } else { format!("{owner}:{group}") };
    let flag = if recursive { "-R " } else { "" };
    let cmd = format!("chown {flag}{spec} -- {}", quote_all(&paths));
    state.session(&id)?.exec(&cmd).await?.check()?;
    Ok(())
}

/// 内置编辑器打开的文件。
#[derive(Debug, Serialize)]
pub struct TextFile {
    content: String,
    /// 当前用户能否把修改写回去；不能时编辑器以只读方式打开。
    writable: bool,
}

/// 服务器是否允许当前用户写这个文件。试着以写入方式打开它，不创建也不截断，
/// 文件本身原样不动。
async fn can_write(sftp: &SftpSession, path: &str) -> bool {
    match sftp.open_with_flags(path, OpenFlags::WRITE).await {
        Ok(mut file) => {
            let _ = file.shutdown().await;
            true
        }
        // 服务器明确拒绝才算只读；探测本身出了岔子（比如超时）不该拦着用户保存。
        Err(russh_sftp::client::error::Error::Status(_)) => false,
        Err(_) => true,
    }
}

#[tauri::command]
pub async fn sftp_read_text(state: State<'_, AppState>, id: String, path: String) -> Result<TextFile> {
    let sftp = state.session(&id)?.sftp().await?;
    read_text(&sftp, &path).await
}

async fn read_text(sftp: &SftpSession, path: &str) -> Result<TextFile> {
    let size = sftp.metadata(path).await?.size.unwrap_or(0);
    if size > MAX_TEXT_SIZE {
        return Err(AppError::msg(tr(
            "文件超过 4 MB，请下载后编辑",
            "The file is larger than 4 MB; download it to edit",
        )));
    }
    // 宁可拒绝也不做有损解码：保存有损解码的结果会损坏文件。
    let content = String::from_utf8(sftp.read(path).await?)
        .map_err(|_| AppError::msg(tr(
            "不是 UTF-8 文本文件，无法在内置编辑器中打开",
            "Not a UTF-8 text file; it cannot be opened in the built-in editor",
        )))?;
    Ok(TextFile { content, writable: can_write(sftp, path).await })
}

#[tauri::command]
pub async fn sftp_write_text(
    state: State<'_, AppState>,
    id: String,
    path: String,
    content: String,
) -> Result<()> {
    let sftp = state.session(&id)?.sftp().await?;
    let flags = OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE;
    let mut file = sftp.open_with_flags(path, flags).await?;
    file.write_all(content.as_bytes()).await?;
    file.shutdown().await?;
    Ok(())
}

/// 文件或目录树的总字节数。
#[tauri::command]
pub async fn sftp_du(state: State<'_, AppState>, id: String, path: String) -> Result<u64> {
    let out = state
        .session(&id)?
        .exec(&format!("du -sk -- {}", sh_quote(&path)))
        .await?;
    // 有条目读不了时 `du` 以非零退出，但仍会输出总计。
    out.stdout
        .split_whitespace()
        .next()
        .and_then(|kb| kb.parse::<u64>().ok())
        .map(|kb| kb * 1024)
        .ok_or_else(|| AppError::msg(out.stderr.trim().to_string()))
}

/// 压缩包的打包方式，由文件名决定。
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

/// 打包命令，其后依次接压缩包和要打包的条目。`level` 是压缩级别，
/// 从 0（不压缩，仅部分格式支持）到 9；不指定时使用工具自身的默认级别，
/// 并采用所有 tar 都认识的普通写法。
fn packer(format: Format, level: Option<u8>) -> String {
    let through = |tool: &str, level: u8| format!("tar --use-compress-program='{tool} -{level}' -cf");
    match (format, level.map(|l| l.min(9))) {
        (Format::Tar, _) => "tar -cf".into(),
        (Format::Zip, None) => "zip -r -q".into(),
        (Format::Zip, Some(l)) => format!("zip -r -q -{l}"),
        (Format::Gzip, None) => "tar -czf".into(),
        (Format::Bzip2, None) => "tar -cjf".into(),
        (Format::Xz, None) => "tar -cJf".into(),
        // gzip 和 bzip2 的级别从 1 开始。
        (Format::Gzip, Some(l)) => through("gzip", l.max(1)),
        (Format::Bzip2, Some(l)) => through("bzip2", l.max(1)),
        (Format::Xz, Some(l)) => through("xz", l),
    }
}

/// 把目录下的条目拼成参数。前面加 "./"，避免以 "-" 开头的名字被当成选项。
fn items(names: &[String]) -> String {
    names.iter().map(|n| sh_quote(&format!("./{n}"))).collect::<Vec<_>>().join(" ")
}

/// [`sftp_compress`] 实际执行的命令。
fn compress_command(dir: &str, names: &[String], archive: &str, level: Option<u8>, remove_source: bool) -> Result<String> {
    let format = format_of(archive)?;
    let target = sh_quote(&format!("./{archive}"));
    let mut cmd = format!("cd {} && ", sh_quote(dir));
    // zip 会往已有的压缩包里追加，而这里要的是一个新压缩包。
    if format == Format::Zip {
        cmd += &format!("rm -f {target} && ");
    }
    cmd += &format!("{} {target} {}", packer(format, level), items(names));
    if remove_source {
        // 压缩包写完之后才删除；并且绝不删除压缩包本身。
        let sources: Vec<String> = names.iter().filter(|n| n.as_str() != archive).cloned().collect();
        if !sources.is_empty() {
            cmd += &format!(" && rm -rf {}", items(&sources));
        }
    }
    Ok(cmd)
}

/// 把 `dir` 下的条目打包成 `archive`（同一目录下的文件名），格式由扩展名决定。
/// 指定 `remove_source` 时打包后删除源条目。
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
    let cmd = compress_command(&dir, &names, &archive, level, remove_source.unwrap_or(false))?;
    state.session(&id)?.exec(&cmd).await?.check()?;
    Ok(())
}

/// 包装 POSIX 脚本，使其不受用户登录 shell 的影响。
fn sh(script: &str) -> String {
    format!("sh -c {}", sh_quote(script))
}

/// 为打包下载创建存放压缩包的目录并输出其路径。使用单独的目录，不放在数据旁边，
/// 这样事后删除该目录时删掉的只有压缩包。选 /var/tmp 而不是 /tmp，
/// 因为后者在很多系统上位于内存中。
pub fn pack_dir_command() -> String {
    sh(r#"mktemp -d "${TMPDIR:-/var/tmp}/shx-pack.XXXXXX" 2>/dev/null || mktemp -d"#)
}

/// 从 [`pack_dir_command`] 的输出中取出目录。登录脚本可能在前面输出了别的内容，
/// 所以只看最后一行，并且要求它符合预期的形式：这个路径之后会被删除。
pub fn pack_dir(output: &str) -> Option<String> {
    let line = output.lines().map(str::trim).rfind(|line| !line.is_empty())?;
    let name = line.rsplit('/').next()?;
    (line.starts_with('/') && (name.starts_with("shx-pack.") || name.starts_with("tmp."))).then(|| line.to_string())
}

/// 把 `dir` 下的条目打包成 `holder` 中的 `archive`（文件名），格式由扩展名决定。
/// 压缩包不完整时命令失败。
pub fn pack_command(dir: &str, names: &[String], holder: &str, archive: &str, level: Option<u8>) -> Result<String> {
    if names.is_empty() || archive.is_empty() || archive.contains('/') {
        return Err(AppError::msg(tr("没有可打包的内容", "Nothing to pack")));
    }
    let format = format_of(archive)?;
    let target = sh_quote(&join(holder, archive));
    // 读取期间有文件发生变化（比如日志）时 GNU tar 以 1 退出，此时压缩包仍然可用。
    // 对其他 tar 和 zip 而言，1 是真正的失败。
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
    let lower = path.to_lowercase();
    let (file, dest) = (sh_quote(&path), sh_quote(&dest_dir));
    let cmd = if lower.ends_with(".zip") {
        format!("unzip -o -q {file} -d {dest}")
    } else if [".tar", ".tar.gz", ".tgz", ".tar.bz2", ".tbz2", ".tar.xz", ".txz"]
        .iter()
        .any(|ext| lower.ends_with(ext))
    {
        format!("tar -xf {file} -C {dest}")
    } else if lower.ends_with(".gz") {
        format!("gunzip -k -f {file}")
    } else {
        return Err(AppError::msg(tr("不支持的压缩格式", "Unsupported archive format")));
    };
    state.session(&id)?.exec(&cmd).await?.check()?;
    Ok(())
}

/// 在 `dir` 下按文件名搜索，不区分大小写，最多返回 `SEARCH_LIMIT` 条结果。
#[tauri::command]
pub async fn sftp_search(
    state: State<'_, AppState>,
    id: String,
    dir: String,
    pattern: String,
) -> Result<Vec<FileEntry>> {
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
            // 显示相对于搜索根目录的路径，结果更易读。
            let name = path.strip_prefix(dir).unwrap_or(path).trim_start_matches('/');
            Some(entry(name.to_string(), path.to_string(), &meta, names))
        }
    }))
    .await;
    Ok(found.into_iter().flatten().collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ssh::tests::{env, session};

    #[test]
    fn the_archive_name_decides_the_format_and_the_level_the_flags() {
        let cmd = |archive: &str, level| packer(format_of(archive).unwrap(), level);
        assert_eq!(cmd("a.tar.gz", None), "tar -czf");
        assert_eq!(cmd("A.TGZ", None), "tar -czf");
        assert_eq!(cmd("a.tar.bz2", None), "tar -cjf");
        assert_eq!(cmd("a.tar.xz", None), "tar -cJf");
        assert_eq!(cmd("a.tar", Some(9)), "tar -cf");
        assert_eq!(cmd("a.zip", None), "zip -r -q");
        assert_eq!(cmd("a.zip", Some(0)), "zip -r -q -0");
        assert_eq!(cmd("a.tar.gz", Some(9)), "tar --use-compress-program='gzip -9' -cf");
        // gzip 没有 0 级；任何格式都没有高于 9 的级别。
        assert_eq!(cmd("a.tgz", Some(0)), "tar --use-compress-program='gzip -1' -cf");
        assert_eq!(cmd("a.tar.xz", Some(40)), "tar --use-compress-program='xz -9' -cf");
        assert!(format_of("a.rar").is_err() && format_of("a.gz").is_err() && format_of("archive").is_err());
    }

    /// 在本机执行真实命令：只用到 sh、tar 和 zip，不需要 SSH。
    #[cfg(unix)]
    mod local {
        use super::super::*;
        use std::path::{Path, PathBuf};
        use std::process::{Command, Output};

        struct Work(PathBuf);

        impl Work {
            fn new() -> Self {
                let dir = std::env::temp_dir().join(format!("shx-pack-test-{}", uuid::Uuid::new_v4()));
                std::fs::create_dir_all(dir.join("my site").join("sub")).unwrap();
                std::fs::write(dir.join("my site").join("sub").join("it's.txt"), b"hello").unwrap();
                std::fs::write(dir.join("-dash.log"), b"x".repeat(4096)).unwrap();
                Work(dir)
            }
            fn path(&self) -> &str {
                self.0.to_str().unwrap()
            }
            fn sh(&self, script: &str) -> Output {
                Command::new("sh").arg("-c").arg(script).env("TMPDIR", &self.0).output().unwrap()
            }
        }

        impl Drop for Work {
            fn drop(&mut self) {
                let _ = std::fs::remove_dir_all(&self.0);
            }
        }

        fn has(tool: &str) -> bool {
            Command::new("sh").arg("-c").arg(format!("command -v {tool}")).output().unwrap().status.success()
        }

        fn names() -> Vec<String> {
            vec!["my site".to_string(), "-dash.log".to_string()]
        }

        fn listing(archive: &Path) -> String {
            let zip = archive.extension().unwrap() == "zip";
            let out = if zip { Command::new("unzip").arg("-Z1").arg(archive).output() } else { Command::new("tar").arg("-tf").arg(archive).output() };
            String::from_utf8(out.unwrap().stdout).unwrap()
        }

        #[test]
        fn packing_leaves_one_archive_in_a_directory_of_its_own() {
            let work = Work::new();
            let mut cases = vec![("my site.tar.gz", None), ("my site.tar.gz", Some(9)), ("my site.tar", None), ("my site.tar.bz2", Some(1))];
            if has("zip") && has("unzip") {
                cases.extend([("my site.zip", None), ("my site.zip", Some(0))]);
            }
            for (archive, level) in cases {
                let made = work.sh(&pack_dir_command());
                let holder = pack_dir(&String::from_utf8(made.stdout).unwrap()).unwrap();
                assert!(holder.starts_with(work.path()), "{holder}");
                let out = work.sh(&pack_command(work.path(), &names(), &holder, archive, level).unwrap());
                assert!(out.status.success(), "{archive} {level:?}: {}", String::from_utf8_lossy(&out.stderr));
                // 压缩包独占一个目录，删除该目录时删掉的只有它。
                assert_eq!(std::fs::read_dir(&holder).unwrap().count(), 1);
                let listed = listing(&Path::new(&holder).join(archive));
                assert!(listed.contains("my site/sub/it's.txt") && listed.contains("-dash.log"), "{archive}: {listed}");
                std::fs::remove_dir_all(&holder).unwrap();
            }

            // 没有可打包的内容，或内容不全：命令失败。
            let holder = pack_dir(&String::from_utf8(work.sh(&pack_dir_command()).stdout).unwrap()).unwrap();
            let missing = ["missing".to_string()];
            assert!(!work.sh(&pack_command(work.path(), &missing, &holder, "x.tar.gz", None).unwrap()).status.success());
            assert!(pack_command(work.path(), &[], &holder, "x.tar.gz", None).is_err());
            assert!(pack_command(work.path(), &names(), &holder, "../x.tar.gz", None).is_err());
            assert!(pack_command(work.path(), &names(), &holder, "x.rar", None).is_err());
        }

        #[test]
        fn only_a_directory_made_for_packing_is_taken_from_the_output() {
            assert_eq!(pack_dir("/var/tmp/shx-pack.Ab12Cd\n").as_deref(), Some("/var/tmp/shx-pack.Ab12Cd"));
            // 登录脚本先输出的内容不算结果。
            assert_eq!(pack_dir("Welcome!\r\n/var/tmp/shx-pack.x1\r\n").as_deref(), Some("/var/tmp/shx-pack.x1"));
            // 作为后备的普通 `mktemp -d` 生成的目录名是这种形式。
            assert_eq!(pack_dir("/tmp/tmp.Qw3rTy").as_deref(), Some("/tmp/tmp.Qw3rTy"));
            for bad in ["", "\n", "/", "/home/me", "shx-pack.abc", "/var/tmp/shx-pack.x1\n/etc"] {
                assert_eq!(pack_dir(bad), None, "{bad:?}");
            }
        }

        #[test]
        fn compressing_writes_next_to_the_data_and_can_remove_what_it_packed() {
            let work = Work::new();
            let out = work.sh(&compress_command(work.path(), &names(), "all.tar.gz", Some(1), false).unwrap());
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert!(listing(&work.0.join("all.tar.gz")).contains("my site/sub/it's.txt"));
            assert!(work.0.join("my site").exists() && work.0.join("-dash.log").exists());

            // 打包失败时不得动源文件。
            let mut wrong = names();
            wrong.push("missing".to_string());
            assert!(!work.sh(&compress_command(work.path(), &wrong, "bad.tar.gz", None, true).unwrap()).status.success());
            assert!(work.0.join("my site").exists() && work.0.join("-dash.log").exists());

            let out = work.sh(&compress_command(work.path(), &names(), "moved.tar", None, true).unwrap());
            assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
            assert!(listing(&work.0.join("moved.tar")).contains("-dash.log"));
            assert!(!work.0.join("my site").exists() && !work.0.join("-dash.log").exists());
            assert!(work.0.join("all.tar.gz").exists(), "other files are not touched");
        }

        #[test]
        fn a_zip_is_made_afresh_rather_than_added_to() {
            if !(has("zip") && has("unzip")) {
                return;
            }
            let work = Work::new();
            let one = ["-dash.log".to_string()];
            assert!(work.sh(&compress_command(work.path(), &names(), "a.zip", None, false).unwrap()).status.success());
            assert!(work.sh(&compress_command(work.path(), &one, "a.zip", Some(9), false).unwrap()).status.success());
            let listed = listing(&work.0.join("a.zip"));
            assert!(listed.contains("-dash.log") && !listed.contains("my site"), "{listed}");
        }
    }

    #[tokio::test]
    #[ignore]
    async fn list_resolves_links_and_owner_names() {
        let s = session().await;
        let sftp = s.sftp().await.unwrap();
        let dir = join(&env("SHX_TEST_DIR"), &format!("list-{}", uuid::Uuid::new_v4()));
        sftp.create_dir(&dir).await.unwrap();
        sftp.create_dir(join(&dir, "sub")).await.unwrap();
        sftp.write_new(&dir, "a.txt", b"12345").await;
        // OpenSSH 的参数顺序：先目标，后要创建的链接。
        sftp.symlink(join(&dir, "sub"), join(&dir, "to-dir")).await.unwrap();
        sftp.symlink(join(&dir, "missing"), join(&dir, "dangling")).await.unwrap();

        let listing = list(&s, &dir).await.unwrap();
        let get = |name: &str| listing.entries.iter().find(|e| e.name == name).unwrap();
        assert_eq!(listing.entries.len(), 4);
        assert_eq!((get("a.txt").kind, get("a.txt").size, get("a.txt").is_dir), ("file", 5, false));
        assert_eq!((get("sub").kind, get("sub").is_dir), ("dir", true));
        assert_eq!((get("to-dir").kind, get("to-dir").is_dir), ("link", true));
        assert!(get("to-dir").link_target.as_deref().unwrap().ends_with("/sub"));
        assert_eq!((get("dangling").kind, get("dangling").is_dir), ("link", false));
        // 登录用户可能来自目录服务，但 root 一定在 /etc/passwd 里。
        assert!(get("a.txt").uid.is_some());
        assert_eq!(s.names.get().unwrap().users.get(&0).map(String::as_str), Some("root"));
        assert_eq!(get("a.txt").path, join(&listing.path, "a.txt"));

        assert_eq!(unique_path(&sftp, &dir, "a.txt").await.unwrap(), join(&dir, "a (1).txt"));
        assert_eq!(unique_path(&sftp, &dir, "sub").await.unwrap(), join(&dir, "sub (1)"));
        assert_eq!(unique_path(&sftp, &dir, "new").await.unwrap(), join(&dir, "new"));

        remove_recursive(&sftp, dir.clone()).await.unwrap();
        assert!(!sftp.try_exists(&dir).await.unwrap());
    }

    #[tokio::test]
    #[ignore]
    async fn opening_a_file_to_edit_tells_whether_it_can_be_saved() {
        let s = session().await;
        let sftp = s.sftp().await.unwrap();
        let dir = join(&env("SHX_TEST_DIR"), &format!("edit-{}", uuid::Uuid::new_v4()));
        sftp.create_dir(&dir).await.unwrap();
        sftp.write_new(&dir, "conf", "listen 80;\n".as_bytes()).await;
        let path = join(&dir, "conf");
        let chmod = |mode| sftp.set_metadata(&path, Metadata { permissions: Some(mode), ..Metadata::empty() });

        let file = read_text(&sftp, &path).await.unwrap();
        assert_eq!((file.content.as_str(), file.writable), ("listen 80;\n", true));

        // 只读的文件照样能打开查看；探测是否可写不会动它的内容。
        chmod(0o444).await.unwrap();
        let before = sftp.metadata(&path).await.unwrap().mtime;
        let file = read_text(&sftp, &path).await.unwrap();
        assert_eq!((file.content.as_str(), file.writable), ("listen 80;\n", false));
        assert_eq!(sftp.read(&path).await.unwrap(), b"listen 80;\n");
        assert_eq!(sftp.metadata(&path).await.unwrap().mtime, before);

        // 连读都不行的文件打不开。
        chmod(0o000).await.unwrap();
        assert!(read_text(&sftp, &path).await.is_err());
        assert!(read_text(&sftp, &join(&dir, "missing")).await.is_err());

        chmod(0o644).await.unwrap();
        remove_recursive(&sftp, dir.clone()).await.unwrap();
    }

    trait WriteNew {
        async fn write_new(&self, dir: &str, name: &str, data: &[u8]);
    }

    impl WriteNew for SftpSession {
        async fn write_new(&self, dir: &str, name: &str, data: &[u8]) {
            let flags = OpenFlags::CREATE | OpenFlags::TRUNCATE | OpenFlags::WRITE;
            let mut file = self.open_with_flags(join(dir, name), flags).await.unwrap();
            file.write_all(data).await.unwrap();
            file.shutdown().await.unwrap();
        }
    }

    #[test]
    fn parses_passwd_and_group_lines() {
        let users = parse_id_file("root:x:0:0:root:/root:/bin/sh\n# junk\nweb:x:33:33::/var/www:", 2);
        assert_eq!(users.get(&0).map(String::as_str), Some("root"));
        assert_eq!(users.get(&33).map(String::as_str), Some("web"));
        assert_eq!(users.len(), 2);
    }

    #[test]
    fn path_helpers() {
        assert_eq!(join("/", "a"), "/a");
        assert_eq!(join("/a", "b"), "/a/b");
        assert_eq!(base_name("/a/b/"), "b");
        assert_eq!(base_name("/a/b.txt"), "b.txt");
    }
}
