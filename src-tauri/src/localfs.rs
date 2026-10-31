//! 本地文件系统：本地 shell 会话（id 以 `local:` 开头）的文件管理器后端。
//! 命令与 sftp.rs 一一对应，由 sftp.rs 按会话 id 分流，前端无需区分；
//! 返回的路径统一用 / 分隔（Windows 也接受），且都是绝对路径。

use std::collections::HashMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use std::sync::OnceLock;
use std::time::UNIX_EPOCH;

use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::sftp::{decode_text, parse_id_file, FileEntry, Listing, TextFile, MAX_TEXT_SIZE, SEARCH_LIMIT};
use crate::ssh::{ExecOutput, Names};
use crate::transfer::unique_local;

/// 搜索最多访问的条目数，避免在超大目录里长时间占用线程。
const SEARCH_VISITS: u64 = 200_000;

// ─── 路径与基础操作 ─────────────────────────────────────────────────────────

/// 文件操作是阻塞的，放到独立线程执行，避免占用异步运行时。
async fn off<T, F>(f: F) -> Result<T>
where
    F: FnOnce() -> Result<T> + Send + 'static,
    T: Send + 'static,
{
    tokio::task::spawn_blocking(f)
        .await
        .map_err(|e| AppError::msg(e.to_string()))?
}

pub(crate) fn home() -> Result<PathBuf> {
    dirs::home_dir().ok_or_else(|| {
        AppError::msg(tr(
            "找不到用户主目录",
            "Could not find the user home directory",
        ))
    })
}

/// 用户给的路径 → 绝对路径；空串与 "." 表示主目录，相对路径从主目录算起（与 SFTP 一致）。
/// 只做词法规范化，不解析符号链接（否则界面上的路径会跳到链接目标）。
fn absolute(path: &str) -> Result<PathBuf> {
    let raw = match path {
        "" | "." | "~" => home()?,
        _ => PathBuf::from(path.strip_prefix("~/").unwrap_or(path)),
    };
    let abs = if raw.is_absolute() { raw } else { home()?.join(raw) };
    let mut out = PathBuf::new();
    for part in abs.components() {
        match part {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            part => out.push(part.as_os_str()),
        }
    }
    Ok(out)
}

fn text(path: &Path) -> String {
    let path = path.to_string_lossy();
    // Unix 上反斜杠是文件名的合法字符，不能替换。
    if cfg!(windows) {
        path.replace('\\', "/")
    } else {
        path.into_owned()
    }
}

fn display_name(path: &Path) -> String {
    path.file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| text(path))
}

// ─── 条目 ───────────────────────────────────────────────────────────────────

/// 本机 uid/gid → 名称。只在首次使用时读取，与远程一样按 /etc/passwd 解析。
fn names() -> &'static Names {
    static NAMES: OnceLock<Names> = OnceLock::new();
    NAMES.get_or_init(|| Names {
        users: read_users(),
        groups: fs::read_to_string("/etc/group")
            .map(|t| parse_id_file(&t, 2))
            .unwrap_or_default(),
    })
}

fn read_users() -> HashMap<u32, String> {
    #[allow(unused_mut)]
    let mut users = fs::read_to_string("/etc/passwd")
        .map(|t| parse_id_file(&t, 2))
        .unwrap_or_default();
    // macOS 的 /etc/passwd 只有系统账户，普通账户在目录服务里。
    #[cfg(target_os = "macos")]
    if let Ok(out) = Command::new("/usr/bin/dscl")
        .args([".", "-list", "/Users", "UniqueID"])
        .output()
    {
        users.extend(parse_dscl_users(&String::from_utf8_lossy(&out.stdout)));
    }
    users
}

