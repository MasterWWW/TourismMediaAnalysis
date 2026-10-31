//! 计划执行：只处理勾选项、先归档后隔离、不覆盖目标，并逐项写入回滚日志。

use std::io::{Read, Write};
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};

use chrono::Utc;

use crate::dedup::byte::hash_file;
use crate::journal;
use crate::models::{ApplyProgress, ApplyResult, Failure, JournalEntry, PlanAction, PlanItem};

/// 执行一组计划项。调用方负责传入同一 run 的计划快照。
pub fn execute(
    run_id: &str,
    items: &[PlanItem],
    journal_path: &Path,
    cancel: &AtomicBool,
    mut on_progress: impl FnMut(ApplyProgress),
) -> ApplyResult {
    let mut selected: Vec<PlanItem> = items
        .iter()
        .filter(|item| item.selected && item.action != PlanAction::Keep)
        .cloned()
        .collect();
    selected.sort_by_key(|item| action_order(item.action));

    let total = selected.len() as u64;
    let mut result = ApplyResult {
        run_id: run_id.to_owned(),
        archived: 0,
        isolated: 0,
        unresolved: 0,
        freed_est_bytes: 0,
        failed: Vec::new(),
        journal_path: journal_path.to_path_buf(),
        canceled: false,
    };

    if let Err(err) = journal::write_journal(journal_path, run_id, Vec::new()) {
        result.failed.push(Failure {
            path: journal_path.to_path_buf(),
            code: "journal_unavailable".into(),
            message: err.to_string(),
        });
        return result;
    }

    let mut entries = Vec::with_capacity(selected.len());
    on_progress(ApplyProgress {
        run_id: run_id.to_owned(),
        done: 0,
        total,
        phase: "preparing".into(),
    });

    for (index, item) in selected.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            result.canceled = true;
            break;
        }

        let Some(target) = item.target_path.as_ref() else {
            result.failed.push(Failure {
                path: item.source_path.clone(),
                code: "missing_target".into(),
                message: "plan item has no target path".into(),
            });
            continue;
        };

        // 普通唯一文件不再为日志做一次全量重读；重复组文件保留内容指纹。
        let hash =
            if item.group_id.is_some() || item.reason.as_deref() == Some("existing_identical") {
                hash_file(&item.source_path).ok()
            } else {
                None
            };
        match move_no_overwrite(&item.source_path, target) {
            Ok(()) => {
                entries.push(JournalEntry {
                    action: action_name(item.action).into(),
                    from: item.source_path.clone(),
                    to: target.clone(),
                    hash,
                    reason: item.reason.clone(),
                    timestamp: Utc::now(),
                });
                match item.action {
                    PlanAction::Archive => result.archived += 1,
                    PlanAction::Isolate => {
                        result.isolated += 1;
                        result.freed_est_bytes += item.size_bytes;
                    }
                    PlanAction::Unresolved => result.unresolved += 1,
                    PlanAction::Keep => {}
                }
            }
            Err(err) => result.failed.push(Failure {
                path: item.source_path.clone(),
                code: "move_failed".into(),
                message: format!(
                    "{} -> {}: {err}",
                    item.source_path.display(),
                    target.display()
                ),
            }),
        }

        let done = (index + 1) as u64;
        if done == total || done % 20 == 0 {
            on_progress(ApplyProgress {
                run_id: run_id.to_owned(),
                done,
                total,
                phase: "moving".into(),
            });
        }
    }

    if let Err(err) = journal::write_journal(journal_path, run_id, entries) {
        result.failed.push(Failure {
            path: journal_path.to_path_buf(),
            code: "journal_write_failed".into(),
            message: err.to_string(),
        });
    }

    on_progress(ApplyProgress {
        run_id: run_id.to_owned(),
        done: total,
        total,
        phase: "done".into(),
    });
    result
}

fn action_order(action: PlanAction) -> u8 {
    match action {
        PlanAction::Archive => 0,
        PlanAction::Unresolved => 1,
        PlanAction::Isolate => 2,
        PlanAction::Keep => 3,
    }
}

fn action_name(action: PlanAction) -> &'static str {
    match action {
        PlanAction::Archive => "archive",
        PlanAction::Isolate => "isolate",
        PlanAction::Unresolved => "unresolved",
        PlanAction::Keep => "keep",
    }
}

/// 同卷使用硬链接占位后删除源名，跨卷/不支持硬链接时复制到 `create_new` 新文件。
/// 两条路径都不会对已存在的目标执行覆盖式 rename。
pub fn move_no_overwrite(from: &Path, to: &Path) -> anyhow::Result<()> {
    if !from.is_file() {
        anyhow::bail!("source is not a regular file: {}", from.display());
    }
    if to.exists() {
        anyhow::bail!("target already exists: {}", to.display());
    }
    if let Some(parent) = to.parent() {
        std::fs::create_dir_all(parent)?;
    }

    match std::fs::hard_link(from, to) {
        Ok(()) => {
            if let Err(err) = std::fs::remove_file(from) {
                if std::fs::remove_file(to).is_ok() {
                    return Err(err.into());
                }
                anyhow::bail!(
                    "linked to {} but could not remove source {}: {err}",
                    to.display(),
                    from.display()
                );
            }
            Ok(())
        }
        Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => Err(err.into()),
        Err(_) => copy_then_remove(from, to),
    }
}

