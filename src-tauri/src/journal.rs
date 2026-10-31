//! 执行日志与一次性回滚。
//!
//! 日志只在移动成功后追加，按逆序回滚即可恢复执行前的路径。任何回滚冲突都只记录
//! 到失败清单，不覆盖现有文件。

use std::path::{Path, PathBuf};

use chrono::Utc;
use directories::ProjectDirs;

use crate::apply::move_no_overwrite;
use crate::models::{Failure, JournalDocument, JournalEntry, RollbackResult};

pub fn default_journal_dir() -> anyhow::Result<PathBuf> {
    let dirs = ProjectDirs::from("com", "TourismMediaOrganizer", "tourism-media-organizer")
        .ok_or_else(|| anyhow::anyhow!("cannot locate the application data directory"))?;
    Ok(dirs.data_local_dir().join("journals"))
}

pub fn journal_path_for(run_id: &str) -> anyhow::Result<PathBuf> {
    let safe_id: String = run_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '_'
            }
        })
        .collect();
    Ok(default_journal_dir()?.join(format!("{safe_id}.json")))
}

pub fn write_journal(path: &Path, run_id: &str, entries: Vec<JournalEntry>) -> anyhow::Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let doc = JournalDocument {
        run_id: run_id.to_owned(),
        created_at: Utc::now(),
        entries,
    };
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(&doc)?)?;
    if let Err(first_error) = std::fs::rename(&temp, path) {
        if !path.exists() {
            return Err(first_error.into());
        }
        // Windows 不允许 rename 覆盖现有文件，先替换应用自己的日志文件。
        std::fs::remove_file(path)?;
        std::fs::rename(&temp, path)?;
    }
    Ok(())
}

pub fn read_journal(path: &Path) -> anyhow::Result<JournalDocument> {
    let bytes = std::fs::read(path)?;
    Ok(serde_json::from_slice(&bytes)?)
}

pub fn rollback(path: &Path) -> anyhow::Result<RollbackResult> {
    let doc = read_journal(path)?;
    let mut restored = 0u64;
    let mut failed = Vec::new();

    for entry in doc.entries.iter().rev() {
        match move_no_overwrite(&entry.to, &entry.from) {
            Ok(()) => restored += 1,
            Err(err) => failed.push(Failure {
                path: entry.to.clone(),
                code: "rollback_failed".into(),
                message: format!("{} -> {}: {err}", entry.to.display(), entry.from.display()),
            }),
        }
    }

    Ok(RollbackResult {
        run_id: doc.run_id,
        restored,
        failed,
        journal_path: path.to_path_buf(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    #[test]
    fn writes_and_reads_journal() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("run.json");
        let entry = JournalEntry {
            action: "archive".into(),
            from: PathBuf::from("/tmp/a"),
            to: PathBuf::from("/tmp/b"),
            hash: Some("abc".into()),
            reason: Some("dated".into()),
            timestamp: Utc::now(),
        };
        write_journal(&path, "run", vec![entry]).unwrap();
        let doc = read_journal(&path).unwrap();
        assert_eq!(doc.run_id, "run");
        assert_eq!(doc.entries.len(), 1);
    }
}
