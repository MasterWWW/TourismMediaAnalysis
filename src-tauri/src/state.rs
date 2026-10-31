//! 进程内会话状态：每个 `run_id` 对应一次扫描/整理会话。
//!
//! 后台线程与命令共享同一个 `Arc<Mutex<RunSession>>`，因此进度、取消标志、
//! inventory 都能被命令读取；日志仍由 `journal.rs` 落盘持久化。

use std::collections::HashMap;
use std::sync::atomic::AtomicBool;
use std::sync::{Arc, Mutex};

use crate::date::ToolProbe;
use crate::models::{
    ApplyResult, MediaFile, PlanItem, ScanOptions, ScanPhase, ScanProgress, ScanStats,
};
use crate::tools;

pub struct RunSession {
    pub run_id: String,
    pub options: ScanOptions,
    pub phase: ScanPhase,
    pub progress: ScanProgress,
    pub stats: Option<ScanStats>,
    pub inventory: Vec<MediaFile>,
    pub byte_groups: Vec<Vec<usize>>,
    pub similar_groups: Vec<Vec<usize>>,
    pub plan: Vec<PlanItem>,
    pub apply_result: Option<ApplyResult>,
    pub apply_running: bool,
    pub maintenance_running: bool,
    pub rolled_back: bool,
    pub quarantine_deleted: bool,
    pub done: bool,
    pub canceled: bool,
    pub error: Option<String>,
    pub cancel: Arc<AtomicBool>,
}

pub struct AppState {
    pub runs: Mutex<HashMap<String, Arc<Mutex<RunSession>>>>,
    pub tools: Mutex<ToolProbe>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            runs: Mutex::new(HashMap::new()),
            tools: Mutex::new(tools::detect(&tools::load_settings())),
        }
    }

    pub fn insert(&self, run_id: String, session: Arc<Mutex<RunSession>>) -> Result<(), String> {
        self.runs
            .lock()
            .map_err(|e| e.to_string())?
            .insert(run_id, session);
        Ok(())
    }

    pub fn get(&self, run_id: &str) -> Result<Arc<Mutex<RunSession>>, String> {
        self.runs
            .lock()
            .map_err(|e| e.to_string())?
            .get(run_id)
            .cloned()
            .ok_or_else(|| format!("unknown run_id: {run_id}"))
    }
}

impl Default for AppState {
    fn default() -> Self {
        Self::new()
    }
}
