//! 后端消息的语言。

use std::sync::atomic::{AtomicBool, Ordering};

/// 默认中文。
static ENGLISH: AtomicBool = AtomicBool::new(false);

#[tauri::command]
pub fn set_locale(locale: String) {
    let english = locale == "en";
    ENGLISH.store(english, Ordering::Relaxed);
    remember_for_system(english);
}

/// 写入 AppleLanguages，使下次启动时系统窗口（文件选择框等）跟随应用语言。
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

pub fn tr<'a>(zh: &'a str, en: &'a str) -> &'a str {
    if ENGLISH.load(Ordering::Relaxed) {
        en
    } else {
        zh
    }
}
