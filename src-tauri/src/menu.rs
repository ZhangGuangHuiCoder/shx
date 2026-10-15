//! 处理 macOS 往应用菜单追加的系统项。
//!
//! 听写、表情项可通过偏好设置关闭（[`disable_system_items`]），其余由 [`menu_trim`] 删除。
//! muda 关闭了菜单的自动启用，[`menu_autoenable`] 将其交还 AppKit 判断。

use tauri::AppHandle;

/// 必须在应用启动前执行。
pub fn disable_system_items() {
    #[cfg(target_os = "macos")]
    {
        use objc2_foundation::{ns_string, NSUserDefaults};

        let defaults = NSUserDefaults::standardUserDefaults();
        defaults.setBool_forKey(true, ns_string!("NSDisabledDictationMenuItem"));
        defaults.setBool_forKey(true, ns_string!("NSDisabledCharacterPaletteMenuItem"));
    }
}

#[cfg(target_os = "macos")]
fn with_menu(title: &str, f: impl Fn(&objc2_app_kit::NSMenu)) {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSApplication;

    let Some(mtm) = MainThreadMarker::new() else { return };
    let Some(bar) = NSApplication::sharedApplication(mtm).mainMenu() else { return };
    let items = bar.itemArray();
    for index in 0..items.count() {
        let item = items.objectAtIndex(index);
        if item.title().to_string() != title {
            continue;
        }
        if let Some(menu) = item.submenu() {
            f(&menu);
        }
    }
}

#[cfg(target_os = "macos")]
fn trim(title: &str, keep: isize) {
    with_menu(title, |menu| {
        while menu.numberOfItems() > keep {
            menu.removeItemAtIndex(menu.numberOfItems() - 1);
        }
    });
}

/// 截断顶层菜单，去掉系统追加的项。
#[tauri::command]
pub fn menu_trim(app: AppHandle, title: String, keep: usize) {
    #[cfg(target_os = "macos")]
    let _ = app.run_on_main_thread(move || trim(&title, keep as isize));
    #[cfg(not(target_os = "macos"))]
    let _ = (app, title, keep);
}

/// 由 AppKit 决定菜单项可用性。只适用于没有手动停用项的菜单，否则它们会被重新启用。
#[tauri::command]
pub fn menu_autoenable(app: AppHandle, title: String) {
    #[cfg(target_os = "macos")]
    let _ = app.run_on_main_thread(move || with_menu(&title, |menu| menu.setAutoenablesItems(true)));
    #[cfg(not(target_os = "macos"))]
    let _ = (app, title);
}
