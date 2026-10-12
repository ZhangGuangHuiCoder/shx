mod config;
mod error;
mod i18n;
mod keepalive;
mod menu;
mod sftp;
mod ssh;
mod transfer;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // 必须在创建应用（及其菜单）之前执行。
    menu::disable_system_items();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(ssh::AppState::default())
        // 主窗口创建时是隐藏的（tauri.conf.json 的 `visible: false`），由前端在界面画好后
        // 显示（main.ts），启动时才不会先白一下。万一前端没能跑起来，应用也不能一直看不见：
        // 过几秒还没显示就直接显示。
        .setup(|app| {
            if let Some(window) = tauri::Manager::get_webview_window(app, "main") {
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_secs(4));
                    if !window.is_visible().unwrap_or(true) {
                        let _ = window.show();
                    }
                });
            }
            Ok(())
        })
        // 主窗口销毁即退出应用，不依赖“最后一个窗口关闭时退出”的默认行为。
        .on_window_event(|window, event| {
            if window.label() == "main" && matches!(event, tauri::WindowEvent::Destroyed) {
                tauri::Manager::app_handle(window).exit(0);
            }
        })
        .invoke_handler(tauri::generate_handler![
            i18n::set_locale,
            menu::menu_trim,
            ssh::ssh_connect,
            ssh::ssh_disconnect,
            ssh::key_import,
            ssh::term_open,
            ssh::term_write,
            ssh::term_resize,
            keepalive::keepalive_prepare,
            keepalive::keepalive_list,
            keepalive::keepalive_end,
            sftp::sftp_list,
            sftp::sftp_stat,
            sftp::sftp_mkdir,
            sftp::sftp_create_file,
            sftp::sftp_rename,
            sftp::sftp_remove,
            sftp::sftp_move,
            sftp::sftp_copy,
            sftp::sftp_chmod,
            sftp::sftp_chown,
            sftp::sftp_read_text,
            sftp::sftp_write_text,
            sftp::sftp_du,
            sftp::sftp_compress,
            sftp::sftp_extract,
            sftp::sftp_search,
            transfer::transfer_start,
            transfer::transfer_pack,
            transfer::transfer_rebind,
            transfer::transfer_release,
            transfer::transfer_control,
            transfer::transfer_retry,
            transfer::transfer_list,
            transfer::transfer_clear,
            transfer::local_existing,
            transfer::default_download_dir,
            config::config_export,
            config::config_import,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
