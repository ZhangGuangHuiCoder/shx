use std::path::PathBuf;

fn main() {
    tauri_build::build();
    place_info_plist();
}

/// 把 `Info.plist` 复制到 cargo 生成的可执行文件旁边。
///
/// macOS 的系统窗口（主要是文件选择框）由独立进程绘制，它根据磁盘上应用的
/// Info.plist 里的本地化列表决定语言。直接从 `target/` 运行时没有应用包，
/// Tauri 嵌进二进制的那份外部读不到，这些窗口就会显示成英文；
/// 放在可执行文件旁边的 Info.plist 则读得到。打包后的应用自带真正的 Info.plist。
fn place_info_plist() {
    println!("cargo:rerun-if-changed=Info.plist");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    // OUT_DIR 是 <target>/<profile>/build/<package>-<hash>/out，可执行文件在 <target>/<profile>。
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    if let Some(dir) = out.ancestors().nth(3) {
        // 尽力而为：复制失败应用照常运行，只是这些窗口显示英文。
        let _ = std::fs::copy("Info.plist", dir.join("Info.plist"));
    }
}
