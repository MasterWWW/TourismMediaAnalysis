// Rust `src-tauri/src/models.rs` 的 TS 镜像（serde snake_case 契约）。

export type MediaKind = "photo" | "raw" | "video";

export type DateSource =
  | "exif_date_time_original"
  | "exif_create_date"
  | "video_metadata"
  | "file_birth_time"
  | "file_mtime"
  | "filename";

export type ScanPhase =
  | "enumerating"
  | "reading_metadata"
  | "hashing"
  | "dedup"
  | "done";

export interface SimilarityOptions {
  hamming_threshold: number;
}

export interface DedupOptions {
  byte: boolean;
  similar_images: SimilarityOptions | null;
  similar_videos: SimilarityOptions | null;
}

export interface NamingOptions {
  prefix_date: boolean;
}

export interface ScanOptions {
  source_dirs: string[];
  target_dir: string;
  recursive: boolean;
  follow_symlinks: boolean;
  include_raw: boolean;
  include_heic: boolean;
  date_priority: DateSource[];
  dedup: DedupOptions;
  naming: NamingOptions;
}

export interface MediaFile {
  id: string;
  source_path: string;
  kind: MediaKind;
  size_bytes: number;
  ext: string;
  date: string | null;
  date_source: DateSource | null;
  resolved: boolean;
  byte_hash: string | null;
  image_hash: string | null;
}

export interface RunStarted {
  run_id: string;
}

export interface ScanStats {
  total_files: number;
  media_found: number;
  duplicate_groups: number;
  similar_groups: number;
  unresolved: number;
}

export interface ScanProgress {
  run_id: string;
  phase: ScanPhase;
  total_files: number;
  processed_files: number;
  media_found: number;
  duplicate_groups: number;
  similar_groups: number;
  unresolved: number;
  message: string | null;
}

export interface ScanDone {
  run_id: string;
  stats: ScanStats;
  canceled: boolean;
}

export interface ScanError {
  run_id: string;
  message: string;
}

export interface ScanStatus {
  run_id: string;
  phase: ScanPhase;
  done: boolean;
  canceled: boolean;
  stats: ScanStats | null;
  error: string | null;
}

export type PlanAction = "archive" | "isolate" | "keep" | "unresolved";

export interface PlanItem {
  id: string;
  source_path: string;
  target_path: string | null;
  action: PlanAction;
  reason: string | null;
  group_id: string | null;
  kind: MediaKind;
  size_bytes: number;
  date: string | null;
  selected: boolean;
}

export interface PlanSummary {
  run_id: string;
  items_total: number;
  items_selected: number;
  archive_count: number;
  isolate_count: number;
  unresolved_count: number;
  keep_count: number;
  freed_est_bytes: number;
  duplicate_groups: number;
  similar_groups: number;
}

export interface PlanPage {
  items: PlanItem[];
  next_cursor: string | null;
}

export interface ApplyProgress {
  run_id: string;
  done: number;
  total: number;
  phase: string;
}

export interface Failure {
  path: string;
  code: string;
  message: string;
}

export interface ApplyResult {
  run_id: string;
  archived: number;
  isolated: number;
  unresolved: number;
  freed_est_bytes: number;
  failed: Failure[];
  journal_path: string;
  canceled: boolean;
}

export interface RollbackResult {
  run_id: string;
  restored: number;
  failed: Failure[];
  journal_path: string;
}

export interface ToolStatus {
  exiftool_path: string | null;
  ffprobe_path: string | null;
}

export interface QuarantineDeleteResult {
  run_id: string;
  deleted_files: number;
  freed_bytes: number;
  quarantine_path: string;
}
