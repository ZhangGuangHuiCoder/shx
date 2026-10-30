//! 系统托盘：后台运行时把主窗口找回来，或退出应用；并提供“保持唤醒”开关与 Hosts 方案切换。
//!
//! 左键点击切换窗口显隐；菜单含：保持唤醒（可勾选）、定时子菜单、Hosts 子菜单、显示主窗口、退出。
//! 退出交回前端走统一询问流程（事件 `app:quit`），以便照常断开会话与隧道。

use tauri::menu::{CheckMenuItem, IsMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{App, AppHandle, Emitter, Manager};

use crate::awake;
use crate::hosts;
use crate::i18n;

const AWAKE_ID: &str = "awake";
const SHOW_ID: &str = "tray_show";
const QUIT_ID: &str = "tray_quit";
/// Hosts 子菜单中“编辑”项与方案项的 id 前缀。
const HOSTS_EDIT_ID: &str = "hosts_edit";
const HOSTS_PREFIX: &str = "hosts_";
/// 定时时长（秒）。
const DURATIONS: [u64; 6] = [300, 900, 1800, 3600, 7200, 18000];

fn hosts_menu_id(id: &str) -> String {
    format!("{HOSTS_PREFIX}{id}")
}

fn awake_id(secs: u64) -> String {
    format!("{AWAKE_ID}_{secs}")
}

/// 从菜单项 id 解析定时秒数；非定时项返回 None。
fn secs_of(id: &str) -> Option<u64> {
    id.strip_prefix("awake_")?.parse().ok()
}

/// 托盘菜单项，语言切换时更新文字；保持唤醒的勾选态也在此同步。
struct TrayMenu {
    awake: CheckMenuItem<tauri::Wry>,
    durations: Vec<MenuItem<tauri::Wry>>,
    hosts: Submenu<tauri::Wry>,
    show: MenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

/// 在 setup 中构建托盘；无内置图标时跳过（不阻断启动）。
pub fn build(app: &mut App) -> tauri::Result<()> {
    let handle = app.handle().clone();
    let Some(icon) = handle.default_window_icon() else {
        return Ok(());
    };
    let icon = icon.clone();

    let awake_item = CheckMenuItem::with_id(
        &handle,
        AWAKE_ID,
        label_awake(),
        true,
        awake::is_active(),
        None::<&str>,
    )?;
    let durations: Vec<MenuItem<tauri::Wry>> = DURATIONS
        .iter()
        .map(|secs| MenuItem::with_id(&handle, awake_id(*secs), label_duration(*secs), true, None::<&str>))
        .collect::<tauri::Result<_>>()?;
    let duration_refs: Vec<&dyn IsMenuItem<tauri::Wry>> = durations.iter().map(|item| item as &dyn IsMenuItem<tauri::Wry>).collect();
    let timer = Submenu::with_items(&handle, label_timer(), true, &duration_refs)?;
    let hosts_submenu = Submenu::with_items(&handle, label_hosts(), true, &[])?;
    let show = MenuItem::with_id(&handle, SHOW_ID, label_show(), true, None::<&str>)?;
    let quit = MenuItem::with_id(&handle, QUIT_ID, label_quit(), true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(&handle)?;
    let menu = Menu::with_items(&handle, &[&awake_item, &timer, &hosts_submenu, &separator, &show, &quit])?;

    TrayIconBuilder::with_id("main")
        .icon(icon)
        .menu(&menu)
        .show_menu_on_left_click(false)
        .tooltip("Shell Xplore")
        .on_menu_event(|app, event| {
            let id = event.id.as_ref();
            if id == HOSTS_EDIT_ID {
                hosts::open_editor(app);
                return;
            }
            if let Some(scheme) = id.strip_prefix(HOSTS_PREFIX) {
                let (app, scheme) = (app.clone(), scheme.to_owned());
                tauri::async_runtime::spawn(async move {
                    let _ = hosts::toggle(app, scheme).await;
                });
                return;
            }
            match id {
                AWAKE_ID => awake::toggle(app),
                SHOW_ID => reveal(app),
                QUIT_ID => {
                    let _ = app.emit("app:quit", ());
                }
                _ => {
                    if let Some(secs) = secs_of(id) {
                        awake::enable_for(app, secs);
                    }
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                toggle(tray.app_handle());
            }
        })
        .build(&handle)?;

    handle.manage(TrayMenu {
        awake: awake_item,
        durations,
        hosts: hosts_submenu,
        show,
        quit,
    });
    // 方案列表随后端加载状态填充。
    set_hosts(&handle);
    Ok(())
}

/// 同步“保持唤醒”勾选态；菜单操作需在主线程执行。
pub(crate) fn refresh(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        if let Some(state) = handle.try_state::<TrayMenu>() {
            let _ = state.awake.set_checked(awake::is_active());
            let _ = state.awake.set_text(label_awake());
        }
    });
}

/// 语言切换后更新菜单文字；同样派发到主线程。
pub fn set_labels(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(state) = handle.try_state::<TrayMenu>() else { return };
        let _ = state.awake.set_text(label_awake());
        for (item, secs) in state.durations.iter().zip(DURATIONS) {
            let _ = item.set_text(label_duration(secs));
        }
        let _ = state.show.set_text(label_show());
        let _ = state.quit.set_text(label_quit());
    });
    // Hosts 子菜单标题与“编辑”项文字随语言重建。
    set_hosts(app);
}

