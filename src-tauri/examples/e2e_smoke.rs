//! End-to-end acceptance smoke test for scan -> plan -> apply -> idempotence -> rollback.
//!
//! Run from `src-tauri`:
//! `cargo run --example e2e_smoke`

use std::path::Path;
use std::sync::atomic::AtomicBool;

use tourism_media_organizer_lib::apply;
use tourism_media_organizer_lib::date::ToolProbe;
use tourism_media_organizer_lib::journal;
use tourism_media_organizer_lib::models::{
    DateSource, DedupOptions, NamingOptions, PlanAction, ScanOptions,
};
use tourism_media_organizer_lib::{plan, scan};

fn options(source: &Path, target: &Path) -> ScanOptions {
    ScanOptions {
        source_dirs: vec![source.to_path_buf()],
        target_dir: target.to_path_buf(),
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

fn main() -> anyhow::Result<()> {
    let temp = tempfile::tempdir()?;
    let source = temp.path().join("source");
    let target = temp.path().join("organized");
    let a = source.join("album-a/2024-05-01_120000.jpg");
    let b = source.join("album-b/2024-05-01_120000.jpg");
    let video = source.join("2024-06-02_080000.MOV");
    let unknown = source.join("mystery.jpg");
    std::fs::create_dir_all(a.parent().unwrap())?;
    std::fs::create_dir_all(b.parent().unwrap())?;
    std::fs::write(&a, b"duplicate-bytes")?;
    std::fs::write(&b, b"duplicate-bytes")?;
    std::fs::write(&video, b"not-a-real-video-but-dated-by-filename")?;
    std::fs::write(&unknown, b"unknown-date")?;

    let cancel = AtomicBool::new(false);
    let scanning = scan::scan(
        "e2e",
        &options(&source, &target),
        &ToolProbe::default(),
        &cancel,
        |_| {},
    )?;
    assert_eq!(scanning.files.len(), 4);
    assert_eq!(scanning.stats.duplicate_groups, 1);
    assert_eq!(scanning.stats.unresolved, 1);

    let plan_items = plan::build_plan(
        &scanning.files,
        &scanning.byte_groups,
        &scanning.similar_groups,
        &options(&source, &target),
    )?;
    assert_eq!(plan_items.len(), 4);
    assert_eq!(
        plan_items
            .iter()
            .filter(|item| item.action == PlanAction::Isolate)
            .count(),
        1
    );
    assert_eq!(
        plan_items
            .iter()
            .filter(|item| item.action == PlanAction::Unresolved)
            .count(),
        1
    );

    let journal_path = temp.path().join("journals/e2e.json");
    let result = apply::execute("e2e", &plan_items, &journal_path, &cancel, |_| {});
    assert!(result.failed.is_empty(), "{:?}", result.failed);
    assert_eq!(result.archived, 2);
    assert_eq!(result.isolated, 1);
    assert_eq!(result.unresolved, 1);
    assert!(target.join("2024/2024-05/2024-05-01_120000.jpg").is_file());
    assert!(target.join("2024/2024-06/2024-06-02_080000.MOV").is_file());
    assert!(target.join("unresolved/mystery.jpg").is_file());
    assert!(target.join("_duplicates/2024/2024-05").is_dir());

    // 再次扫描已整理目录：归档项应为 Keep，不能产生二次移动。
    let rerun = scan::scan(
        "e2e-rerun",
        &options(&target, &target),
        &ToolProbe::default(),
        &cancel,
        |_| {},
    )?;
    assert_eq!(rerun.files.len(), 3);
    let rerun_plan = plan::build_plan(
        &rerun.files,
        &rerun.byte_groups,
        &rerun.similar_groups,
        &options(&target, &target),
    )?;
    assert!(
        rerun_plan
            .iter()
            .all(|item| item.action == PlanAction::Keep),
        "{rerun_plan:#?}"
    );

    let rolled_back = journal::rollback(&journal_path)?;
    assert!(rolled_back.failed.is_empty(), "{:?}", rolled_back.failed);
    assert_eq!(rolled_back.restored, 4);
    for original in [&a, &b, &video, &unknown] {
        assert!(
            original.is_file(),
            "missing restored file: {}",
            original.display()
        );
    }

    println!("E2E OK: scan, dedup, plan, apply, idempotent rerun, and rollback");
    Ok(())
}
