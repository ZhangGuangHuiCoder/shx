//! 阻止 macOS 往应用菜单里添加它自己的菜单项。
//!
//! AppKit 会在它认定的“编辑”菜单末尾追加“自动填充”“开始听写…”“表情与符号”，
//! 还会在标准菜单项后面加上“全部关闭”之类的备选项。这些对终端客户端没有用，
//! 也不跟随应用语言。听写和表情可以通过偏好设置关闭，见 [`disable_system_items`]；
//! 其余的关不掉，由 [`menu_trim`] 在菜单就位后删除。

use tauri::AppHandle;

/// 关闭那些 AppKit 允许通过偏好设置去掉的菜单项。必须在应用启动前执行。
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
fn trim(title: &str, keep: isize) {
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
            while menu.numberOfItems() > keep {
                menu.removeItemAtIndex(menu.numberOfItems() - 1);
            }
        }
    }
}

/// 把标题为 `title` 的顶层菜单截断到前 `keep` 项，去掉系统追加的部分。
/// 前端每次安装菜单后调用。
#[tauri::command]
pub fn menu_trim(app: AppHandle, title: String, keep: usize) {
    #[cfg(target_os = "macos")]
    let _ = app.run_on_main_thread(move || trim(&title, keep as isize));
    #[cfg(not(target_os = "macos"))]
    let _ = (app, title, keep);
}
