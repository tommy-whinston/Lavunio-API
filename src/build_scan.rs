use std::io::Cursor;

use serde_json::{json, Value};
use zip::ZipArchive;

const MAX_ENTRIES: usize = 20_000;
const MAX_UNCOMPRESSED_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;

const SUSPICIOUS_EXTENSIONS: &[&str] = &[
    ".exe", ".msi", ".bat", ".cmd", ".ps1", ".scr", ".vbs", ".vbe", ".js", ".jse", ".wsf",
    ".wsh", ".lnk", ".url", ".reg", ".hta", ".cpl", ".pif", ".gadget", ".msc",
];

pub fn scan_zip(bytes: &[u8]) -> Result<Value, String> {
    let mut archive = ZipArchive::new(Cursor::new(bytes))
        .map_err(|error| format!("Не удалось прочитать ZIP: {error}"))?;
    if archive.len() > MAX_ENTRIES {
        return Err(format!(
            "В архиве слишком много файлов: {} (максимум {MAX_ENTRIES})",
            archive.len()
        ));
    }

    let mut files = 0usize;
    let mut bytes_total = 0u64;
    let mut suspicious = Vec::new();

    for index in 0..archive.len() {
        let file = archive
            .by_index(index)
            .map_err(|error| format!("Не удалось прочитать файл архива: {error}"))?;
        let name = file.name().to_string();
        if name.is_empty() {
            continue;
        }
        let normalized = name.replace('\\', "/");
        if normalized.contains('\0') {
            return Err("В архиве есть файл с недопустимым именем".to_string());
        }
        if normalized.starts_with('/') {
            return Err(format!("Абсолютный путь в архиве: {name}"));
        }
        if normalized.split('/').any(|part| part == "..") {
            return Err(format!("Path traversal в архиве: {name}"));
        }
        if normalized
            .split('/')
            .next()
            .map(|part| part.contains(':'))
            .unwrap_or(false)
        {
            return Err(format!("Недопустимый путь в архиве: {name}"));
        }
        if file.is_symlink() {
            return Err(format!("Символическая ссылка в архиве: {name}"));
        }
        if file.is_dir() {
            continue;
        }
        if file.size() > MAX_FILE_BYTES {
            return Err(format!(
                "Файл слишком большой: {name} ({} байт)",
                file.size()
            ));
        }
        bytes_total = bytes_total.saturating_add(file.size());
        if bytes_total > MAX_UNCOMPRESSED_BYTES {
            return Err("Архив распаковывается в слишком большой размер".to_string());
        }
        files += 1;
        let lower = normalized.to_lowercase();
        if SUSPICIOUS_EXTENSIONS
            .iter()
            .any(|extension| lower.ends_with(extension))
        {
            suspicious.push(name);
        }
    }

    if files == 0 {
        return Err("Архив не содержит файлов".to_string());
    }

    Ok(json!({
        "ok": true,
        "checkedAt": chrono::Utc::now().to_rfc3339(),
        "entries": archive.len(),
        "files": files,
        "bytes": bytes_total,
        "suspicious": suspicious,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use zip::write::SimpleFileOptions;
    use zip::{CompressionMethod, ZipWriter};

    fn make_zip(entries: &[(&str, &[u8], Option<u32>)]) -> Vec<u8> {
        let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
        for (name, data, mode) in entries {
            let mut options =
                SimpleFileOptions::default().compression_method(CompressionMethod::Stored);
            if let Some(mode) = mode {
                options = options.unix_permissions(*mode);
            }
            writer.start_file(*name, options).unwrap();
            writer.write_all(data).unwrap();
        }
        writer.finish().unwrap().into_inner()
    }

    #[test]
    fn accepts_normal_archive() {
        let bytes = make_zip(&[
            ("mods/example.jar", b"jar", None),
            ("config/example.toml", b"config", None),
        ]);
        let scan = scan_zip(&bytes).unwrap();
        assert_eq!(scan["files"], 2);
        assert_eq!(scan["suspicious"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn rejects_path_traversal() {
        let bytes = make_zip(&[("../evil.txt", b"evil", None)]);
        assert!(scan_zip(&bytes).is_err());
    }

    #[test]
    fn detects_symlink_mode() {
        let mode = 0o120777u32;
        assert_eq!(mode & 0o170000, 0o120000);
        let file_mode = 0o100644u32;
        assert_ne!(file_mode & 0o170000, 0o120000);
    }

    #[test]
    fn flags_suspicious_files() {
        let bytes = make_zip(&[("installer.exe", b"binary", None)]);
        let scan = scan_zip(&bytes).unwrap();
        assert_eq!(scan["suspicious"].as_array().unwrap().len(), 1);
    }
}
