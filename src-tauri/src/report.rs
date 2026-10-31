//! 计划/执行结果汇总。汇总只描述当前选中项，便于 GUI 在执行前实时预览影响范围。

use crate::models::{PlanAction, PlanItem, PlanSummary};

pub fn plan_summary(
    run_id: &str,
    items: &[PlanItem],
    byte_groups: usize,
    similar_groups: usize,
) -> PlanSummary {
    let actionable = items
        .iter()
        .filter(|item| item.selected && item.action != PlanAction::Keep);
    let archive_count = actionable
        .clone()
        .filter(|item| item.action == PlanAction::Archive)
        .count() as u64;
    let isolate_count = actionable
        .clone()
        .filter(|item| item.action == PlanAction::Isolate)
        .count() as u64;
    let unresolved_count = actionable
        .clone()
        .filter(|item| item.action == PlanAction::Unresolved)
        .count() as u64;

    PlanSummary {
        run_id: run_id.to_owned(),
        items_total: items.len() as u64,
        items_selected: archive_count + isolate_count + unresolved_count,
        archive_count,
        isolate_count,
        unresolved_count,
        keep_count: items
            .iter()
            .filter(|item| item.action == PlanAction::Keep)
            .count() as u64,
        freed_est_bytes: items
            .iter()
            .filter(|item| item.selected && item.action == PlanAction::Isolate)
            .map(|item| item.size_bytes)
            .sum(),
        duplicate_groups: byte_groups as u64,
        similar_groups: similar_groups as u64,
    }
}

pub fn directory_stats(root: &std::path::Path) -> std::io::Result<(u64, u64)> {
    if !root.exists() {
        return Ok((0, 0));
    }
    let mut files = 0u64;
    let mut bytes = 0u64;
    for entry in jwalk::WalkDir::new(root).skip_hidden(false) {
        let entry = entry?;
        if entry.file_type().is_file() {
            files += 1;
            bytes += entry.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        }
    }
    Ok((files, bytes))
}

pub fn delete_quarantine_dir(root: &std::path::Path) -> anyhow::Result<(u64, u64)> {
    if root.file_name().and_then(|name| name.to_str()) != Some("_duplicates") {
        anyhow::bail!("refusing to delete a directory not named _duplicates");
    }
    if root
        .symlink_metadata()
        .is_ok_and(|metadata| metadata.file_type().is_symlink())
    {
        anyhow::bail!("refusing to delete a symlinked quarantine directory");
    }
    let stats = directory_stats(root)?;
    if root.exists() {
        std::fs::remove_dir_all(root)?;
    }
    Ok(stats)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::MediaKind;
    use std::path::PathBuf;

    fn item(action: PlanAction, selected: bool, size: u64) -> PlanItem {
        PlanItem {
            id: "x".into(),
            source_path: PathBuf::from("/x"),
            target_path: Some(PathBuf::from("/y")),
            action,
            reason: None,
            group_id: None,
            kind: MediaKind::Photo,
            size_bytes: size,
            date: None,
            selected,
        }
    }

    #[test]
    fn summary_only_counts_selected_actions() {
        let items = vec![
            item(PlanAction::Archive, true, 10),
            item(PlanAction::Isolate, true, 20),
            item(PlanAction::Isolate, false, 30),
            item(PlanAction::Keep, false, 40),
        ];
        let summary = plan_summary("r", &items, 1, 2);
        assert_eq!(summary.items_selected, 2);
        assert_eq!(summary.archive_count, 1);
        assert_eq!(summary.isolate_count, 1);
        assert_eq!(summary.keep_count, 1);
        assert_eq!(summary.freed_est_bytes, 20);
    }

    #[test]
    fn quarantine_delete_is_limited_to_duplicates_directory() {
        let dir = tempfile::tempdir().unwrap();
        let quarantine = dir.path().join("_duplicates/2024");
        std::fs::create_dir_all(&quarantine).unwrap();
        std::fs::write(quarantine.join("a.jpg"), b"abc").unwrap();

        assert!(delete_quarantine_dir(dir.path()).is_err());
        let (files, bytes) = delete_quarantine_dir(&dir.path().join("_duplicates")).unwrap();
        assert_eq!((files, bytes), (1, 3));
        assert!(!dir.path().join("_duplicates").exists());
    }
}
