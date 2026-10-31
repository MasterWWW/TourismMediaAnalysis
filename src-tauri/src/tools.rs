//! exiftool / ffprobe 探测、持久化与手动路径配置。
//!
//! GUI 应用从 Finder/开始菜单启动时通常拿不到 shell 的完整 PATH，因此除 PATH 外
//! 还会检查 Homebrew、Scoop、Chocolatey 等常见安装位置。

use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::date::{command_available, ToolProbe};

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ToolSettings {
    pub exiftool_path: Option<PathBuf>,
    pub ffprobe_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolStatus {
    pub exiftool_path: Option<PathBuf>,
    pub ffprobe_path: Option<PathBuf>,
}

impl From<&ToolProbe> for ToolStatus {
    fn from(value: &ToolProbe) -> Self {
        Self {
            exiftool_path: value.exiftool.clone(),
            ffprobe_path: value.ffprobe.clone(),
        }
    }
}

pub fn detect(settings: &ToolSettings) -> ToolProbe {
    ToolProbe {
        exiftool: detect_one(
            settings.exiftool_path.as_deref(),
            "exiftool",
            "-ver",
            exiftool_candidates(),
        ),
        ffprobe: detect_one(
            settings.ffprobe_path.as_deref(),
            "ffprobe",
            "-version",
            ffprobe_candidates(),
        ),
    }
}

pub fn load_settings() -> ToolSettings {
    let Ok(path) = settings_path() else {
        return ToolSettings::default();
    };
    std::fs::read(path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

pub fn save_settings(settings: &ToolSettings) -> anyhow::Result<()> {
    let path = settings_path()?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let temp = path.with_extension("json.tmp");
    std::fs::write(&temp, serde_json::to_vec_pretty(settings)?)?;
    if let Err(first_error) = std::fs::rename(&temp, &path) {
        if !path.exists() {
            return Err(first_error.into());
        }
        std::fs::remove_file(&path)?;
        std::fs::rename(&temp, &path)?;
    }
    Ok(())
}

pub fn validate_exiftool(path: &Path) -> bool {
    command_available(path, "-ver")
}

pub fn validate_ffprobe(path: &Path) -> bool {
    command_available(path, "-version")
}

fn detect_one(
    configured: Option<&Path>,
    command: &str,
    version_arg: &str,
    candidates: Vec<PathBuf>,
) -> Option<PathBuf> {
    let mut seen = HashSet::new();
    let command_path = PathBuf::from(command);
    let paths = configured
        .map(Path::to_path_buf)
        .into_iter()
        .chain(std::iter::once(command_path))
        .chain(candidates);
    for candidate in paths {
        if !seen.insert(candidate.clone()) {
            continue;
        }
        if command_available(&candidate, version_arg) {
            return Some(candidate);
        }
    }
    None
}

fn settings_path() -> anyhow::Result<PathBuf> {
    let dirs =
        directories::ProjectDirs::from("com", "TourismMediaOrganizer", "tourism-media-organizer")
            .ok_or_else(|| anyhow::anyhow!("cannot locate the application data directory"))?;
    Ok(dirs.config_dir().join("tools.json"))
}

fn exiftool_candidates() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/opt/homebrew/bin/exiftool"),
        PathBuf::from("/usr/local/bin/exiftool"),
        PathBuf::from("/usr/bin/exiftool"),
    ];
    paths.extend(windows_candidates(
        &[
            r"C:\Program Files\ExifTool\exiftool.exe",
            r"C:\Program Files\exiftool\exiftool.exe",
            r"C:\exiftool\exiftool.exe",
        ],
        &[
            "scoop\\shims\\exiftool.exe",
            "chocolatey\\bin\\exiftool.exe",
        ],
    ));
    dedupe(paths)
}

fn ffprobe_candidates() -> Vec<PathBuf> {
    let mut paths = vec![
        PathBuf::from("/opt/homebrew/bin/ffprobe"),
        PathBuf::from("/usr/local/bin/ffprobe"),
        PathBuf::from("/usr/bin/ffprobe"),
    ];
    paths.extend(windows_candidates(
        &[
            r"C:\Program Files\ffmpeg\bin\ffprobe.exe",
            r"C:\ffmpeg\bin\ffprobe.exe",
        ],
        &["scoop\\shims\\ffprobe.exe", "chocolatey\\bin\\ffprobe.exe"],
    ));
    dedupe(paths)
}

#[cfg(windows)]
fn windows_candidates(fixed: &[&str], relative: &[&str]) -> Vec<PathBuf> {
    let mut paths: Vec<PathBuf> = fixed.iter().map(PathBuf::from).collect();
    for key in ["USERPROFILE", "LOCALAPPDATA", "PROGRAMDATA"] {
        if let Some(base) = std::env::var_os(key) {
            let base = PathBuf::from(base);
            paths.extend(relative.iter().map(|suffix| base.join(suffix)));
        }
    }
    paths
}

#[cfg(not(windows))]
fn windows_candidates(_fixed: &[&str], _relative: &[&str]) -> Vec<PathBuf> {
    Vec::new()
}

fn dedupe(paths: Vec<PathBuf>) -> Vec<PathBuf> {
    let mut seen = HashSet::new();
    paths
        .into_iter()
        .filter(|path| seen.insert(path.clone()))
        .collect()
}
