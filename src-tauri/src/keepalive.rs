//! 会话保活：shell 运行在服务器 tmux 中，断线后仍存活。
//!
//! 使用独立的 tmux 服务（`-L shx`），不干扰用户自己的 tmux；关闭状态栏和备用屏幕，输出照常进入回滚缓冲区。

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::ssh::{sh_quote, AppState};

/// 非登录 shell 的 PATH 很简陋，补上 tmux 常见的安装目录。
const PATH: &str = r#"PATH="$PATH:/usr/local/bin:/opt/homebrew/bin:/snap/bin""#;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeepAlive {
    ready: bool,
    /// 会话为新建。
    fresh: bool,
    /// 缺少 tmux 时的安装命令，无法识别包管理器时为 None。
    install: Option<String>,
}

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeptSession {
    name: String,
    /// Unix 秒。
    created: u64,
    activity: u64,
    attached: bool,
    /// 正在运行的程序，空闲时为 shell。
    command: String,
    path: String,
}

/// 会话名来自前端且会拼入 shell 命令，须校验。
fn check_name(name: &str) -> Result<()> {
    let ok = !name.is_empty()
        && name.len() <= 64
        && name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_');
    if ok {
        Ok(())
    } else {
        Err(AppError::msg(tr("无效的保活会话名", "Invalid keep-alive session name")))
    }
}

/// 用 sh 执行，不受用户登录 shell 影响。
fn sh(script: &str) -> String {
    format!("sh -c {}", sh_quote(script))
}

/// 查找 tmux 并确保会话存在；结果以 `shx:` 行报告，其他输出忽略。
fn prepare_script(name: &str, cols: u32, rows: u32) -> String {
    format!(
        r#"{PATH}
if command -v tmux >/dev/null 2>&1; then
  if tmux -L shx has-session -t {name} 2>/dev/null; then
    echo shx:alive
  elif tmux -L shx new-session -d -s {name} -x {cols} -y {rows}; then
    echo shx:fresh
    tmux -L shx set-option -g -a terminal-overrides ',xterm*:smcup@:rmcup@' >/dev/null 2>&1
    tmux -L shx set-option -g -q allow-passthrough on >/dev/null 2>&1
  else
    echo shx:failed
  fi
  tmux -L shx set-option -t {name} status off >/dev/null 2>&1
else
  echo shx:missing
  for m in apt-get dnf yum zypper pacman apk brew; do
    if command -v "$m" >/dev/null 2>&1; then echo "shx:manager=$m"; break; fi
  done
  echo "shx:uid=$(id -u)"
fi"#
    )
}

/// 列出全部会话，每行一个 `shx:`。目录放最后，因为只有它可能含分隔符。
fn list_script() -> String {
    format!(
        "{PATH}; tmux -L shx list-sessions -F \
         'shx:#{{session_name}}|#{{session_created}}|#{{session_activity}}|#{{session_attached}}|#{{pane_current_command}}|#{{pane_current_path}}' \
         2>/dev/null"
    )
}

fn parse_list(stdout: &str) -> Vec<KeptSession> {
    let session = |line: &str| {
        let mut fields = line.trim().strip_prefix("shx:")?.splitn(6, '|');
        let name = fields.next()?;
        check_name(name).ok()?;
        Some(KeptSession {
            name: name.to_string(),
            created: fields.next()?.parse().ok()?,
            activity: fields.next()?.parse().ok()?,
            attached: fields.next()? != "0",
            command: fields.next()?.to_string(),
            path: fields.next()?.to_string(),
        })
    };
    stdout.lines().filter_map(session).collect()
}

/// 接入会话的命令。-u：无 locale 时强制 UTF-8；-d：踢掉未察觉断开的旧客户端。
pub fn attach_command(name: &str) -> Result<String> {
    check_name(name)?;
    Ok(sh(&format!("{PATH}; exec tmux -u -L shx attach-session -d -t {name}")))
}

/// 结束会话；会话不存在也视为成功。
fn end_command(name: &str) -> Result<String> {
    check_name(name)?;
    Ok(sh(&format!(
        "{PATH}; tmux -L shx kill-session -t {name} 2>/dev/null || ! tmux -L shx has-session -t {name} 2>/dev/null"
    )))
}

/// 以 root 或 sudo 安装 tmux 的命令。
fn install_command(manager: &str, root: bool) -> Option<String> {
    let steps: &[&str] = match manager {
        "apt-get" => &["apt-get update", "apt-get install -y tmux"],
        "dnf" => &["dnf install -y tmux"],
        "yum" => &["yum install -y tmux"],
        "zypper" => &["zypper install -y tmux"],
        "pacman" => &["pacman -S --noconfirm tmux"],
        "apk" => &["apk add tmux"],
        // Homebrew 拒绝以 root 运行。
        "brew" => return Some("brew install tmux".into()),
        _ => return None,
    };
    let sudo = if root { "" } else { "sudo " };
    Some(steps.iter().map(|step| format!("{sudo}{step}")).collect::<Vec<_>>().join(" && "))
}

fn parse(stdout: &str, stderr: &str) -> Result<KeepAlive> {
    let reports: Vec<&str> = stdout.lines().filter_map(|line| line.trim().strip_prefix("shx:")).collect();
    let value = |key: &str| reports.iter().find_map(|r| r.strip_prefix(key)?.strip_prefix('='));
    let has = |word: &str| reports.contains(&word);

    if has("alive") || has("fresh") {
        return Ok(KeepAlive { ready: true, fresh: has("fresh"), install: None });
    }
    if has("missing") {
        let root = value("uid") == Some("0");
        let install = value("manager").and_then(|manager| install_command(manager, root));
        return Ok(KeepAlive { ready: false, fresh: false, install });
    }
    let detail = stderr.trim();
    let what = tr("无法创建 tmux 会话", "Could not create the tmux session");
    Err(AppError::msg(if detail.is_empty() { what.to_string() } else { format!("{what}: {detail}") }))
}

/// 准备保活会话，新建时使用给定尺寸。
#[tauri::command]
pub async fn keepalive_prepare(
    state: State<'_, AppState>,
    id: String,
    name: String,
    cols: u32,
    rows: u32,
) -> Result<KeepAlive> {
    check_name(&name)?;
    let session = state.session(&id)?;
    let out = session.exec(&sh(&prepare_script(&name, cols.clamp(20, 500), rows.clamp(5, 300)))).await?;
    parse(&out.stdout, &out.stderr)
}

/// 列出所有保活会话，由前端筛选属于当前主机的。
#[tauri::command]
pub async fn keepalive_list(state: State<'_, AppState>, id: String) -> Result<Vec<KeptSession>> {
    let out = state.session(&id)?.exec(&sh(&list_script())).await?;
    Ok(parse_list(&out.stdout))
}

#[tauri::command]
pub async fn keepalive_end(state: State<'_, AppState>, id: String, name: String) -> Result<()> {
    let command = end_command(&name)?;
    let out = state.session(&id)?.exec(&command).await?;
    if out.code == 0 {
        return Ok(());
    }
    Err(AppError::msg(tr("无法结束 tmux 会话", "Could not end the tmux session")))
}
