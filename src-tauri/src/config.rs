//! 读写配置备份文件。
//!
//! 备份内容（JSON）由前端生成和解析。这里在写盘前用 XChaCha20-Poly1305 加密，
//! 文件在应用之外既读不了也改不了：任何改动都会导致导入失败。
//!
//! 密钥内置在应用里，备份因此能在不同安装之间通用，也不需要记密码；
//! 但它只能防住没有这把密钥的人。

use std::io::Write;
use std::path::Path;

use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};

use crate::error::{AppError, Result};
use crate::i18n::tr;

const KEY: [u8; 32] = [
    0xda, 0x03, 0x0d, 0x56, 0xf3, 0xdc, 0x34, 0xf8, 0x6a, 0x0b, 0x97, 0x40, 0x26, 0x65, 0x64, 0x5a,
    0x9f, 0xc5, 0x01, 0x56, 0x27, 0x1b, 0x56, 0xf2, 0xa2, 0xc7, 0x60, 0x33, 0x72, 0xbc, 0x40, 0x1c,
];
/// 标识文件类型和格式版本；同样参与校验，无法被替换。
const MAGIC: &[u8] = b"SHX-BACKUP-1\n";
const NONCE_LEN: usize = 24;
/// `config_import` 能读取的最大文件。
const MAX_IMPORT_SIZE: u64 = 16 * 1024 * 1024;

fn cipher() -> XChaCha20Poly1305 {
    XChaCha20Poly1305::new_from_slice(&KEY).expect("key is 32 bytes")
}

fn seal(content: &str) -> Result<Vec<u8>> {
    let mut nonce = [0u8; NONCE_LEN];
    getrandom::fill(&mut nonce).map_err(|e| AppError::msg(format!("{}{e}", tr("无法生成随机数：", "Could not generate random data: "))))?;
    let payload = Payload {
        msg: content.as_bytes(),
        aad: MAGIC,
    };
    let sealed = cipher()
        .encrypt(&XNonce::from(nonce), payload)
        .map_err(|_| AppError::msg(tr("加密失败", "Encryption failed")))?;
    Ok([MAGIC, &nonce, &sealed].concat())
}

fn open(file: &[u8]) -> Result<String> {
    let damaged = || AppError::msg(tr(
            "备份文件已损坏或被篡改，无法恢复",
            "The backup file is damaged or has been modified and cannot be restored",
        ));
    let body = file
        .strip_prefix(MAGIC)
        .ok_or_else(|| AppError::msg(tr("不是 shx 配置备份文件", "Not an shx configuration backup file")))?;
    if body.len() < NONCE_LEN {
        return Err(damaged());
    }
    let (nonce, msg) = body.split_at(NONCE_LEN);
    let nonce: [u8; NONCE_LEN] = nonce.try_into().map_err(|_| damaged())?;
    let plain = cipher()
        .decrypt(&XNonce::from(nonce), Payload { msg, aad: MAGIC })
        .map_err(|_| damaged())?;
    String::from_utf8(plain).map_err(|_| damaged())
}

/// 把加密后的备份写入 `path`，仅文件所有者可读。
#[tauri::command]
pub fn config_export(path: String, content: String) -> Result<()> {
    let path = Path::new(&path);
    let sealed = seal(&content)?;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        options.mode(0o600);
        // `mode` 只对新建的文件生效。
        if path.exists() {
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o600))?;
        }
    }
    options.open(path)?.write_all(&sealed)?;
    Ok(())
}

/// 读取 `config_export` 写出的备份并返回其内容；文件不是备份或被改动过则失败。
#[tauri::command]
pub fn config_import(path: String) -> Result<String> {
    if std::fs::metadata(&path)?.len() > MAX_IMPORT_SIZE {
        return Err(AppError::msg(tr(
            "文件过大，不是 shx 配置备份文件",
            "The file is too large to be an shx configuration backup",
        )));
    }
    open(&std::fs::read(&path)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    const CONTENT: &str = "{\"app\":\"shx\",\"secrets\":{\"a\":\"correct-horse-battery\"},\"note\":\"中文\"}";

    fn temp_dir() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("shx-config-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn backup_round_trips_and_hides_its_content() {
        let dir = temp_dir();
        let path = dir.join("backup.shxbak").to_string_lossy().into_owned();
        config_export(path.clone(), CONTENT.into()).unwrap();
        assert_eq!(config_import(path.clone()).unwrap(), CONTENT);

        let raw = std::fs::read(&path).unwrap();
        let text = String::from_utf8_lossy(&raw);
        for leaked in ["correct-horse-battery", "secrets", "\"app\""] {
            assert!(!text.contains(leaked), "{leaked} is readable in the backup");
        }
        // 同样的数据两次备份结果不同，无法通过比对猜测内容。
        assert_ne!(seal(CONTENT).unwrap(), seal(CONTENT).unwrap());

        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = || std::fs::metadata(&path).unwrap().permissions().mode() & 0o777;
            assert_eq!(mode(), 0o600);
            std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
            config_export(path.clone(), "short".into()).unwrap();
            assert_eq!(mode(), 0o600, "overwriting tightens a world-readable file");
            assert_eq!(config_import(path).unwrap(), "short");
        }
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn any_change_to_the_file_is_rejected() {
        let sealed = seal(CONTENT).unwrap();
        assert_eq!(open(&sealed).unwrap(), CONTENT);

        // 逐位翻转整个文件：文件头、nonce、数据和校验标签。
        for index in 0..sealed.len() {
            let mut tampered = sealed.clone();
            tampered[index] ^= 0x01;
            assert!(open(&tampered).is_err(), "byte {index} was changed and still accepted");
        }
        for cut in [0, 5, MAGIC.len(), MAGIC.len() + NONCE_LEN, sealed.len() - 1] {
            assert!(open(&sealed[..cut]).is_err(), "truncated to {cut} bytes");
        }
        let mut extended = sealed.clone();
        extended.push(0);
        assert!(open(&extended).is_err(), "data appended");
    }

    #[test]
    fn files_that_are_not_backups_are_refused() {
        let dir = temp_dir();
        let import = |name: &str| config_import(dir.join(name).to_string_lossy().into_owned());

        // 手写的明文 JSON“备份”不能被导入。
        std::fs::write(dir.join("plain.json"), CONTENT).unwrap();
        assert_eq!(import("plain.json").unwrap_err().to_string(), "不是 shx 配置备份文件");
        std::fs::write(dir.join("empty"), "").unwrap();
        assert!(import("empty").is_err());
        assert!(import("missing").is_err());
        std::fs::remove_dir_all(dir).unwrap();
    }
}