/// 解析 `dscl . -list /Users UniqueID` 的输出：每行“名称 空白 uid”；uid 为负的账户跳过。
#[cfg(target_os = "macos")]
fn parse_dscl_users(text: &str) -> HashMap<u32, String> {
    text.lines()
        .filter_map(|line| {
            let (name, id) = line.trim_end().rsplit_once(char::is_whitespace)?;
            Some((id.parse().ok()?, name.trim_end().to_string()))
        })
        .collect()
}

#[cfg(unix)]
fn attrs(meta: &fs::Metadata) -> (Option<u32>, Option<u32>, Option<u32>) {
    use std::os::unix::fs::MetadataExt;
    (Some(meta.uid()), Some(meta.gid()), Some(meta.mode() & 0o7777))
}

#[cfg(not(unix))]
fn attrs(_meta: &fs::Metadata) -> (Option<u32>, Option<u32>, Option<u32>) {
    (None, None, None)
}

/// 构造条目；符号链接按目标判断类型，与远程一致。
fn entry_of(path: &Path, name: String) -> Option<FileEntry> {
    let link = fs::symlink_metadata(path).ok()?;
    let is_link = link.file_type().is_symlink();
    // 目标不存在（断链）时退回链接自身。
    let meta = if is_link { fs::metadata(path).unwrap_or(link) } else { link };
    let (uid, gid, mode) = attrs(&meta);
    let dir = meta.is_dir();
    let names = names();
    Some(FileEntry {
        name,
        path: text(path),
        kind: if is_link {
            "link"
        } else if dir {
            "dir"
        } else if meta.is_file() {
            "file"
        } else {
            "other"
        },
        is_dir: dir,
        size: if dir { 0 } else { meta.len() },
        mtime: meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
            .map(|d| d.as_secs() as u32),
        mode,
        uid,
        gid,
        owner: uid.and_then(|id| names.users.get(&id).cloned()),
        group: gid.and_then(|id| names.groups.get(&id).cloned()),
        link_target: if is_link {
            fs::read_link(path).ok().map(|p| text(&p))
        } else {
            None
        },
    })
}

// ─── 目录与文件 ─────────────────────────────────────────────────────────────

pub async fn list(path: String) -> Result<Listing> {
    off(move || {
        let dir = absolute(&path)?;
        if !fs::metadata(&dir)?.is_dir() {
            return Err(AppError::msg(tr("不是文件夹", "Not a folder")));
        }
        let mut entries = Vec::new();
        for item in fs::read_dir(&dir)? {
            // 单个条目读取失败（权限、瞬时消失）时跳过，不影响其余。
            let Ok(item) = item else { continue };
            let name = item.file_name().to_string_lossy().into_owned();
            if let Some(e) = entry_of(&item.path(), name) {
                entries.push(e);
            }
        }
        Ok(Listing { path: text(&dir), entries })
    })
    .await
}

pub async fn stat(path: String) -> Result<FileEntry> {
    off(move || {
        let p = absolute(&path)?;
        entry_of(&p, display_name(&p))
            .ok_or_else(|| AppError::msg(tr("文件不存在", "The file does not exist")))
    })
    .await
}

pub async fn mkdir(path: String) -> Result<()> {
    off(move || {
        // 已存在时报错，与远程的 create_dir 一致。
        fs::create_dir(absolute(&path)?)?;
        Ok(())
    })
    .await
}

pub async fn create_file(path: String) -> Result<()> {
    off(move || {
        // 已存在时报错，与远程的排他创建一致。
        fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(absolute(&path)?)?;
        Ok(())
    })
    .await
}

pub async fn rename(from: String, to: String) -> Result<()> {
    off(move || {
        let (from, to) = (absolute(&from)?, absolute(&to)?);
        if fs::symlink_metadata(&to).is_ok() && !same_entry(&from, &to) {
            return Err(AppError::msg(format!(
                "{} {}",
                display_name(&to),
                tr("已存在", "already exists")
            )));
        }
        fs::rename(from, to)?;
        Ok(())
    })
    .await
}

