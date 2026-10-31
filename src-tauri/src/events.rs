//! Tauri 事件名统一出口，避免前端/后端字符串漂移。
//! 事件契约见 `ARCHITECTURE.md §4.2`。

pub const SCAN_PROGRESS: &str = "scan://progress";
pub const SCAN_DONE: &str = "scan://done";
pub const SCAN_ERROR: &str = "scan://error";
pub const APPLY_PROGRESS: &str = "apply://progress";
pub const APPLY_DONE: &str = "apply://done";
