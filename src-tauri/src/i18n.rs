//! 后端产生的消息（显示给用户的错误）所用的语言。

use std::sync::atomic::{AtomicBool, Ordering};

/// 前端通知之前默认中文。
static ENGLISH: AtomicBool = AtomicBool::new(false);

/// 前端在启动时和用户切换语言时调用。
#[tauri::command]
pub fn set_locale(locale: String) {
    let english = locale == "en";
    ENGLISH.store(english, Ordering::Relaxed);
    remember_for_system(english);
}

/// macOS 的系统窗口（文件选择框、“关于”面板）使用应用的运行语言，该语言在启动时
/// 确定，默认跟随系统。把应用自己的选择记下来，下次启动起这些窗口就会跟随它，
/// 效果等同于在“系统设置”里为单个应用指定语言。
#[cfg(target_os = "macos")]
fn remember_for_system(english: bool) {
    use objc2::runtime::AnyObject;
    use objc2_foundation::{ns_string, NSArray, NSString, NSUserDefaults};

    let languages = NSArray::from_retained_slice(&[NSString::from_str(if english { "en" } else { "zh-Hans" })]);
    let value: &AnyObject = &languages;
    unsafe { NSUserDefaults::standardUserDefaults().setObject_forKey(Some(value), ns_string!("AppleLanguages")) };
}

#[cfg(not(target_os = "macos"))]
fn remember_for_system(_english: bool) {}

/// 按当前界面语言选取文字。
pub fn tr<'a>(zh: &'a str, en: &'a str) -> &'a str {
    if ENGLISH.load(Ordering::Relaxed) {
        en
    } else {
        zh
    }
}