/// 大小写不敏感的文件系统上，只改大小写时目标看似已存在，其实就是源本身。
fn same_entry(a: &Path, b: &Path) -> bool {
    if text(a).to_lowercase() != text(b).to_lowercase() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        match (fs::symlink_metadata(a), fs::symlink_metadata(b)) {
            (Ok(a), Ok(b)) => a.dev() == b.dev() && a.ino() == b.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    true
}

pub async fn remove(paths: Vec<String>) -> Result<()> {
    off(move || {
        for path in paths {
            remove_tree(&absolute(&path)?)?;
        }
        Ok(())
    })
    .await
}

pub async fn move_paths(paths: Vec<String>, dest_dir: String) -> Result<()> {
    off(move || {
        let dest_dir = absolute(&dest_dir)?;
        for path in paths {
            let src = absolute(&path)?;
            let Some(name) = src.file_name().map(|n| n.to_owned()) else {
                return Err(AppError::msg(tr("无效的源路径", "Invalid source path")));
            };
            let dest = dest_dir.join(&name);
            if dest == src {
                continue;
            }
            if fs::symlink_metadata(&dest).is_ok() {
                return Err(AppError::msg(format!(
                    "{} {}",
                    name.to_string_lossy(),
                    tr("已存在于目标目录", "already exists in the destination")
                )));
            }
            refuse_into_itself(&src, &dest_dir)?;
            match fs::rename(&src, &dest) {
                // 只有跨文件系统才改为复制后删除源；其他错误（如无权限）如实报告。
                Err(e) if e.kind() == std::io::ErrorKind::CrossesDevices => {
                    copy_fresh(&src, &dest)?;
                    remove_tree(&src)?;
                }
                other => other?,
            }
        }
        Ok(())
    })
    .await
}

pub async fn copy(paths: Vec<String>, dest_dir: String) -> Result<()> {
    off(move || {
        let dest_dir = absolute(&dest_dir)?;
        for path in paths {
            let src = absolute(&path)?;
            let Some(name) = src.file_name() else {
                continue;
            };
            refuse_into_itself(&src, &dest_dir)?;
            copy_fresh(&src, &unique_local(&dest_dir.join(name)))?;
        }
        Ok(())
    })
    .await
}

/// 文件夹不能放进自身或其子目录，否则复制会无限递归。按真实路径判断，经符号链接绕回也能发现。
fn refuse_into_itself(src: &Path, dest_dir: &Path) -> Result<()> {
    if fs::symlink_metadata(src)?.is_dir()
        && fs::canonicalize(dest_dir)?.starts_with(fs::canonicalize(src)?)
    {
        return Err(AppError::msg(tr(
            "不能把文件夹放进它自身",
            "A folder cannot be placed inside itself",
        )));
    }
    Ok(())
}

/// 复制到尚不存在的 dest；失败时清掉已复制的部分。
fn copy_fresh(src: &Path, dest: &Path) -> Result<()> {
    copy_tree(src, dest).inspect_err(|_| {
        let _ = remove_tree(dest);
    })
}

/// 递归复制；符号链接按链接本身复制，不展开，避免环路。
/// 任何一项失败都要报错：跨文件系统移动随后会删除源。
fn copy_tree(src: &Path, dest: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(src)?;
    if meta.file_type().is_symlink() {
        return copy_link(src, dest);
    }
    if meta.is_dir() {
        fs::create_dir(dest)?;
        for item in fs::read_dir(src)? {
            let item = item?;
            copy_tree(&item.path(), &dest.join(item.file_name()))?;
        }
        // 最后才设权限：源文件夹可能只读，提前设置就写不进去了。
        fs::set_permissions(dest, meta.permissions())?;
        return Ok(());
    }
    // 管道、设备等不能按内容复制，打开管道还会一直阻塞。
    if !meta.is_file() {
        return Err(AppError::msg(format!(
            "{} {}",
            display_name(src),
            tr("不是普通文件，无法复制", "is not a regular file and cannot be copied")
        )));
    }
    fs::copy(src, dest)?;
    Ok(())
}

#[cfg(unix)]
fn copy_link(src: &Path, dest: &Path) -> Result<()> {
    std::os::unix::fs::symlink(fs::read_link(src)?, dest)?;
    Ok(())
}

#[cfg(not(unix))]
fn copy_link(src: &Path, dest: &Path) -> Result<()> {
    // Windows 上创建符号链接常需额外权限，退化为复制目标内容。
    fs::copy(fs::canonicalize(src)?, dest)?;
    Ok(())
}

/// 删除文件或目录；不跟随符号链接。
fn remove_tree(path: &Path) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.is_dir() {
        fs::remove_dir_all(path)?;
    } else if let Err(e) = fs::remove_file(path) {
        // Windows 的目录符号链接要按目录删除。
        if !(cfg!(windows) && fs::remove_dir(path).is_ok()) {
            return Err(e.into());
        }
    }
    Ok(())
}

