//! 会话保持：shell 运行在服务器的 tmux 会话里，连接断开后仍然存活，重连即可接着用。
//!
//! tmux 对用户不可见，也不干扰用户自己的 tmux：使用独立的 tmux 服务（`-L shx`），
//! 不显示状态栏，不使用备用屏幕，输出像没有 tmux 时一样进入终端的回滚缓冲区。

use serde::Serialize;
use tauri::State;

use crate::error::{AppError, Result};
use crate::i18n::tr;
use crate::ssh::{sh_quote, AppState};

/// 通过 SSH 执行的非登录 shell 命令只有最基本的 PATH，tmux 往往装在这些目录里。
const PATH: &str = r#"PATH="$PATH:/usr/local/bin:/opt/homebrew/bin:/snap/bin""#;

#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeepAlive {
    /// tmux 已安装且会话存在，可以接入。
    ready: bool,
    /// 会话是刚创建的：里面是新 shell，而不是保持下来的旧 shell。
    fresh: bool,
    /// 没有 tmux 时的安装命令；仅在识别出系统的包管理器时提供。
    install: Option<String>,
}

/// 服务器上留着的一个保持会话。
#[derive(Serialize, Debug, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct KeptSession {
    name: String,
    /// 创建时间和最后一次有输出的时间（Unix 秒）。
    created: u64,
    activity: u64,
    /// 此刻有终端接在上面。
    attached: bool,
    /// 里面正在运行的程序，空闲时是 shell 本身。
    command: String,
    /// 它所在的目录。
    path: String,
}

/// 会话名来自前端，并且会拼进 shell 命令。
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

/// 包装 POSIX 脚本，使其不受用户登录 shell 的影响。
fn sh(script: &str) -> String {
    format!("sh -c {}", sh_quote(script))
}

/// 查找 tmux，找到后确保会话存在。每一步用一行 `shx:` 报告结果，命令的其他输出一律忽略。
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

/// 列出 shx 的 tmux 服务里的全部会话，每个一行 `shx:`。没有 tmux、没有会话时什么都不输出。
/// 目录放在最后：只有它可能含有分隔符。
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

/// 终端用来代替登录 shell、进入会话的命令。`-u`：这样的命令没有 locale，
/// 需要明确告诉 tmux 终端使用 UTF-8；`-d`：踢掉服务器尚未发现已断开的旧客户端。
pub fn attach_command(name: &str) -> Result<String> {
    check_name(name)?;
    Ok(sh(&format!("{PATH}; exec tmux -u -L shx attach-session -d -t {name}")))
}

/// 结束会话及其中运行的一切。会话已不存在也算成功。
fn end_command(name: &str) -> Result<String> {
    check_name(name)?;
    Ok(sh(&format!(
        "{PATH}; tmux -L shx kill-session -t {name} 2>/dev/null || ! tmux -L shx has-session -t {name} 2>/dev/null"
    )))
}

