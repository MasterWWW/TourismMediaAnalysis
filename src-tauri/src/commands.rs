//! Tauri 命令薄层：只做参数校验、会话调度与事件转发，业务在 `scan`/`plan`/`apply`。

use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use tauri::{AppHandle, Emitter, State};

use crate::apply;
use crate::events;
use crate::journal;
use crate::models::{
    ApplyResult, MediaFile, PlanPage, PlanSummary, QuarantineDeleteResult, RollbackResult,
    RunStarted, ScanDone, ScanError, ScanOptions, ScanPhase, ScanProgress, ScanStatus,
};
use crate::plan;
use crate::report;
use crate::scan;
use crate::state::{AppState, RunSession};
use crate::tools::{self, ToolSettings, ToolStatus};

/// 启动一次扫描；立即返回 `run_id`，进度通过 `scan://progress` 事件推送。
#[tauri::command]
pub fn scan(
    app: AppHandle,
    state: State<'_, AppState>,
    options: ScanOptions,
) -> Result<RunStarted, String> {
    if options.source_dirs.is_empty() {
        return Err("至少选择一个源目录".into());
    }
    if options.target_dir.as_os_str().is_empty() {
        return Err("请选择目标根目录".into());
    }
    for source in &options.source_dirs {
        if !source.is_dir() {
            return Err(format!("源目录不存在或不可访问: {}", source.display()));
        }
    }
    if options.target_dir.exists() && !options.target_dir.is_dir() {
        return Err(format!(
            "目标路径不是目录: {}",
            options.target_dir.display()
        ));
    }
    let run_id = uuid::Uuid::new_v4().to_string();
    let cancel = Arc::new(std::sync::atomic::AtomicBool::new(false));

    let initial = ScanProgress {
        run_id: run_id.clone(),
        phase: ScanPhase::Enumerating,
        total_files: 0,
        processed_files: 0,
        media_found: 0,
        duplicate_groups: 0,
        similar_groups: 0,
        unresolved: 0,
        message: None,
    };
    let tools = state.tools.lock().map_err(|e| e.to_string())?.clone();

    let session = Arc::new(Mutex::new(RunSession {
        run_id: run_id.clone(),
        options: options.clone(),
        phase: ScanPhase::Enumerating,
        progress: initial,
        stats: None,
        inventory: Vec::new(),
        byte_groups: Vec::new(),
        similar_groups: Vec::new(),
        plan: Vec::new(),
        apply_result: None,
        apply_running: false,
        maintenance_running: false,
        rolled_back: false,
        quarantine_deleted: false,
        done: false,
        canceled: false,
        error: None,
        cancel: cancel.clone(),
    }));

    state.insert(run_id.clone(), session.clone())?;

    let app_handle = app.clone();
    let thread_run_id = run_id.clone();
    let thread_cancel = cancel.clone();
    let thread_session = session.clone();

    std::thread::spawn(move || {
        let progress_session = thread_session.clone();
        let progress_app = app_handle.clone();

        let result = scan::scan(&thread_run_id, &options, &tools, &thread_cancel, |p| {
            if let Ok(mut s) = progress_session.lock() {
                s.phase = p.phase;
                s.progress = p.clone();
            }
            let _ = progress_app.emit(events::SCAN_PROGRESS, p);
        });

        match result {
            Ok(outcome) => {
                let canceled = thread_cancel.load(Ordering::Relaxed);
                let stats = outcome.stats;
                tracing::info!(
                    run_id = %thread_run_id,
                    total_files = stats.total_files,
                    media_found = stats.media_found,
                    duplicate_groups = stats.duplicate_groups,
                    similar_groups = stats.similar_groups,
                    unresolved = stats.unresolved,
                    canceled,
                    "scan completed"
                );
                let planned = if canceled {
                    Ok(Vec::new())
                } else {
                    plan::build_plan(
                        &outcome.files,
                        &outcome.byte_groups,
                        &outcome.similar_groups,
                        &options,
                    )
                };
                if let Err(err) = &planned {
                    if let Ok(mut s) = thread_session.lock() {
                        s.phase = ScanPhase::Done;
                        s.done = true;
                        s.error = Some(format!("plan generation failed: {err}"));
                    }
                    let _ = app_handle.emit(
                        events::SCAN_ERROR,
                        ScanError {
                            run_id: thread_run_id,
                            message: format!("plan generation failed: {err}"),
                        },
                    );
                    return;
                }
                if let Ok(mut s) = thread_session.lock() {
                    s.phase = ScanPhase::Done;
                    s.progress = ScanProgress {
                        run_id: thread_run_id.clone(),
                        phase: ScanPhase::Done,
                        total_files: stats.total_files,
                        processed_files: stats.total_files,
                        media_found: stats.media_found,
                        duplicate_groups: stats.duplicate_groups,
                        similar_groups: stats.similar_groups,
                        unresolved: stats.unresolved,
                        message: None,
                    };
                    s.stats = Some(stats);
                    s.inventory = outcome.files;
                    s.byte_groups = outcome.byte_groups;
                    s.similar_groups = outcome.similar_groups;
                    s.plan = planned.unwrap_or_default();
                    s.done = true;
                    s.canceled = canceled;
                }
                let _ = app_handle.emit(
                    events::SCAN_DONE,
                    ScanDone {
                        run_id: thread_run_id,
                        stats,
                        canceled,
                    },
                );
            }
            Err(err) => {
                let message = err.to_string();
                tracing::error!(run_id = %thread_run_id, error = %message, "scan failed");
                if let Ok(mut s) = thread_session.lock() {
                    s.phase = ScanPhase::Done;
                    s.done = true;
                    s.error = Some(message.clone());
                }
                let _ = app_handle.emit(
                    events::SCAN_ERROR,
                    ScanError {
                        run_id: thread_run_id,
                        message,
                    },
                );
            }
        }
    });

    Ok(RunStarted { run_id })
}