/// 重建 Hosts 子菜单：每个方案一个可勾选项，末尾接“编辑 Hosts…”；需在主线程操作菜单。
pub fn set_hosts(app: &AppHandle) {
    let handle = app.clone();
    let _ = app.run_on_main_thread(move || {
        let Some(state) = handle.try_state::<TrayMenu>() else { return };
        let submenu = &state.hosts;
        let _ = submenu.set_text(label_hosts());
        while submenu.remove_at(0).ok().flatten().is_some() {}
        let schemes: Vec<CheckMenuItem<tauri::Wry>> = hosts::schemes(&handle)
            .into_iter()
            .map(|scheme| CheckMenuItem::with_id(&handle, hosts_menu_id(&scheme.id), scheme.name, true, scheme.enabled, None::<&str>))
            .collect::<tauri::Result<_>>()
            .unwrap_or_default();
        let Ok(separator) = PredefinedMenuItem::separator(&handle) else { return };
        let Ok(edit) = MenuItem::with_id(&handle, HOSTS_EDIT_ID, label_hosts_edit(), true, None::<&str>) else { return };
        let mut refs: Vec<&dyn IsMenuItem<tauri::Wry>> = schemes.iter().map(|item| item as &dyn IsMenuItem<tauri::Wry>).collect();
        refs.push(&separator as &dyn IsMenuItem<tauri::Wry>);
        refs.push(&edit as &dyn IsMenuItem<tauri::Wry>);
        let _ = submenu.append_items(&refs);
    });
}

fn label_awake() -> String {
    let base = i18n::tr("保持唤醒", "Keep Awake");
    if !awake::is_active() {
        return base.to_owned();
    }
    match awake::remaining() {
        Some(left) => {
            let (m, s) = (left / 60, left % 60);
            if i18n::is_english() {
                format!("{base} · {m}:{s:02} left")
            } else {
                format!("{base} · 剩余 {m}:{s:02}")
            }
        }
        None if i18n::is_english() => format!("{base} · Indefinite"),
        None => format!("{base} · 无限期"),
    }
}

fn label_timer() -> String {
    i18n::tr("保持唤醒时长", "Keep Awake For").to_owned()
}

fn label_hosts() -> String {
    i18n::tr("Hosts 管理", "Hosts").to_owned()
}

fn label_hosts_edit() -> String {
    i18n::tr("编辑 Hosts…", "Edit Hosts…").to_owned()
}

fn label_show() -> String {
    i18n::tr("显示主窗口", "Show Window").to_owned()
}

fn label_quit() -> String {
    i18n::tr("退出", "Quit").to_owned()
}

fn label_duration(secs: u64) -> String {
    if i18n::is_english() {
        if secs % 3600 == 0 {
            let h = secs / 3600;
            format!("{h} hour{}", if h == 1 { "" } else { "s" })
        } else {
            format!("{} min", secs / 60)
        }
    } else if secs % 3600 == 0 {
        format!("{} 小时", secs / 3600)
    } else {
        format!("{} 分钟", secs / 60)
    }
}

pub(crate) fn reveal(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.show();
        let _ = window.set_focus();
    }
}

fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window("main") else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        reveal(app);
    }
}
