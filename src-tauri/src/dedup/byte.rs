//! 字节级重复：先按文件大小分组，仅对潜在重复计算 blake3。

use std::collections::HashMap;
use std::io::Read;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use rayon::prelude::*;

use crate::models::MediaFile;

const CHUNK: usize = 64 * 1024;

pub fn hash_file(path: &Path) -> anyhow::Result<String> {
    let mut file = std::fs::File::open(path)?;
    let mut hasher = blake3::Hasher::new();
    let mut buf = vec![0u8; CHUNK];
    loop {
        let n = file.read(&mut buf)?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(to_hex(hasher.finalize().as_bytes()))
}

fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{:02x}", b)).collect()
}

/// 只为「大小出现 >= 2 次」的文件计算哈希，避免对唯一文件做无谓 I/O。
pub fn assign_byte_hashes(files: &mut [MediaFile], cancel: &AtomicBool) {
    let mut size_counts: HashMap<u64, usize> = HashMap::new();
    for f in files.iter() {
        *size_counts.entry(f.size_bytes).or_default() += 1;
    }

    files.par_iter_mut().for_each(|f| {
        if cancel.load(Ordering::Relaxed) {
            return;
        }
        if size_counts.get(&f.size_bytes).copied().unwrap_or(0) < 2 {
            return;
        }
        if let Ok(hash) = hash_file(&f.source_path) {
            f.byte_hash = Some(hash);
        }
    });
}

/// 返回重复分组（每组 >= 2 个下标）。
pub fn group_duplicates(files: &[MediaFile]) -> Vec<Vec<usize>> {
    let mut by_hash: HashMap<&str, Vec<usize>> = HashMap::new();
    for (i, f) in files.iter().enumerate() {
        if let Some(hash) = f.byte_hash.as_deref() {
            by_hash.entry(hash).or_default().push(i);
        }
    }

    let mut groups: Vec<Vec<usize>> = by_hash
        .into_values()
        .filter(|group| group.len() > 1)
        .collect();
    groups.sort_by_key(|g| g[0]);
    groups
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{MediaFile, MediaKind, ScanOptions};

    fn file(path: std::path::PathBuf, size: u64) -> MediaFile {
        MediaFile {
            id: String::new(),
            source_path: path,
            kind: MediaKind::Photo,
            size_bytes: size,
            ext: "jpg".into(),
            date: None,
            date_source: None,
            resolved: false,
            byte_hash: None,
            image_hash: None,
        }
    }

    #[test]
    fn detects_identical_bytes() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.jpg");
        let b = dir.path().join("b.jpg");
        let c = dir.path().join("c.jpg");
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();
        std::fs::write(&c, b"diff").unwrap();

        let mut files = vec![file(a.clone(), 4), file(b.clone(), 4), file(c.clone(), 4)];
        let cancel = AtomicBool::new(false);
        assign_byte_hashes(&mut files, &cancel);

        assert!(files[0].byte_hash.is_some());
        assert_eq!(files[0].byte_hash, files[1].byte_hash);
        assert_ne!(files[0].byte_hash, files[2].byte_hash);

        let groups = group_duplicates(&files);
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0], vec![0, 1]);
    }

    #[test]
    fn skips_unique_sizes() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a.jpg");
        let b = dir.path().join("b.jpg");
        std::fs::write(&a, b"a").unwrap();
        std::fs::write(&b, b"bb").unwrap();

        let mut files = vec![file(a, 1), file(b, 2)];
        let cancel = AtomicBool::new(false);
        assign_byte_hashes(&mut files, &cancel);

        assert!(files[0].byte_hash.is_none());
        assert!(files[1].byte_hash.is_none());
        assert!(group_duplicates(&files).is_empty());
    }

    // 编译期确保 ScanOptions 的 dedup 字段可用；避免贪心引入未使用告警。
    #[allow(dead_code)]
    fn _assert_options_exists(_: ScanOptions) {}
}
