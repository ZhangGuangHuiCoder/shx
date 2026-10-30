//! Hosts 方案管理：多个命名方案，勾选启用，合并写入系统 hosts（需提权）。
//!
//! 方案存于应用配置目录的 hosts.json（后端为唯一数据源，托盘与对话框共用）。
//! 写入系统 hosts 时只替换本模块用标记包裹的托管块，其余原有内容保持不变。
//! 提权：macOS 用 osascript 以管理员身份 cp；Windows 以 `--hosts-write` 重启自身提权复制；其他平台直接写。

use std::path::PathBuf;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, State};

use crate::error::{AppError, Result};
use crate::i18n::tr;

/// 一个 hosts 方案；lines 为正文，每行 “IP 域名” 或注释，不含方案名标题。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostScheme {
    pub id: String,
    pub name: String,
    pub enabled: bool,
    pub lines: String,
}

/// 托管块标记，成对出现；块外内容一律保留。
const BEGIN: &str = "# >>> Shell Xplore hosts (managed) >>>";
const END: &str = "# <<< Shell Xplore hosts (managed) <<<";

/// 后端持有的 hosts 状态：方案列表与其持久化文件路径。
pub struct HostsState {
    file: PathBuf,
    schemes: Mutex<Vec<HostScheme>>,
}

/// 系统 hosts 文件路径。
fn system_path() -> PathBuf {
    #[cfg(windows)]
    {
        let root = std::env::var("SystemRoot").unwrap_or_else(|_| r"C:\Windows".into());
        std::path::Path::new(&root).join("System32").join("drivers").join("etc").join("hosts")
    }
    #[cfg(not(windows))]
    {
        PathBuf::from("/etc/hosts")
    }
}

/// 在 setup 中加载方案并纳入托管状态；须在 tray::build 之前调用。
pub fn setup(app: &mut tauri::App) -> Result<()> {
    let dir = app.handle().path().app_config_dir().map_err(|e| AppError::msg(e.to_string()))?;
    std::fs::create_dir_all(&dir).ok();
    let file = dir.join("hosts.json");
    let schemes = std::fs::read_to_string(&file)
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default();
    app.manage(HostsState { file, schemes: Mutex::new(schemes) });
    Ok(())
}

fn persist(state: &HostsState, schemes: &[HostScheme]) {
    if let Ok(json) = serde_json::to_string_pretty(schemes) {
        let _ = std::fs::write(&state.file, json);
    }
}

/// 当前方案列表（供托盘构建菜单）。
pub fn schemes(app: &AppHandle) -> Vec<HostScheme> {
    app.try_state::<HostsState>().map(|s| s.schemes.lock().clone()).unwrap_or_default()
}

/// 启用方案渲染为托管块正文；每段前加 “# [方案名]”。
pub fn render(schemes: &[HostScheme]) -> String {
    let mut out = String::new();
    for scheme in schemes.iter().filter(|s| s.enabled) {
        let body = scheme.lines.trim();
        if body.is_empty() {
            continue;
        }
        out.push_str(&format!("\n# [{}]\n{body}\n", scheme.name));
    }
    out
}

