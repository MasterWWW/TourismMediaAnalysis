//! 扫描：目录遍历 + 扩展名分类 + 拍摄日期解析 + 去重 + 进度回调 + 取消。

use std::collections::HashSet;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use crate::date::{self, ToolProbe};
use crate::dedup::{self, DedupResult};
use crate::models::{MediaFile, MediaKind, ScanOptions, ScanPhase, ScanProgress, ScanStats};

const PHOTO_EXTS: &[&str] = &["jpg", "jpeg", "png", "heic", "heif", "webp", "tif", "tiff"];
const RAW_EXTS: &[&str] = &["dng", "cr2", "cr3", "nef", "arw", "orf", "rw2", "raf"];
const VIDEO_EXTS: &[&str] = &["mp4", "mov", "m4v", "avi", "mkv", "3gp", "webm"];

#[derive(Debug, Clone)]
pub struct ScanOutcome {
    pub files: Vec<MediaFile>,
    pub stats: ScanStats,
    pub byte_groups: Vec<Vec<usize>>,
    pub similar_groups: Vec<Vec<usize>>,
}

/// 提取小写扩展名（不含点）。无扩展名或非 UTF-8 时返回 None。
pub fn ext_of(path: &Path) -> Option<String> {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
}

/// 按扩展名归类；内部会转小写，故对大小写不敏感。不认识的扩展名返回 None（忽略）。
pub fn classify_kind(ext: &str) -> Option<MediaKind> {
    let e = ext.to_ascii_lowercase();
    if VIDEO_EXTS.contains(&e.as_str()) {
        Some(MediaKind::Video)
    } else if RAW_EXTS.contains(&e.as_str()) {
        Some(MediaKind::Raw)
    } else if PHOTO_EXTS.contains(&e.as_str()) {
        Some(MediaKind::Photo)
    } else {
        None
    }
}

/// 是否纳入扫描（考虑 RAW / HEIC 开关）。
pub fn should_include(ext: &str, options: &ScanOptions) -> bool {
    let e = ext.to_ascii_lowercase();
    match classify_kind(&e) {
        Some(MediaKind::Raw) => options.include_raw,
        Some(MediaKind::Photo) if e == "heic" || e == "heif" => options.include_heic,
        Some(_) => true,
        None => false,
    }
}

/// 稳定 id：路径的确定性 blake3 哈希（跨进程稳定，保证后续幂等/日志一致）。
fn stable_id(path: &Path) -> String {
    let bytes = blake3::hash(path.to_string_lossy().as_bytes());
    bytes
        .as_bytes()
        .iter()
        .map(|b| format!("{:02x}", b))
        .collect()
}

#[allow(clippy::too_many_arguments)]
fn progress(
    run_id: &str,
    total: u64,
    media: u64,
    unresolved: u64,
    phase: ScanPhase,
    processed: u64,
) -> ScanProgress {
    ScanProgress {
        run_id: run_id.to_owned(),
        phase,
        total_files: total,
        processed_files: processed,
        media_found: media,
        duplicate_groups: 0,
        similar_groups: 0,
        unresolved,
        message: None,
    }
}

