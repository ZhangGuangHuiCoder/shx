use std::path::PathBuf;

fn main() {
    tauri_build::build();
    place_info_plist();
}

/// 把 Info.plist 复制到可执行文件旁。
///
/// macOS 文件选择框等系统窗口按磁盘上 Info.plist 的本地化列表决定语言；
/// 开发时直接从 target/ 运行没有应用包，缺少它会显示英文。
fn place_info_plist() {
    println!("cargo:rerun-if-changed=Info.plist");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    // OUT_DIR 为 <target>/<profile>/build/<pkg>-<hash>/out，可执行文件在 <target>/<profile>。
    let out = PathBuf::from(std::env::var_os("OUT_DIR").expect("cargo sets OUT_DIR"));
    if let Some(dir) = out.ancestors().nth(3) {
        // 失败不影响运行，只是系统窗口显示英文。
        let _ = std::fs::copy("Info.plist", dir.join("Info.plist"));
    }
}