fn copy_then_remove(from: &Path, to: &Path) -> anyhow::Result<()> {
    let mut source = std::fs::File::open(from)?;
    let mut target = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(to)?;

    let copy_result = (|| -> anyhow::Result<()> {
        let mut buf = vec![0u8; 1024 * 1024];
        loop {
            let read = source.read(&mut buf)?;
            if read == 0 {
                break;
            }
            target.write_all(&buf[..read])?;
        }
        target.sync_all()?;
        std::fs::set_permissions(to, source.metadata()?.permissions())?;
        Ok(())
    })();

    if let Err(err) = copy_result {
        drop(target);
        let _ = std::fs::remove_file(to);
        return Err(err);
    }
    drop(source);
    drop(target);

    if let Err(err) = std::fs::remove_file(from) {
        // 删除失败时尽量撤回新副本，避免留下两个看似成功的文件。
        if std::fs::remove_file(to).is_ok() {
            return Err(err.into());
        }
        anyhow::bail!(
            "copied to {} but could not remove source {}: {err}",
            to.display(),
            from.display()
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::{
        DateSource, DedupOptions, MediaKind, NamingOptions, PlanItem, ScanOptions,
    };
    use crate::plan;
    use chrono::{TimeZone, Utc};
    use std::path::PathBuf;

    fn media(path: PathBuf, hash: &str) -> crate::models::MediaFile {
        let date = Utc.with_ymd_and_hms(2024, 5, 1, 12, 0, 0).unwrap();
        crate::models::MediaFile {
            id: path.to_string_lossy().to_string(),
            source_path: path,
            kind: MediaKind::Photo,
            size_bytes: 4,
            ext: "jpg".into(),
            date: Some(date),
            date_source: Some(DateSource::Filename),
            resolved: true,
            byte_hash: Some(hash.into()),
            image_hash: None,
        }
    }

    fn options(root: &Path) -> ScanOptions {
        ScanOptions {
            source_dirs: vec![root.to_path_buf()],
            target_dir: root.join("organized"),
            recursive: true,
            follow_symlinks: false,
            include_raw: true,
            include_heic: true,
            date_priority: vec![DateSource::Filename],
            dedup: DedupOptions {
                byte: true,
                similar_images: None,
                similar_videos: None,
            },
            naming: NamingOptions { prefix_date: false },
        }
    }

    #[test]
    fn refuses_to_overwrite_existing_target() {
        let dir = tempfile::tempdir().unwrap();
        let from = dir.path().join("a");
        let to = dir.path().join("b");
        std::fs::write(&from, b"new").unwrap();
        std::fs::write(&to, b"old").unwrap();

        assert!(move_no_overwrite(&from, &to).is_err());
        assert_eq!(std::fs::read(&to).unwrap(), b"old");
        assert_eq!(std::fs::read(&from).unwrap(), b"new");
    }

    #[test]
    fn apply_then_rollback_restores_every_file() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        std::fs::create_dir_all(source.join("nested")).unwrap();
        let a = source.join("a.jpg");
        let b = source.join("nested/b.jpg");
        std::fs::write(&a, b"same").unwrap();
        std::fs::write(&b, b"same").unwrap();

        let files = vec![media(a.clone(), "abc"), media(b.clone(), "abc")];
        let opts = options(root.path());
        let plan = plan::build_plan(&files, &[vec![0, 1]], &[], &opts).unwrap();
        let journal_path = root.path().join("journal/run.json");
        let cancel = AtomicBool::new(false);
        let result = execute("run", &plan, &journal_path, &cancel, |_| {});

        assert!(result.failed.is_empty(), "{:?}", result.failed);
        assert_eq!(result.archived, 1);
        assert_eq!(result.isolated, 1);
        assert!(journal_path.exists());

        let rollback = journal::rollback(&journal_path).unwrap();
        assert!(rollback.failed.is_empty(), "{:?}", rollback.failed);
        assert_eq!(rollback.restored, 2);
        assert_eq!(std::fs::read(&a).unwrap(), b"same");
        assert_eq!(std::fs::read(&b).unwrap(), b"same");
    }

    #[test]
    fn execute_skips_unselected_items() {
        let dir = tempfile::tempdir().unwrap();
        let source = dir.path().join("a.jpg");
        let target = dir.path().join("target/a.jpg");
        std::fs::write(&source, b"a").unwrap();
        let item = PlanItem {
            id: "a".into(),
            source_path: source.clone(),
            target_path: Some(target),
            action: PlanAction::Archive,
            reason: Some("dated".into()),
            group_id: None,
            kind: MediaKind::Photo,
            size_bytes: 1,
            date: Some(Utc::now()),
            selected: false,
        };
        let cancel = AtomicBool::new(false);
        let result = execute(
            "run",
            &[item],
            &dir.path().join("journal.json"),
            &cancel,
            |_| {},
        );
        assert_eq!(result.archived, 0);
        assert!(source.exists());
    }
}