/// 扫描入口。每处理一批文件调用一次 `on_progress`；`cancel` 置位后尽快退出。
pub fn scan(
    run_id: &str,
    options: &ScanOptions,
    tools: &ToolProbe,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(ScanProgress),
) -> anyhow::Result<ScanOutcome> {
    let mut files = Vec::new();
    let mut total = 0u64;
    let mut media = 0u64;
    let mut unresolved = 0u64;
    let mut seen_paths = HashSet::new();

    on_progress(progress(
        run_id,
        total,
        media,
        unresolved,
        ScanPhase::Enumerating,
        0,
    ));

    'outer: for dir in &options.source_dirs {
        let mut builder = jwalk::WalkDir::new(dir);
        builder = builder
            .sort(true)
            .skip_hidden(true)
            .follow_links(options.follow_symlinks);
        if !options.recursive {
            builder = builder.max_depth(1);
        }

        for entry in builder {
            if cancel.load(Ordering::Relaxed) {
                break 'outer;
            }
            let entry = match entry {
                Ok(entry) => entry,
                Err(err) => {
                    tracing::warn!(error = %err, "skipping unreadable directory entry");
                    continue;
                }
            };
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            if should_skip_path(&path, options) {
                continue;
            }
            total += 1;
            let dedup_key = normalized_path(&path);
            if !seen_paths.insert(dedup_key) {
                continue;
            }

            let Some(ext) = ext_of(&path) else {
                continue;
            };
            if !should_include(&ext, options) {
                continue;
            }

            let kind = classify_kind(&ext).expect("should_include 已保证可识别");
            let resolution = date::resolve_date(&path, kind, &options.date_priority, tools);
            let resolved = resolution.is_resolved();
            if !resolved {
                unresolved += 1;
            }

            let size = entry.metadata().map(|m| m.len()).unwrap_or(0);
            media += 1;

            files.push(MediaFile {
                id: stable_id(&path),
                source_path: path,
                kind,
                size_bytes: size,
                ext,
                date: resolution.date,
                date_source: resolution.source,
                resolved,
                // M3 填充
                byte_hash: None,
                image_hash: None,
            });

            if total % 500 == 0 {
                on_progress(progress(
                    run_id,
                    total,
                    media,
                    unresolved,
                    ScanPhase::ReadingMetadata,
                    total,
                ));
            }
        }
    }

    on_progress(progress(
        run_id,
        total,
        media,
        unresolved,
        ScanPhase::Hashing,
        total,
    ));
    let dedup = if cancel.load(Ordering::Relaxed) {
        DedupResult::default()
    } else {
        dedup::run(&mut files, &options.dedup, cancel)
    };
    on_progress(progress(
        run_id,
        total,
        media,
        unresolved,
        ScanPhase::Dedup,
        total,
    ));

    let stats = ScanStats {
        total_files: total,
        media_found: media,
        duplicate_groups: dedup.byte_groups.len() as u64,
        similar_groups: dedup.similar_groups.len() as u64,
        unresolved,
    };
    on_progress(progress(
        run_id,
        total,
        media,
        unresolved,
        ScanPhase::Done,
        total,
    ));
    Ok(ScanOutcome {
        files,
        stats,
        byte_groups: dedup.byte_groups,
        similar_groups: dedup.similar_groups,
    })
}

fn should_skip_path(path: &Path, options: &ScanOptions) -> bool {
    let name = path
        .file_name()
        .map(|name| name.to_string_lossy().to_ascii_lowercase())
        .unwrap_or_default();
    if matches!(
        name.as_str(),
        ".ds_store" | ".picasa.ini" | "thumbs.db" | "desktop.ini"
    ) || name.starts_with("._")
    {
        return true;
    }

    // 不扫描隔离区；如果目标目录嵌在源目录中，也不把它当作新输入。
    if is_under(path, &options.target_dir.join("_duplicates")) {
        return true;
    }
    options.source_dirs.iter().any(|source| {
        !same_path(source, &options.target_dir)
            && is_under(&options.target_dir, source)
            && is_under(path, &options.target_dir)
    })
}