// ─── 权限 ───────────────────────────────────────────────────────────────────

pub async fn chmod(paths: Vec<String>, mode: u32, recursive: bool) -> Result<()> {
    off(move || {
        for path in paths {
            chmod_one(&absolute(&path)?, mode & 0o7777, recursive)?;
        }
        Ok(())
    })
    .await
}

/// 与 chmod 命令一致：选中的符号链接改其目标，递归时遇到的符号链接跳过。
fn chmod_one(path: &Path, mode: u32, recursive: bool) -> Result<()> {
    let is_link = fs::symlink_metadata(path)?.file_type().is_symlink();
    if recursive && !is_link {
        return chmod_tree(path, mode);
    }
    fs::set_permissions(path, with_mode(&fs::metadata(path)?, mode))?;
    Ok(())
}

fn chmod_tree(path: &Path, mode: u32) -> Result<()> {
    let meta = fs::symlink_metadata(path)?;
    if meta.file_type().is_symlink() {
        return Ok(());
    }
    if meta.is_dir() {
        // 先保证自己能进入，目标权限再严也能改到子项；结束时才设为目标权限。
        fs::set_permissions(path, with_mode(&meta, mode | 0o700))?;
        for item in fs::read_dir(path)? {
            chmod_tree(&item?.path(), mode)?;
        }
    }
    fs::set_permissions(path, with_mode(&meta, mode))?;
    Ok(())
}

#[cfg(unix)]
fn with_mode(_meta: &fs::Metadata, mode: u32) -> fs::Permissions {
    use std::os::unix::fs::PermissionsExt;
    fs::Permissions::from_mode(mode)
}

/// Windows 只有只读位：去掉写位即设为只读。
#[cfg(not(unix))]
fn with_mode(meta: &fs::Metadata, mode: u32) -> fs::Permissions {
    let mut permissions = meta.permissions();
    permissions.set_readonly(mode & 0o200 == 0);
    permissions
}

#[cfg(unix)]
pub async fn chown(paths: Vec<String>, owner: String, group: String, recursive: bool) -> Result<()> {
    // 换主需要 root，std 无法设置，交给系统 chown。
    let spec = crate::sftp::owner_spec(&owner, &group)?;
    off(move || {
        let mut cmd = command("chown");
        if recursive {
            cmd.arg("-R");
        }
        cmd.arg("--").arg(spec);
        for path in paths {
            cmd.arg(absolute(&path)?);
        }
        run(&mut cmd)
    })
    .await
}

#[cfg(not(unix))]
pub async fn chown(_paths: Vec<String>, _owner: String, _group: String, _recursive: bool) -> Result<()> {
    Err(AppError::msg(tr(
        "此平台不支持修改所有者",
        "Changing the owner is not supported on this platform",
    )))
}

// ─── 文本读写 ───────────────────────────────────────────────────────────────

