// 避免 Windows 发布版运行时多出一个控制台窗口，请勿删除！
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    shx_lib::run()
}
