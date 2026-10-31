//! 由扫描库存生成可审阅的整理计划。
//!
//! 计划阶段永远只读。保存规则、冲突处理和隔离路径都在这里确定，执行层只负责
//! 按计划移动文件，从而保证预览与最终结果一致。

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use crate::dedup::byte::hash_file;
use crate::models::{MediaFile, PlanAction, PlanItem, ScanOptions};

#[derive(Debug, Clone)]
struct Decision {
    action: PlanAction,
    reason: String,
    group_id: Option<String>,
}

/// 生成完整计划。所有输入文件都会得到且只得到一个 `PlanItem`。
pub fn build_plan(
    files: &[MediaFile],
    byte_groups: &[Vec<usize>],
    similar_groups: &[Vec<usize>],
    options: &ScanOptions,
) -> anyhow::Result<Vec<PlanItem>> {
    let mut decisions: Vec<Option<Decision>> = vec![None; files.len()];
    let mut hash_cache: HashMap<usize, String> = files
        .iter()
        .enumerate()
        .filter_map(|(i, file)| file.byte_hash.clone().map(|hash| (i, hash)))
        .collect();

    // 字节重复优先，避免同一份内容的多个副本再次参与相似图比较。
    for (group_no, group) in byte_groups.iter().enumerate() {
        let eligible: Vec<usize> = group
            .iter()
            .copied()
            .filter(|&i| files.get(i).is_some_and(|f| f.resolved))
            .collect();
        if eligible.len() < 2 {
            continue;
        }

        let group_id = format!("byte-{:04}", group_no + 1);
        let keeper = *eligible
            .iter()
            .min_by(|&&a, &&b| compare_byte_keeper(&files[a], &files[b]))
            .expect("eligible is not empty");

        for &i in &eligible {
            if i == keeper {
                decisions[i] = Some(Decision {
                    action: PlanAction::Archive,
                    reason: "byte_duplicate_keeper".into(),
                    group_id: Some(group_id.clone()),
                });
            } else {
                decisions[i] = Some(Decision {
                    action: PlanAction::Isolate,
                    reason: "byte_duplicate".into(),
                    group_id: Some(group_id.clone()),
                });
            }
        }
    }

    // 相似图组仍然保留一个最优副本归档；原本属于字节重复组的副本不再晋升为保留项。
    for (group_no, group) in similar_groups.iter().enumerate() {
        let eligible: Vec<usize> = group
            .iter()
            .copied()
            .filter(|&i| {
                files.get(i).is_some_and(|f| f.resolved)
                    && decisions
                        .get(i)
                        .and_then(Option::as_ref)
                        .map_or(true, |d| d.action != PlanAction::Isolate)
            })
            .collect();
        if eligible.len() < 2 {
            continue;
        }

        let group_id = format!("similar-{:04}", group_no + 1);
        let merged_byte_groups: HashSet<String> = eligible
            .iter()
            .filter_map(|index| {
                decisions[*index]
                    .as_ref()
                    .and_then(|decision| decision.group_id.clone())
                    .filter(|id| id.starts_with("byte-"))
            })
            .collect();
        for decision in decisions.iter_mut().flatten() {
            if decision
                .group_id
                .as_ref()
                .is_some_and(|id| merged_byte_groups.contains(id))
            {
                decision.group_id = Some(group_id.clone());
            }
        }

        let keeper = *eligible
            .iter()
            .min_by(|&&a, &&b| compare_similar_keeper(&files[a], &files[b]))
            .expect("eligible is not empty");

        for &i in &eligible {
            if i == keeper {
                let prior_reason = decisions[i].as_ref().map(|d| d.reason.as_str());
                decisions[i] = Some(Decision {
                    action: PlanAction::Archive,
                    reason: prior_reason
                        .filter(|reason| *reason == "byte_duplicate_keeper")
                        .unwrap_or("similar_image_keeper")
                        .into(),
                    group_id: Some(group_id.clone()),
                });
            } else {
                decisions[i] = Some(Decision {
                    action: PlanAction::Isolate,
                    reason: "similar_image".into(),
                    group_id: Some(group_id.clone()),
                });
            }
        }
    }

    for (i, file) in files.iter().enumerate() {
        if decisions[i].is_none() {
            decisions[i] = Some(if file.resolved {
                Decision {
                    action: PlanAction::Archive,
                    reason: "dated".into(),
                    group_id: None,
                }
            } else {
                Decision {
                    action: PlanAction::Unresolved,
                    reason: "date_unresolved".into(),
                    group_id: None,
                }
            });
        }
    }

    let mut reserved: HashMap<String, Option<String>> = HashMap::new();
    let mut items = Vec::with_capacity(files.len());

    for (i, file) in files.iter().enumerate() {
        let mut decision = decisions[i].clone().expect("decision filled above");

        if is_under(&file.source_path, &options.target_dir.join("_duplicates")) {
            items.push(plan_item(
                file,
                None,
                PlanAction::Keep,
                "already_isolated",
                decision.group_id,
            ));
            continue;
        }

        let desired = match decision.action {
            PlanAction::Isolate => isolate_target(&options.target_dir, file),
            PlanAction::Unresolved => options.target_dir.join("unresolved").join(file_name(file)?),
            PlanAction::Archive => archive_target(&options.target_dir, file, &options.naming)?,
            PlanAction::Keep => unreachable!("Keep is only produced below"),
        };

        if path_key(&desired) == path_key(&file.source_path) {
            let reason = if decision.group_id.is_some() && decision.action != PlanAction::Isolate {
                "already_organized_keeper"
            } else {
                "already_organized"
            };
            items.push(plan_item(
                file,
                None,
                PlanAction::Keep,
                reason,
                decision.group_id,
            ));
            continue;
        }

        // 唯一文件不需要预先重读；只有去重组或目标同名冲突才需要内容指纹。
        let may_collide = decision.action == PlanAction::Archive
            && (desired.exists() || reserved.contains_key(&path_key(&desired)));
        let source_hash = if file.byte_hash.is_some() || may_collide {
            cached_hash(i, file, &mut hash_cache)
        } else {
            None
        };
        let (target, existing_same) = reserve_target(
            &desired,
            source_hash.as_deref(),
            decision.action != PlanAction::Archive,
            &mut reserved,
        );

        if existing_same && decision.action == PlanAction::Archive {
            decision = Decision {
                action: PlanAction::Isolate,
                reason: "existing_identical".into(),
                group_id: decision.group_id,
            };
            let isolate_desired = isolate_target(&options.target_dir, file);
            let (isolate_target, _) = reserve_target(
                &isolate_desired,
                source_hash.as_deref(),
                true,
                &mut reserved,
            );
            items.push(plan_item(
                file,
                Some(isolate_target),
                PlanAction::Isolate,
                &decision.reason,
                decision.group_id,
            ));
            continue;
        }

        items.push(PlanItem {
            id: file.id.clone(),
            source_path: file.source_path.clone(),
            target_path: Some(target),
            action: decision.action,
            reason: Some(decision.reason),
            group_id: decision.group_id,
            kind: file.kind,
            size_bytes: file.size_bytes,
            date: file.date,
            selected: decision.action != PlanAction::Keep,
        });
    }

    Ok(items)
}

