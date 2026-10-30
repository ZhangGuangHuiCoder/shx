//! 保持唤醒：阻止系统休眠（类似 KeepingYouAwake）。
//!
//! macOS 起 caffeinate 子进程，Windows 调 SetThreadExecutionState；
//! 定时开启到期后自动恢复，手动开关会使待执行的定时失效。

use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use tauri::AppHandle;

#[cfg(target_os = "macos")]
use std::sync::Mutex;

static ACTIVE: AtomicBool = AtomicBool::new(false);
/// 代号：每次开关或新定时都会递增，使旧的定时任务失效。
static GENERATION: AtomicUsize = AtomicUsize::new(0);
/// 定时到期的 Unix 秒；0 表示无定时（无限期开启或已关闭）。
static DEADLINE: AtomicU64 = AtomicU64::new(0);

/// caffeinate 子进程；退出或关闭时终止，避免残留。
#[cfg(target_os = "macos")]
static CAFFEINATE: Mutex<Option<std::process::Child>> = Mutex::new(None);

pub fn is_active() -> bool {
    ACTIVE.load(Ordering::Relaxed)
}

fn now_secs() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0)
}

/// 定时剩余秒数；无限期开启或未开启时为 None。
pub fn remaining() -> Option<u64> {
    let deadline = DEADLINE.load(Ordering::Relaxed);
    if deadline == 0 {
        return None;
    }
    let now = now_secs();
    if deadline > now {
        Some(deadline - now)
    } else {
        None
    }
}

pub fn toggle(app: &AppHandle) {
    if is_active() {
        disable(app);
    } else {
        enable(app);
    }
}

/// 无限期开启。
pub fn enable(app: &AppHandle) {
    if is_active() {
        return;
    }
    platform_enable();
    ACTIVE.store(true, Ordering::Relaxed);
    DEADLINE.store(0, Ordering::Relaxed);
    crate::tray::refresh(app);
}

/// 开启并在 secs 秒后自动恢复；期间每秒刷新托盘倒计时。
pub fn enable_for(app: &AppHandle, secs: u64) {
    enable(app);
    let deadline = now_secs() + secs;
    DEADLINE.store(deadline, Ordering::Relaxed);
    crate::tray::refresh(app);
    let generation = GENERATION.fetch_add(1, Ordering::SeqCst) + 1;
    let handle = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_millis(1000));
        // 期间若被手动关闭或重新计时，则放弃本次定时。
        if GENERATION.load(Ordering::SeqCst) != generation || !is_active() {
            break;
        }
        if now_secs() >= deadline {
            disable(&handle);
            break;
        }
        crate::tray::refresh(&handle);
    });
}

pub fn disable(app: &AppHandle) {
    if !is_active() {
        return;
    }
    GENERATION.fetch_add(1, Ordering::SeqCst);
    platform_disable();
    ACTIVE.store(false, Ordering::Relaxed);
    DEADLINE.store(0, Ordering::Relaxed);
    crate::tray::refresh(app);
}

/// 退出前清理，避免 caffeinate 残留继续阻止休眠。
pub fn cleanup() {
    GENERATION.fetch_add(1, Ordering::SeqCst);
    platform_disable();
    ACTIVE.store(false, Ordering::Relaxed);
    DEADLINE.store(0, Ordering::Relaxed);
}

#[cfg(target_os = "macos")]
fn platform_enable() {
    let mut guard = CAFFEINATE.lock().unwrap();
    if guard.is_none() {
        // -d 防显示器休眠，-i 防系统空闲休眠。
        if let Ok(child) = std::process::Command::new("caffeinate").args(["-di"]).spawn() {
            *guard = Some(child);
        }
    }
}

#[cfg(target_os = "macos")]
fn platform_disable() {
    let mut guard = CAFFEINATE.lock().unwrap();
    if let Some(mut child) = guard.take() {
        let _ = child.kill();
        let _ = child.wait();
    }
}

#[cfg(windows)]
fn platform_enable() {
    use windows_sys::Win32::System::Power::{
        SetThreadExecutionState, ES_CONTINUOUS, ES_DISPLAY_REQUIRED, ES_SYSTEM_REQUIRED,
    };
    unsafe { SetThreadExecutionState(ES_CONTINUOUS | ES_SYSTEM_REQUIRED | ES_DISPLAY_REQUIRED) };
}

#[cfg(windows)]
fn platform_disable() {
    use windows_sys::Win32::System::Power::{SetThreadExecutionState, ES_CONTINUOUS};
    unsafe { SetThreadExecutionState(ES_CONTINUOUS) };
}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_enable() {}

#[cfg(not(any(target_os = "macos", windows)))]
fn platform_disable() {}