fn normalized_path(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

fn same_path(a: &Path, b: &Path) -> bool {
    normalized_path(a) == normalized_path(b)
}

fn is_under(path: &Path, root: &Path) -> bool {
    let path = normalized_path(path);
    let root_value = normalized_path(root);
    if root_value.is_empty() {
        return false;
    }
    let mut prefix = root_value;
    if !prefix.ends_with(std::path::MAIN_SEPARATOR) && !prefix.ends_with('/') {
        prefix.push(std::path::MAIN_SEPARATOR);
    }
    path.starts_with(&prefix)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn options_for(dir: &Path, include_raw: bool, include_heic: bool) -> ScanOptions {
        ScanOptions {
            source_dirs: vec![dir.to_path_buf()],
            target_dir: PathBuf::new(),
            recursive: true,
            follow_symlinks: false,
            include_raw,
            include_heic,
            date_priority: vec![crate::models::DateSource::Filename],
            dedup: crate::models::DedupOptions {
                byte: true,
                similar_images: None,
                similar_videos: None,
            },
            naming: crate::models::NamingOptions { prefix_date: false },
        }
    }

    #[test]
    fn classify_known_extensions() {
        assert_eq!(classify_kind("jpg"), Some(MediaKind::Photo));
        assert_eq!(classify_kind("heic"), Some(MediaKind::Photo));
        assert_eq!(classify_kind("cr3"), Some(MediaKind::Raw));
        assert_eq!(classify_kind("mov"), Some(MediaKind::Video));
        assert_eq!(classify_kind("zip"), None);
    }

    #[test]
    fn classify_is_case_insensitive() {
        assert_eq!(classify_kind("JPG"), Some(MediaKind::Photo));
        assert_eq!(classify_kind("Mp4"), Some(MediaKind::Video));
        assert_eq!(classify_kind("Cr2"), Some(MediaKind::Raw));
    }

    #[test]
    fn ext_of_lowercases_and_strips_dot() {
        assert_eq!(ext_of(Path::new("IMG_0001.JPG")).as_deref(), Some("jpg"));
        assert_eq!(ext_of(Path::new("noext")), None);
    }

    #[test]
    fn include_flags_are_respected() {
        let dir = Path::new("/tmp");
        assert!(should_include("jpg", &options_for(dir, true, true)));
        assert!(should_include("cr2", &options_for(dir, true, false)));
        assert!(!should_include("cr2", &options_for(dir, false, false)));
        assert!(should_include("heic", &options_for(dir, false, true)));
        assert!(!should_include("heic", &options_for(dir, false, false)));
    }

    #[test]
    fn scan_collects_media_dates_and_stats() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("2024-05-01_120000.JPG"), b"a").unwrap();
        std::fs::write(dir.path().join("b.mp4"), b"bb").unwrap();
        std::fs::write(dir.path().join("notes.txt"), b"ccc").unwrap();

        let cancel = AtomicBool::new(false);
        let outcome = scan(
            "run-test",
            &options_for(dir.path(), true, true),
            &ToolProbe::default(),
            &cancel,
            |_| {},
        )
        .unwrap();

        assert_eq!(outcome.files.len(), 2);
        assert_eq!(outcome.stats.total_files, 3);
        assert_eq!(outcome.stats.media_found, 2);
        assert_eq!(outcome.stats.unresolved, 1); // b.mp4 文件名无日期

        let dated = outcome
            .files
            .iter()
            .find(|f| f.ext == "jpg")
            .expect("jpg should exist");
        assert!(dated.resolved);
        assert_eq!(
            dated.date.unwrap().to_rfc3339(),
            "2024-05-01T12:00:00+00:00"
        );
    }

    #[test]
    fn scan_stops_when_canceled() {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..5 {
            std::fs::write(dir.path().join(format!("img{i}.jpg")), b"x").unwrap();
        }

        let cancel = AtomicBool::new(true);
        let outcome = scan(
            "run-cancel",
            &options_for(dir.path(), true, true),
            &ToolProbe::default(),
            &cancel,
            |_| {},
        )
        .unwrap();
        assert!(outcome.files.is_empty());
    }

    #[test]
    fn skips_target_subtree_and_duplicate_source_entries() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("source");
        let target = source.join("organized");
        std::fs::create_dir_all(target.join("2024/2024-05")).unwrap();
        std::fs::write(source.join("2024-01-01_100000.jpg"), b"a").unwrap();
        std::fs::write(target.join("2024/2024-05/already.jpg"), b"b").unwrap();

        let mut options = options_for(&source, true, true);
        options.target_dir = target;
        let cancel = AtomicBool::new(false);
        let outcome = scan(
            "run-target",
            &options,
            &ToolProbe::default(),
            &cancel,
            |_| {},
        )
        .unwrap();

        assert_eq!(outcome.files.len(), 1);
        assert!(outcome.files[0]
            .source_path
            .ends_with("2024-01-01_100000.jpg"));
    }

    #[test]
    fn recognizes_nested_target_directory() {
        let root = PathBuf::from("/photos");
        assert!(is_under(
            Path::new("/photos/organized/2024/a.jpg"),
            &root.join("organized")
        ));
        assert!(!is_under(
            Path::new("/photos/import/a.jpg"),
            &root.join("organized")
        ));
    }
}