fn plan_item(
    file: &MediaFile,
    target_path: Option<PathBuf>,
    action: PlanAction,
    reason: &str,
    group_id: Option<String>,
) -> PlanItem {
    PlanItem {
        id: file.id.clone(),
        source_path: file.source_path.clone(),
        target_path,
        action,
        reason: Some(reason.into()),
        group_id,
        kind: file.kind,
        size_bytes: file.size_bytes,
        date: file.date,
        selected: false,
    }
}

fn archive_target(
    target_dir: &Path,
    file: &MediaFile,
    naming: &crate::models::NamingOptions,
) -> anyhow::Result<PathBuf> {
    let date = file
        .date
        .ok_or_else(|| anyhow::anyhow!("archive target requires a resolved date"))?;
    let year = date.format("%Y").to_string();
    let month = date.format("%Y-%m").to_string();
    let original = file_name(file)?;
    let name = if naming.prefix_date {
        let prefix = date.format("%Y%m%d_%H%M%S");
        PathBuf::from(format!("{prefix}_{}", original.to_string_lossy()))
    } else {
        original
    };
    Ok(target_dir.join(year).join(month).join(name))
}

fn isolate_target(target_dir: &Path, file: &MediaFile) -> PathBuf {
    let name = file
        .source_path
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("unnamed"));
    match file.date {
        Some(date) => target_dir
            .join("_duplicates")
            .join(date.format("%Y").to_string())
            .join(date.format("%Y-%m").to_string())
            .join(name),
        None => target_dir.join("_duplicates").join("unresolved").join(name),
    }
}

fn file_name(file: &MediaFile) -> anyhow::Result<PathBuf> {
    file.source_path
        .file_name()
        .map(PathBuf::from)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "source path has no file name: {}",
                file.source_path.display()
            )
        })
}

fn compare_byte_keeper(a: &MediaFile, b: &MediaFile) -> Ordering {
    path_regularity(a).cmp(&path_regularity(b))
}

fn path_regularity(file: &MediaFile) -> (usize, usize, u128, String) {
    let depth = file.source_path.components().count();
    let name_len = file
        .source_path
        .file_name()
        .map(|n| n.to_string_lossy().chars().count())
        .unwrap_or(usize::MAX);
    let created = std::fs::metadata(&file.source_path)
        .and_then(|m| m.created().or_else(|_| m.modified()))
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_nanos())
        .unwrap_or(u128::MAX);
    (
        depth,
        name_len,
        created,
        file.source_path.to_string_lossy().to_string(),
    )
}