/// 去掉既有托管块，返回可安全重新拼接的基础内容。
fn strip(text: &str) -> String {
    let mut out = String::new();
    let mut skipping = false;
    for line in text.lines() {
        let trimmed = line.trim_end();
        if trimmed == BEGIN {
            skipping = true;
            continue;
        }
        if trimmed == END {
            skipping = false;
            continue;
        }
        if !skipping {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// 生成写入系统 hosts 的完整内容：原内容（去旧块）+ 新托管块。
pub fn merged(current: &str, schemes: &[HostScheme]) -> String {
    let base = strip(current);
    let mut out = base.trim_end().to_string();
    let block = render(schemes);
    if !block.is_empty() {
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(&format!("\n{BEGIN}\n{}\n{END}\n", block.trim_end()));
    } else if !out.is_empty() {
        out.push('\n');
    }
    out
}

/// 用当前启用的方案重写系统 hosts；无变化时跳过（避免多余授权）。
async fn apply(app: &AppHandle) -> Result<()> {
    let enabled = schemes(app);
    let target = system_path();
    let current = tokio::fs::read_to_string(&target).await.unwrap_or_default();
    let content = merged(&current, &enabled);
    if content.trim_end() == current.trim_end() {
        return Ok(());
    }
    write_privileged(content, target).await
}

fn temp_file(content: &str) -> Result<PathBuf> {
    let path = std::env::temp_dir().join(format!("shx-hosts-{}", uuid::Uuid::new_v4()));
    std::fs::write(&path, content)?;
    Ok(path)
}

// ---- 提权写入 ----

#[cfg(target_os = "macos")]
fn applescript_string(text: &str) -> String {
    format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
}

#[cfg(target_os = "macos")]
async fn write_privileged(content: String, target: PathBuf) -> Result<()> {
    use std::process::Command;

    use crate::ssh::sh_quote;

    let tmp = temp_file(&content)?;
    let command = format!("/bin/cp {} {}", sh_quote(&tmp.to_string_lossy()), sh_quote(&target.to_string_lossy()));
    let prompt = tr("Shell Xplore 需要管理员权限以写入 hosts 文件。", "Shell Xplore requires administrator privileges to write the hosts file.");
    let script = format!(
        "do shell script {} with prompt {} with administrator privileges",
        applescript_string(&command),
        applescript_string(prompt)
    );
    let run = tokio::task::spawn_blocking(move || Command::new("/usr/bin/osascript").args(["-e", &script]).output());
    let output = run.await.map_err(|e| AppError::msg(e.to_string()))??;
    let _ = std::fs::remove_file(&tmp);
    if output.status.success() {
        return Ok(());
    }
    let detail = String::from_utf8_lossy(&output.stderr);
    // 用户在授权框取消。
    if detail.contains("-128") {
        return Err(AppError::Cancelled);
    }
    Err(AppError::msg(format!("{}{}", tr("未获得管理员权限：", "Administrator privileges were not granted: "), detail.trim())))
}

#[cfg(windows)]
fn elevate_and_wait(exe: &std::ffi::OsStr, parameters: &str) -> Result<u32> {
    use std::ffi::OsStr;
    use std::os::windows::ffi::OsStrExt;

    use windows_sys::Win32::Foundation::{CloseHandle, ERROR_CANCELLED, HANDLE, INFINITE};
    use windows_sys::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject};
    use windows_sys::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows_sys::Win32::UI::WindowsAndMessaging::SW_HIDE;

    let wide = |text: &OsStr| text.encode_wide().chain(std::iter::once(0)).collect::<Vec<_>>();
    let (verb, file, params) = (wide(OsStr::new("runas")), wide(exe), wide(OsStr::new(parameters)));
    // SAFETY: 结构体只含整数与指针，全零合法。
    let mut info: SHELLEXECUTEINFOW = unsafe { std::mem::zeroed() };
    info.cbSize = size_of::<SHELLEXECUTEINFOW>() as u32;
    info.fMask = SEE_MASK_NOCLOSEPROCESS;
    info.lpVerb = verb.as_ptr();
    info.lpFile = file.as_ptr();
    info.lpParameters = params.as_ptr();
    info.nShow = SW_HIDE;
    // SAFETY: info 已设大小，指针指向调用期间有效的零结尾字符串。
    if unsafe { ShellExecuteExW(&mut info) } == 0 {
        let error = std::io::Error::last_os_error();
        if error.raw_os_error() == Some(ERROR_CANCELLED as i32) {
            return Err(AppError::Cancelled);
        }
        return Err(AppError::msg(error.to_string()));
    }
    let process: HANDLE = info.hProcess;
    if process.is_null() {
        return Err(AppError::msg(tr("提权进程启动失败", "The elevated process failed to start")));
    }
    // SAFETY: hProcess 为 ShellExecute 返回的有效句柄，等待后取退出码并关闭。
    let code = unsafe {
        WaitForSingleObject(process, INFINITE);
        let mut code = 0u32;
        GetExitCodeProcess(process, &mut code);
        CloseHandle(process);
        code
    };
    Ok(code)
}

#[cfg(windows)]
async fn write_privileged(content: String, target: PathBuf) -> Result<()> {
    let tmp = temp_file(&content)?;
    let exe = std::env::current_exe()?;
    let parameters = format!("--hosts-write \"{}\" \"{}\"", tmp.to_string_lossy(), target.to_string_lossy());
    let code = tokio::task::spawn_blocking(move || elevate_and_wait(exe.as_os_str(), &parameters))
        .await
        .map_err(|e| AppError::msg(e.to_string()))??;
    let _ = std::fs::remove_file(&tmp);
    if code == 0 {
        return Ok(());
    }
    Err(AppError::msg(tr("写入 hosts 失败或被拒绝授权", "Writing hosts failed or was denied")))
}

/// 提权复制子命令：以管理员身份重启自身时执行，仅复制后退出。
#[cfg(windows)]
pub fn dispatch() -> Option<i32> {
    let mut args = std::env::args().skip(1);
    if args.next()?.as_str() != "--hosts-write" {
        return None;
    }
    let (tmp, target) = (args.next()?, args.next()?);
    let code = match std::fs::copy(&tmp, &target) {
        Ok(_) => 0,
        Err(_) => 1,
    };
    let _ = std::fs::remove_file(&tmp);
    Some(code)
}

#[cfg(not(any(target_os = "macos", windows)))]
async fn write_privileged(content: String, target: PathBuf) -> Result<()> {
    tokio::fs::write(&target, content)
        .await
        .map_err(|e| AppError::msg(format!("{}{e}", tr("写入 hosts 失败（可能需要 root 权限）：", "Failed to write hosts (may require root): "))))
}

// ---- 前端命令 ----

#[tauri::command]
pub fn hosts_list(state: State<'_, HostsState>) -> Vec<HostScheme> {
    state.schemes.lock().clone()
}

#[tauri::command]
pub fn hosts_path() -> String {
    system_path().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn hosts_read() -> String {
    std::fs::read_to_string(system_path()).unwrap_or_default()
}

#[tauri::command]
pub async fn hosts_save(app: AppHandle, schemes: Vec<HostScheme>) -> Result<()> {
    if let Some(state) = app.try_state::<HostsState>() {
        *state.schemes.lock() = schemes.clone();
        persist(&state, &schemes);
    }
    crate::tray::set_hosts(&app);
    apply(&app).await
}

#[tauri::command]
pub async fn hosts_revert() -> Result<()> {
    let target = system_path();
    let current = tokio::fs::read_to_string(&target).await.unwrap_or_default();
    write_privileged(merged(&current, &[]), target).await
}

/// 切换某方案启用态：供前端命令与托盘菜单共用。
pub async fn toggle(app: AppHandle, id: String) -> Result<()> {
    if let Some(state) = app.try_state::<HostsState>() {
        let mut schemes = state.schemes.lock();
        if let Some(scheme) = schemes.iter_mut().find(|s| s.id == id) {
            scheme.enabled = !scheme.enabled;
        }
        let snapshot = schemes.clone();
        persist(&state, &snapshot);
    }
    crate::tray::set_hosts(&app);
    let result = apply(&app).await;
    // 通知前端刷新列表（启用态可能已变）。
    let _ = app.emit("hosts:changed", ());
    result
}

#[tauri::command]
pub async fn hosts_toggle(app: AppHandle, id: String) -> Result<()> {
    toggle(app, id).await
}

/// 托盘“编辑 Hosts…”：唤回主窗口并请求前端打开对话框。
pub fn open_editor(app: &AppHandle) {
    crate::tray::reveal(app);
    let _ = app.emit("hosts:edit", ());
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scheme(name: &str, enabled: bool, lines: &str) -> HostScheme {
        HostScheme { id: name.to_owned(), name: name.to_owned(), enabled, lines: lines.to_owned() }
    }

    #[test]
    fn only_enabled_non_empty_schemes_are_rendered() {
        let schemes = [scheme("dev", true, "10.0.0.1 a.test\n"), scheme("off", false, "10.0.0.2 b.test"), scheme("empty", true, "  \n ")];
        let block = render(&schemes);
        assert!(block.contains("# [dev]") && block.contains("10.0.0.1 a.test"));
        assert!(!block.contains("b.test") && !block.contains("# [empty]"));
    }

    #[test]
    fn merged_wraps_block_and_preserves_base() {
        let current = "127.0.0.1 localhost\n::1 localhost\n";
        let out = merged(current, &[scheme("dev", true, "10.0.0.1 a.test")]);
        assert!(out.starts_with("127.0.0.1 localhost\n::1 localhost\n"));
        assert!(out.contains(BEGIN) && out.contains(END));
        assert!(out.contains("10.0.0.1 a.test"));
    }

    #[test]
    fn stripping_then_remerging_is_idempotent() {
        let base = "127.0.0.1 localhost\n";
        let schemes = [scheme("dev", true, "10.0.0.1 a.test")];
        let once = merged(base, &schemes);
        let twice = merged(&once, &schemes);
        assert_eq!(once, twice, "re-applying must not duplicate the block");
        // 剥离托管块后回到原内容（托管块前会留一个空行分隔，故按 trim 比较）。
        assert_eq!(strip(&once).trim_end(), strip(base).trim_end());
    }

    #[test]
    fn disabling_every_scheme_removes_the_block() {
        let with = merged("127.0.0.1 localhost\n", &[scheme("dev", true, "10.0.0.1 a.test")]);
        let reverted = merged(&with, &[]);
        assert_eq!(reverted, "127.0.0.1 localhost\n");
        assert!(!reverted.contains(BEGIN) && !reverted.contains("a.test"));
    }
}