/// 请求取消指定 run；扫描循环在下一次检查时退出。
#[tauri::command]
pub fn cancel_scan(state: State<'_, AppState>, run_id: String) -> Result<(), String> {
    let session = state.get(&run_id)?;
    let guard = session.lock().map_err(|e| e.to_string())?;
    guard.cancel.store(true, Ordering::Relaxed);
    Ok(())
}

#[tauri::command]
pub fn get_scan_status(state: State<'_, AppState>, run_id: String) -> Result<ScanStatus, String> {
    let session = state.get(&run_id)?;
    let s = session.lock().map_err(|e| e.to_string())?;
    Ok(ScanStatus {
        run_id: s.run_id.clone(),
        phase: s.phase,
        done: s.done,
        canceled: s.canceled,
        stats: s.stats,
        error: s.error.clone(),
    })
}

/// 把当前 run 的只读扫描库存返回给前端。
#[tauri::command]
pub fn get_inventory(state: State<'_, AppState>, run_id: String) -> Result<Vec<MediaFile>, String> {
    let session = state.get(&run_id)?;
    let s = session.lock().map_err(|e| e.to_string())?;
    Ok(s.inventory.clone())
}

#[tauri::command]
pub fn get_plan_summary(state: State<'_, AppState>, run_id: String) -> Result<PlanSummary, String> {
    let session = state.get(&run_id)?;
    let s = session.lock().map_err(|e| e.to_string())?;
    if !s.done {
        return Err("scan is still running".into());
    }
    Ok(report::plan_summary(
        &run_id,
        &s.plan,
        s.byte_groups.len(),
        s.similar_groups.len(),
    ))
}

#[tauri::command]
pub fn get_plan_page(
    state: State<'_, AppState>,
    run_id: String,
    cursor: Option<String>,
    page_size: u32,
) -> Result<PlanPage, String> {
    let session = state.get(&run_id)?;
    let s = session.lock().map_err(|e| e.to_string())?;
    let offset = cursor
        .as_deref()
        .unwrap_or("0")
        .parse::<usize>()
        .map_err(|_| "invalid plan cursor".to_string())?;
    let size = page_size.clamp(1, 500) as usize;
    let start = offset.min(s.plan.len());
    let end = (start + size).min(s.plan.len());
    Ok(PlanPage {
        items: s.plan[start..end].to_vec(),
        next_cursor: (end < s.plan.len()).then(|| end.to_string()),
    })
}