fn compare_similar_keeper(a: &MediaFile, b: &MediaFile) -> Ordering {
    b.size_bytes
        .cmp(&a.size_bytes)
        .then_with(|| a.date.cmp(&b.date))
        .then_with(|| path_regularity(a).cmp(&path_regularity(b)))
}

fn cached_hash(
    index: usize,
    file: &MediaFile,
    cache: &mut HashMap<usize, String>,
) -> Option<String> {
    if let Some(hash) = cache.get(&index) {
        return Some(hash.clone());
    }
    let hash = hash_file(&file.source_path).ok()?;
    cache.insert(index, hash.clone());
    Some(hash)
}

fn reserve_target(
    desired: &Path,
    source_hash: Option<&str>,
    always_unique: bool,
    reserved: &mut HashMap<String, Option<String>>,
) -> (PathBuf, bool) {
    for suffix in 0..100_000usize {
        let candidate = if suffix == 0 {
            desired.to_path_buf()
        } else {
            with_suffix(desired, suffix)
        };
        let key = path_key(&candidate);

        if let Some(existing_hash) = reserved.get(&key) {
            if !always_unique && hashes_equal(source_hash, existing_hash.as_deref()) {
                return (candidate, true);
            }
            continue;
        }

        if candidate.exists() {
            let existing_hash = if candidate.is_file() {
                hash_file(&candidate).ok()
            } else {
                None
            };
            reserved.insert(key, existing_hash.clone());
            if !always_unique && hashes_equal(source_hash, existing_hash.as_deref()) {
                return (candidate, true);
            }
            continue;
        }

        reserved.insert(key, source_hash.map(str::to_owned));
        return (candidate, false);
    }

    // 极端同名冲突下仍不覆盖；使用进程级唯一后缀继续保证安全。
    loop {
        let candidate = with_suffix(desired, uuid::Uuid::new_v4().as_u128() as usize);
        let key = path_key(&candidate);
        if !candidate.exists() && !reserved.contains_key(&key) {
            reserved.insert(key, source_hash.map(str::to_owned));
            return (candidate, false);
        }
    }
}

fn hashes_equal(a: Option<&str>, b: Option<&str>) -> bool {
    matches!((a, b), (Some(a), Some(b)) if a == b)
}

fn with_suffix(path: &Path, suffix: usize) -> PathBuf {
    let parent = path.parent().unwrap_or_else(|| Path::new(""));
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "file".into());
    match path.extension().and_then(|e| e.to_str()) {
        Some(ext) => parent.join(format!("{stem} ({suffix}).{ext}")),
        None => parent.join(format!("{stem} ({suffix})")),
    }
}

/// 计划期用于同名冲突检测；统一小写可同时规避 Windows/macOS 默认大小写不敏感文件系统。
fn path_key(path: &Path) -> String {
    path.to_string_lossy().to_lowercase()
}