/// 用给定的包管理器安装 tmux 的命令，以 root 身份或通过 sudo 执行。
fn install_command(manager: &str, root: bool) -> Option<String> {
    let steps: &[&str] = match manager {
        "apt-get" => &["apt-get update", "apt-get install -y tmux"],
        "dnf" => &["dnf install -y tmux"],
        "yum" => &["yum install -y tmux"],
        "zypper" => &["zypper install -y tmux"],
        "pacman" => &["pacman -S --noconfirm tmux"],
        "apk" => &["apk add tmux"],
        // Homebrew 拒绝以 root 身份运行。
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

/// 让服务器为名为 `name` 的保持会话做好准备；新建会话时使用给定的尺寸。
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

/// 服务器上留着的保持会话：所有主机配置的都在内，由前端挑出属于当前主机的。
#[tauri::command]
pub async fn keepalive_list(state: State<'_, AppState>, id: String) -> Result<Vec<KeptSession>> {
    let out = state.session(&id)?.exec(&sh(&list_script())).await?;
    Ok(parse_list(&out.stdout))
}

/// 结束服务器上的保持会话 `name`，用于彻底关闭的标签页。
#[tauri::command]
pub async fn keepalive_end(state: State<'_, AppState>, id: String, name: String) -> Result<()> {
    let command = end_command(&name)?;
    let out = state.session(&id)?.exec(&command).await?;
    if out.code == 0 {
        return Ok(());
    }
    Err(AppError::msg(tr("无法结束 tmux 会话", "Could not end the tmux session")))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_plain_names_reach_the_shell() {
        assert!(check_name("shx-1-0a1b2c3d").is_ok());
        for bad in ["", "a b", "a;rm -rf /", "a'b", "$(id)", "a.b", "a:b", &"x".repeat(65)] {
            assert!(check_name(bad).is_err(), "{bad:?}");
            assert!(attach_command(bad).is_err(), "{bad:?}");
            assert!(end_command(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn attach_runs_tmux_through_sh() {
        assert_eq!(
            attach_command("shx-1-0a1b2c3d").unwrap(),
            r#"sh -c 'PATH="$PATH:/usr/local/bin:/opt/homebrew/bin:/snap/bin"; exec tmux -u -L shx attach-session -d -t shx-1-0a1b2c3d'"#
        );
    }

    #[test]
    fn ending_kills_the_session_and_tolerates_one_already_gone() {
        assert_eq!(
            end_command("shx-1-0a1b2c3d").unwrap(),
            r#"sh -c 'PATH="$PATH:/usr/local/bin:/opt/homebrew/bin:/snap/bin"; tmux -L shx kill-session -t shx-1-0a1b2c3d 2>/dev/null || ! tmux -L shx has-session -t shx-1-0a1b2c3d 2>/dev/null'"#
        );
    }

    #[test]
    fn the_prepare_script_survives_quoting() {
        // 脚本本身带有单引号；包进 `sh -c` 之后仍须是一个引号配对的 shell 单词。
        let wrapped = sh(&prepare_script("shx-1-0a1b2c3d", 120, 40));
        assert!(wrapped.starts_with("sh -c '") && wrapped.ends_with('\''));
        assert!(wrapped.contains("new-session -d -s shx-1-0a1b2c3d -x 120 -y 40"));
        assert_eq!(wrapped.matches('\'').count() % 2, 0);
    }

    #[test]
    fn kept_sessions_are_listed_with_what_runs_in_them() {
        let wrapped = sh(&list_script());
        assert!(wrapped.contains("list-sessions -F"));
        assert!(wrapped.contains("#{session_name}|#{session_created}|#{session_activity}|#{session_attached}|#{pane_current_command}|#{pane_current_path}"));
        assert_eq!(wrapped.matches('\'').count() % 2, 0);

        let out = "Last login: today\nshx:shx-1-0a1b2c3d|1760000000|1760000600|1|vim|/var/www\nshx:shx-3-0a1b2c3d|1760000100|1760000200|0|bash|/data/a|b\r\n";
        assert_eq!(
            parse_list(out),
            vec![
                KeptSession {
                    name: "shx-1-0a1b2c3d".into(),
                    created: 1760000000,
                    activity: 1760000600,
                    attached: true,
                    command: "vim".into(),
                    path: "/var/www".into(),
                },
                KeptSession {
                    name: "shx-3-0a1b2c3d".into(),
                    created: 1760000100,
                    activity: 1760000200,
                    attached: false,
                    command: "bash".into(),
                    path: "/data/a|b".into(),
                },
            ]
        );
        // 不完整的行、名字不像会话名的行一律丢掉；没有输出就是没有会话。
        assert_eq!(parse_list("shx:shx-1-0a1b2c3d|x|1|0|bash|/\nshx:a b|1|1|0|bash|/\nshx:shx-2-0a1b2c3d|1|1\n"), vec![]);
        assert_eq!(parse_list(""), vec![]);
    }

    #[test]
    fn reports_are_read_whatever_else_is_printed() {
        let alive = parse("Last login: today\nshx:alive\n", "").unwrap();
        assert_eq!(alive, KeepAlive { ready: true, fresh: false, install: None });
        let fresh = parse("shx:fresh\r\n", "").unwrap();
        assert_eq!(fresh, KeepAlive { ready: true, fresh: true, install: None });
        assert!(parse("shx:failed\n", "no space left").unwrap_err().to_string().contains("no space left"));
        assert!(parse("", "").is_err());
    }

    #[test]
    fn a_missing_tmux_comes_with_how_to_install_it() {
        let install = |out: &str| parse(out, "").unwrap().install;
        assert_eq!(install("shx:missing\nshx:manager=dnf\nshx:uid=0\n").as_deref(), Some("dnf install -y tmux"));
        assert_eq!(
            install("shx:missing\nshx:manager=apt-get\nshx:uid=1000\n").as_deref(),
            Some("sudo apt-get update && sudo apt-get install -y tmux")
        );
        assert_eq!(install("shx:missing\nshx:manager=brew\nshx:uid=501\n").as_deref(), Some("brew install tmux"));
        // 没有认出包管理器：给不出安装命令，但仍应得到干净的“未就绪”结果。
        assert_eq!(parse("shx:missing\nshx:uid=0\n", "").unwrap(), KeepAlive { ready: false, fresh: false, install: None });
    }
}