#[tauri::command]
pub fn set_item_selected(
    state: State<'_, AppState>,
    run_id: String,
    item_id: String,
    selected: bool,
) -> Result<(), String> {
    let session = state.get(&run_id)?;
    let mut s = session.lock().map_err(|e| e.to_string())?;
    let item = s
        .plan
        .iter_mut()
        .find(|item| item.id == item_id)
        .ok_or_else(|| format!("unknown plan item: {item_id}"))?;
    item.selected = selected && !matches!(item.action, crate::models::PlanAction::Keep);
    Ok(())
}

#[tauri::command]
pub fn set_all_selected(
    state: State<'_, AppState>,
    run_id: String,
    selected: bool,
) -> Result<(), String> {
    let session = state.get(&run_id)?;
    let mut s = session.lock().map_err(|e| e.to_string())?;
    for item in &mut s.plan {
        item.selected = selected && !matches!(item.action, crate::models::PlanAction::Keep);
    }
    Ok(())
}

/// 启动执行线程。命令立即返回；进度与结果分别通过 `apply://progress` / `apply://done` 推送。
#[tauri::command]
pub fn apply(
    app: AppHandle,
    state: State<'_, AppState>,
    run_id: String,
) -> Result<RunStarted, String> {
    let session = state.get(&run_id)?;
    let (items, cancel, journal_path) = {
        let mut s = session.lock().map_err(|e| e.to_string())?;
        if !s.done || s.error.is_some() {
            return Err("scan is not ready for apply".into());
        }
        if s.apply_running {
            return Err("apply is already running".into());
        }
        if s.maintenance_running {
            return Err("rollback or quarantine cleanup is still running".into());
        }
        if s.apply_result.is_some() {
            return Err(
                "this run has already been applied; roll it back or start a new scan".into(),
            );
        }
        if s.rolled_back {
            return Err("this run has already been rolled back".into());
        }
        if !s
            .plan
            .iter()
            .any(|item| item.selected && item.action != crate::models::PlanAction::Keep)
        {
            return Err("no plan items are selected".into());
        }
        s.apply_running = true;
        s.cancel.store(false, Ordering::Relaxed);
        (
            s.plan.clone(),
            s.cancel.clone(),
            journal::journal_path_for(&run_id).map_err(|e| e.to_string())?,
        )
    };

    let app_handle = app.clone();
    let thread_run_id = run_id.clone();
    let thread_session = session.clone();
    std::thread::spawn(move || {
        let progress_app = app_handle.clone();
        let progress_run_id = thread_run_id.clone();
        let result = apply::execute(
            &thread_run_id,
            &items,
            &journal_path,
            &cancel,
            move |mut progress| {
                progress.run_id = progress_run_id.clone();
                let _ = progress_app.emit(events::APPLY_PROGRESS, progress);
            },
        );
        tracing::info!(
            run_id = %thread_run_id,
            archived = result.archived,
            isolated = result.isolated,
            unresolved = result.unresolved,
            failed = result.failed.len(),
            canceled = result.canceled,
            journal = %result.journal_path.display(),
            "apply completed"
        );

        if let Ok(mut s) = thread_session.lock() {
            s.apply_running = false;
            s.apply_result = Some(result.clone());
        }
        let _ = app_handle.emit(events::APPLY_DONE, result);
    });

    Ok(RunStarted { run_id })
}

#[tauri::command]
pub fn get_apply_result(state: State<'_, AppState>, run_id: String) -> Result<ApplyResult, String> {
    let session = state.get(&run_id)?;
    let s = session.lock().map_err(|e| e.to_string())?;
    s.apply_result
        .clone()
        .ok_or_else(|| "apply has not completed".into())
}