pub fn is_under(path: &Path, root: &Path) -> bool {
    let path = path_key(path);
    let mut root = path_key(root);
    if !root.ends_with(std::path::MAIN_SEPARATOR) && !root.ends_with('/') {
        root.push(std::path::MAIN_SEPARATOR);
    }
    path.starts_with(&root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        DateSource, DedupOptions, MediaKind, NamingOptions, ScanOptions, SimilarityOptions,
    };
    use chrono::{DateTime, TimeZone, Utc};

    fn options(target: &Path) -> ScanOptions {
        ScanOptions {
            source_dirs: vec![],
            target_dir: target.to_path_buf(),
            recursive: true,
            follow_symlinks: false,
            include_raw: true,
            include_heic: true,
            date_priority: vec![DateSource::Filename],
            dedup: DedupOptions {
                byte: true,
                similar_images: Some(SimilarityOptions {
                    hamming_threshold: 8,
                }),
                similar_videos: None,
            },
            naming: NamingOptions { prefix_date: false },
        }
    }

    fn media(path: PathBuf, date: Option<DateTime<Utc>>, hash: Option<&str>) -> MediaFile {
        MediaFile {
            id: path.to_string_lossy().to_string(),
            source_path: path,
            kind: MediaKind::Photo,
            size_bytes: 10,
            ext: "jpg".into(),
            date,
            date_source: date.map(|_| DateSource::Filename),
            resolved: date.is_some(),
            byte_hash: hash.map(str::to_owned),
            image_hash: None,
        }
    }

    #[test]
    fn archives_dated_file_under_year_and_month() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("a.jpg");
        std::fs::write(&src, b"a").unwrap();
        let date = Utc.with_ymd_and_hms(2024, 5, 1, 12, 0, 0).unwrap();
        let files = vec![media(src, Some(date), None)];

        let plan = build_plan(&files, &[], &[], &options(&root.path().join("out"))).unwrap();
        assert_eq!(plan[0].action, PlanAction::Archive);
        assert_eq!(
            plan[0].target_path.as_ref().unwrap(),
            &root.path().join("out/2024/2024-05/a.jpg")
        );
    }

    #[test]
    fn isolates_byte_duplicates_and_keeps_one_archive_entry() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("short.jpg");
        let b = root.path().join("nested/longer-name.jpg");
        std::fs::create_dir_all(b.parent().unwrap()).unwrap();
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();
        let date = Utc.with_ymd_and_hms(2024, 5, 1, 12, 0, 0).unwrap();
        let files = vec![
            media(a, Some(date), Some("abc")),
            media(b, Some(date), Some("abc")),
        ];

        let plan = build_plan(
            &files,
            &[vec![0, 1]],
            &[],
            &options(&root.path().join("out")),
        )
        .unwrap();
        assert_eq!(
            plan.iter()
                .filter(|p| p.action == PlanAction::Archive)
                .count(),
            1
        );
        assert_eq!(
            plan.iter()
                .filter(|p| p.action == PlanAction::Isolate)
                .count(),
            1
        );
        assert!(plan
            .iter()
            .find(|p| p.action == PlanAction::Isolate)
            .unwrap()
            .target_path
            .as_ref()
            .unwrap()
            .starts_with(root.path().join("out/_duplicates")));
    }

    #[test]
    fn unresolved_files_are_preserved_not_isolated() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a.jpg");
        std::fs::write(&a, b"a").unwrap();
        let files = vec![media(a, None, Some("abc"))];

        let plan = build_plan(&files, &[vec![0]], &[], &options(&root.path().join("out"))).unwrap();
        assert_eq!(plan[0].action, PlanAction::Unresolved);
        assert!(plan[0]
            .target_path
            .as_ref()
            .unwrap()
            .starts_with(root.path().join("out/unresolved")));
    }

    #[test]
    fn conflicting_names_get_stable_numeric_suffix() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("src/a.jpg");
        std::fs::create_dir_all(src.parent().unwrap()).unwrap();
        std::fs::write(&src, b"new").unwrap();
        let out = root.path().join("out");
        let month = out.join("2024/2024-05");
        std::fs::create_dir_all(&month).unwrap();
        std::fs::write(month.join("a.jpg"), b"old").unwrap();

        let date = Utc.with_ymd_and_hms(2024, 5, 1, 12, 0, 0).unwrap();
        let files = vec![media(src, Some(date), None)];
        let plan = build_plan(&files, &[], &[], &options(&out)).unwrap();
        assert_eq!(
            plan[0].target_path.as_ref().unwrap(),
            &month.join("a (1).jpg")
        );
    }

    #[test]
    fn prefix_date_is_applied() {
        let root = tempfile::tempdir().unwrap();
        let src = root.path().join("a.jpg");
        std::fs::write(&src, b"a").unwrap();
        let date = Utc.with_ymd_and_hms(2024, 5, 1, 9, 8, 7).unwrap();
        let files = vec![media(src, Some(date), None)];
        let mut opts = options(&root.path().join("out"));
        opts.naming.prefix_date = true;

        let plan = build_plan(&files, &[], &[], &opts).unwrap();
        assert!(plan[0]
            .target_path
            .as_ref()
            .unwrap()
            .ends_with("20240501_090807_a.jpg"));
    }

    #[test]
    fn similar_group_merges_exact_duplicate_group_ids() {
        let root = tempfile::tempdir().unwrap();
        let a = root.path().join("a.jpg");
        let b = root.path().join("b.jpg");
        let c = root.path().join("c.jpg");
        std::fs::write(&a, b"aaaa").unwrap();
        std::fs::write(&b, b"aaaa").unwrap();
        std::fs::write(&c, b"cc").unwrap();
        let date = Utc.with_ymd_and_hms(2024, 5, 1, 12, 0, 0).unwrap();
        let files = vec![
            media(a, Some(date), Some("same")),
            media(b, Some(date), Some("same")),
            media(c, Some(date), Some("other")),
        ];

        let plan = build_plan(
            &files,
            &[vec![0, 1]],
            &[vec![0, 2]],
            &options(&root.path().join("out")),
        )
        .unwrap();
        let groups: HashSet<Option<String>> =
            plan.iter().map(|item| item.group_id.clone()).collect();
        assert_eq!(groups.len(), 1, "{plan:#?}");
        assert!(groups.iter().all(Option::is_some));
        assert_eq!(
            plan.iter()
                .filter(|item| item.action == PlanAction::Isolate)
                .count(),
            2
        );
    }
}
