//! 前后端共享的数据模型（M1 扫描阶段）。
//!
//! 与 `ARCHITECTURE.md §4` 的 serde 契约一一对应；字段尽量保持 serde 命名稳定，
//! 前端 `src/api/types.ts` 需按此结构镜像。

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediaKind {
    Photo,
    Raw,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DateSource {
    ExifDateTimeOriginal,
    ExifCreateDate,
    VideoMetadata,
    FileBirthTime,
    FileMtime,
    Filename,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DedupOptions {
    pub byte: bool,
    pub similar_images: Option<SimilarityOptions>,
    pub similar_videos: Option<SimilarityOptions>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimilarityOptions {
    pub hamming_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NamingOptions {
    pub prefix_date: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanOptions {
    pub source_dirs: Vec<PathBuf>,
    pub target_dir: PathBuf,
    pub recursive: bool,
    pub follow_symlinks: bool,
    pub include_raw: bool,
    pub include_heic: bool,
    pub date_priority: Vec<DateSource>,
    pub dedup: DedupOptions,
    pub naming: NamingOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MediaFile {
    pub id: String,
    pub source_path: PathBuf,
    pub kind: MediaKind,
    pub size_bytes: u64,
    pub ext: String,
    pub date: Option<DateTime<Utc>>,
    pub date_source: Option<DateSource>,
    pub resolved: bool,
    pub byte_hash: Option<String>,
    pub image_hash: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanPhase {
    Enumerating,
    ReadingMetadata,
    Hashing,
    Dedup,
    Done,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanProgress {
    pub run_id: String,
    pub phase: ScanPhase,
    pub total_files: u64,
    pub processed_files: u64,
    pub media_found: u64,
    pub duplicate_groups: u64,
    pub similar_groups: u64,
    pub unresolved: u64,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ScanStats {
    pub total_files: u64,
    pub media_found: u64,
    pub duplicate_groups: u64,
    pub similar_groups: u64,
    pub unresolved: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunStarted {
    pub run_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanDone {
    pub run_id: String,
    pub stats: ScanStats,
    pub canceled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanError {
    pub run_id: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScanStatus {
    pub run_id: String,
    pub phase: ScanPhase,
    pub done: bool,
    pub canceled: bool,
    pub stats: Option<ScanStats>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlanAction {
    Archive,
    Isolate,
    Keep,
    Unresolved,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanItem {
    pub id: String,
    pub source_path: PathBuf,
    pub target_path: Option<PathBuf>,
    pub action: PlanAction,
    pub reason: Option<String>,
    pub group_id: Option<String>,
    pub kind: MediaKind,
    pub size_bytes: u64,
    pub date: Option<DateTime<Utc>>,
    pub selected: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummary {
    pub run_id: String,
    pub items_total: u64,
    pub items_selected: u64,
    pub archive_count: u64,
    pub isolate_count: u64,
    pub unresolved_count: u64,
    pub keep_count: u64,
    pub freed_est_bytes: u64,
    pub duplicate_groups: u64,
    pub similar_groups: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanPage {
    pub items: Vec<PlanItem>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyProgress {
    pub run_id: String,
    pub done: u64,
    pub total: u64,
    pub phase: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Failure {
    pub path: PathBuf,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyResult {
    pub run_id: String,
    pub archived: u64,
    pub isolated: u64,
    pub unresolved: u64,
    pub freed_est_bytes: u64,
    pub failed: Vec<Failure>,
    pub journal_path: PathBuf,
    pub canceled: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RollbackResult {
    pub run_id: String,
    pub restored: u64,
    pub failed: Vec<Failure>,
    pub journal_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuarantineDeleteResult {
    pub run_id: String,
    pub deleted_files: u64,
    pub freed_bytes: u64,
    pub quarantine_path: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalEntry {
    pub action: String,
    pub from: PathBuf,
    pub to: PathBuf,
    pub hash: Option<String>,
    pub reason: Option<String>,
    pub timestamp: DateTime<Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JournalDocument {
    pub run_id: String,
    pub created_at: DateTime<Utc>,
    pub entries: Vec<JournalEntry>,
}