pub async fn read_text(path: String) -> Result<TextFile> {
    off(move || {
        let p = absolute(&path)?;
        let meta = fs::metadata(&p)?;
        // 读管道会一直阻塞，读 /dev/zero 这类设备则永无止境。
        if !meta.is_file() {
            return Err(AppError::msg(tr(
                "不是普通文件，无法在内置编辑器中打开",
                "Not a regular file; it cannot be opened in the built-in editor",
            )));
        }
        if meta.len() > MAX_TEXT_SIZE {
            return Err(AppError::msg(tr(
                "文件超过 4 MB，内置编辑器无法打开",
                "The file exceeds 4 MB; the built-in editor cannot open it",
            )));
        }
        let content = decode_text(fs::read(&p)?)?;
        Ok(TextFile { content, writable: can_write(&p) })
    })
    .await
}

/// 以写方式试开（不创建不截断）探测可写性，文件不变。
fn can_write(path: &Path) -> bool {
    match fs::OpenOptions::new().write(true).open(path) {
        Ok(_) => true,
        // 只有明确拒绝才视为只读，探测出错不阻止保存。
        Err(e) => e.kind() != std::io::ErrorKind::PermissionDenied,
    }
}

pub async fn write_text(path: String, content: String) -> Result<()> {
    off(move || {
        fs::write(absolute(&path)?, content.as_bytes())?;
        Ok(())
    })
    .await
}

pub async fn du(path: String) -> Result<u64> {
    off(move || Ok(tree_size(&absolute(&path)?))).await
}

/// 递归累计文件大小；符号链接不展开，避免环路。
fn tree_size(path: &Path) -> u64 {
    let Ok(meta) = fs::symlink_metadata(path) else {
        return 0;
    };
    if !meta.is_dir() {
        return meta.len();
    }
    let mut total = 0;
    if let Ok(items) = fs::read_dir(path) {
        for item in items.flatten() {
            total += tree_size(&item.path());
        }
    }
    total
}

// ─── 压缩包 ─────────────────────────────────────────────────────────────────

pub async fn compress(
    dir: String,
    names: Vec<String>,
    archive: String,
    level: Option<u8>,
    remove_source: bool,
) -> Result<()> {
    off(move || compress_sync(&dir, &names, &archive, level, remove_source)).await
}

/// 与远程共用同一套 tar/zip 命令，交本机 sh 执行。
#[cfg(unix)]
fn compress_sync(
    dir: &str,
    names: &[String],
    archive: &str,
    level: Option<u8>,
    remove_source: bool,
) -> Result<()> {
    run_sh(&crate::sftp::compress_command(
        dir,
        names,
        archive,
        level,
        remove_source,
    )?)
}

/// 交给系统 tar.exe（bsdtar，按扩展名自动选择格式，不支持级别）。
#[cfg(not(unix))]
fn compress_sync(
    dir: &str,
    names: &[String],
    archive: &str,
    _level: Option<u8>,
    remove_source: bool,
) -> Result<()> {
    let dir = absolute(dir)?;
    let target = dir.join(archive);
    let _ = fs::remove_file(&target);
    run(command("tar")
        .current_dir(&dir)
        .arg("-a")
        .arg("-c")
        .arg("-f")
        .arg(&target)
        .arg("--")
        .args(names))?;
    if remove_source {
        for name in names.iter().filter(|n| n.as_str() != archive) {
            remove_tree(&dir.join(name))?;
        }
    }
    Ok(())
}

pub async fn extract(path: String, dest_dir: String) -> Result<()> {
    off(move || extract_sync(&path, &dest_dir)).await
}

#[cfg(unix)]
fn extract_sync(path: &str, dest_dir: &str) -> Result<()> {
    run_sh(&crate::sftp::extract_command(path, dest_dir)?)
}