#[tauri::command]
pub fn rollback(state: State<'_, AppState>, run_id: String) -> Result<RollbackResult, String> {
    let session = state.get(&run_id)?;
    let journal_path = {
        let mut s = session.lock().map_err(|e| e.to_string())?;
        if s.apply_running {
            return Err("apply is still running".into());
        }
        if s.maintenance_running {
            return Err("another maintenance operation is still running".into());
        }
        if s.rolled_back {
            return Err("this run has already been rolled back".into());
        }
        if s.quarantine_deleted {
            return Err("the quarantine was deleted; this run can no longer be rolled back".into());
        }
        let path = s
            .apply_result
            .as_ref()
            .map(|result| result.journal_path.clone())
            .ok_or_else(|| "apply has not completed".to_string())?;
        s.maintenance_running = true;
        path
    };

    let rollback_result = journal::rollback(&journal_path);
    if let Ok(mut s) = session.lock() {
        s.maintenance_running = false;
        if rollback_result.is_ok() {
            s.rolled_back = true;
        }
    }
    let result = rollback_result.map_err(|e| e.to_string())?;
    tracing::warn!(
        run_id = %run_id,
        restored = result.restored,
        failed = result.failed.len(),
        journal = %journal_path.display(),
        "rollback completed"
    );
    Ok(result)
}

#[tauri::command]
pub fn delete_quarantine(
    state: State<'_, AppState>,
    run_id: String,
    confirmation: String,
) -> Result<QuarantineDeleteResult, String> {
    if confirmation != "DELETE_QUARANTINE" {
        return Err("invalid destructive-action confirmation".into());
    }
    let session = state.get(&run_id)?;
    let quarantine_path = {
        let mut s = session.lock().map_err(|e| e.to_string())?;
        if s.apply_running {
            return Err("apply is still running".into());
        }
        if s.maintenance_running {
            return Err("another maintenance operation is still running".into());
        }
        if s.rolled_back {
            return Err("this run has already been rolled back".into());
        }
        if s.apply_result.is_none() {
            return Err("apply has not completed".into());
        }
        if s.quarantine_deleted {
            return Err("quarantine has already been deleted".into());
        }
        s.maintenance_running = true;
        s.options.target_dir.join("_duplicates")
    };

    let delete_result = report::delete_quarantine_dir(&quarantine_path);
    if let Ok(mut s) = session.lock() {
        s.maintenance_running = false;
        if delete_result.is_ok() {
            s.quarantine_deleted = true;
        }
    }
    let (deleted_files, freed_bytes) = delete_result.map_err(|e| e.to_string())?;
    tracing::warn!(
        run_id = %run_id,
        deleted_files,
        freed_bytes,
        path = %quarantine_path.display(),
        "quarantine permanently deleted"
    );

    Ok(QuarantineDeleteResult {
        run_id,
        deleted_files,
        freed_bytes,
        quarantine_path,
    })
}

#[tauri::command]
pub fn get_tool_status(state: State<'_, AppState>) -> Result<ToolStatus, String> {
    let tools = state.tools.lock().map_err(|e| e.to_string())?;
    Ok(ToolStatus::from(&*tools))
}

#[tauri::command]
pub fn set_tool_paths(
    state: State<'_, AppState>,
    exiftool_path: Option<String>,
    ffprobe_path: Option<String>,
) -> Result<ToolStatus, String> {
    let exiftool = exiftool_path
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from);
    let ffprobe = ffprobe_path
        .filter(|path| !path.trim().is_empty())
        .map(std::path::PathBuf::from);

    if let Some(path) = &exiftool {
        if !tools::validate_exiftool(path) {
            return Err(format!("exiftool 无法执行: {}", path.display()));
        }
    }
    if let Some(path) = &ffprobe {
        if !tools::validate_ffprobe(path) {
            return Err(format!("ffprobe 无法执行: {}", path.display()));
        }
    }

    let settings = ToolSettings {
        exiftool_path: exiftool,
        ffprobe_path: ffprobe,
    };
    tools::save_settings(&settings).map_err(|e| e.to_string())?;
    let probe = tools::detect(&settings);
    let status = ToolStatus::from(&probe);
    *state.tools.lock().map_err(|e| e.to_string())? = probe;
    Ok(status)
}