#[cfg(not(unix))]
fn extract_sync(path: &str, dest_dir: &str) -> Result<()> {
    let lower = path.to_lowercase();
    if lower.ends_with(".gz") && !lower.ends_with(".tar.gz") && !lower.ends_with(".tgz") {
        return Err(AppError::msg(tr(
            "此平台不支持解压 .gz 单文件",
            "Extracting a plain .gz file is not supported on this platform",
        )));
    }
    run(command("tar")
        .arg("-x")
        .arg("-f")
        .arg(absolute(path)?)
        .arg("-C")
        .arg(absolute(dest_dir)?))
}

#[cfg(unix)]
fn run_sh(script: &str) -> Result<()> {
    run(command("sh").arg("-c").arg(script))
}

/// 外部命令；Windows 上不弹出控制台窗口（CREATE_NO_WINDOW）。
fn command(program: &str) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        cmd.creation_flags(0x0800_0000);
    }
    cmd
}

/// 运行命令，失败时以 stderr 作为错误信息。
fn run(cmd: &mut Command) -> Result<()> {
    let out = cmd.output()?;
    ExecOutput {
        stdout: String::new(),
        stderr: String::from_utf8_lossy(&out.stderr).into_owned(),
        // 被信号终止时没有退出码，也算失败。
        code: out.status.code().unwrap_or(-1) as u32,
    }
    .check()?;
    Ok(())
}

// ─── 搜索 ───────────────────────────────────────────────────────────────────

pub async fn search(dir: String, pattern: &str) -> Result<Vec<FileEntry>> {
    // 与远程的 find -iname '*pattern*' 同义：不区分大小写，首尾隐含 *。
    let pattern: Vec<char> = format!("*{}*", pattern.to_lowercase()).chars().collect();
    off(move || {
        let root = absolute(&dir)?;
        // 起点不可读要报告；其下不可读的文件夹在 walk 中跳过。
        fs::read_dir(&root)?;
        let mut found = Vec::new();
        walk(&root, &root, &pattern, &mut found, &mut 0);
        Ok(found)
    })
    .await
}

/// 通配符匹配：* 匹配任意串，? 匹配单个字符。
fn wildcard(pattern: &[char], name: &[char]) -> bool {
    let (mut p, mut n) = (0, 0);
    // 最近一个 * 之后的模式位置，及它已吞到的名称位置；失配时回到这里多吞一个字符。
    let mut star = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                p += 1;
                star = Some((p, n));
            }
            Some(&c) if c == '?' || c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => {
                let Some((after, eaten)) = star else {
                    return false;
                };
                p = after;
                n = eaten + 1;
                star = Some((after, n));
            }
        }
    }
    pattern[p..].iter().all(|c| *c == '*')
}

fn walk(root: &Path, dir: &Path, pattern: &[char], found: &mut Vec<FileEntry>, visits: &mut u64) {
    let Ok(items) = fs::read_dir(dir) else {
        return;
    };
    let mut subdirs: Vec<PathBuf> = Vec::new();
    for item in items {
        let Ok(item) = item else { continue };
        *visits += 1;
        if found.len() >= SEARCH_LIMIT || *visits >= SEARCH_VISITS {
            return;
        }
        let name: Vec<char> = item.file_name().to_string_lossy().to_lowercase().chars().collect();
        if wildcard(pattern, &name) {
            let path = item.path();
            // 显示相对路径。
            if let Some(e) = entry_of(&path, text(path.strip_prefix(root).unwrap_or(&path))) {
                found.push(e);
            }
        }
        // file_type 不跟随符号链接，不会进入链接目录，避免重复与环路；也省去逐项 stat。
        if item.file_type().is_ok_and(|t| t.is_dir()) {
            subdirs.push(item.path());
        }
    }
    for path in subdirs {
        walk(root, &path, pattern, found, visits);
        if found.len() >= SEARCH_LIMIT || *visits >= SEARCH_VISITS {
            break;
        }
    }
}
