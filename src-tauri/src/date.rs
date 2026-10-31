//! M2 拍摄日期解析：按 `date_priority` 逐级降级。
//!
//! 优先级见 `PLAN.md §6`：EXIF Original → EXIF CreateDate → 视频元数据 →
//! 文件创建时间 → 文件修改时间 → 文件名内嵌日期。全部失败则 `unresolved`。
//!
//! `ToolProbe` 在应用启动时探测一次，避免每个文件都去尝试外部命令。

use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use chrono::{DateTime, NaiveDate, NaiveDateTime, NaiveTime, Utc};

use crate::models::{DateSource, MediaKind};

#[derive(Debug, Clone, Default)]
pub struct ToolProbe {
    pub exiftool: Option<PathBuf>,
    pub ffprobe: Option<PathBuf>,
}

impl ToolProbe {
    pub fn exiftool_available(&self) -> bool {
        self.exiftool.is_some()
    }

    pub fn ffprobe_available(&self) -> bool {
        self.ffprobe.is_some()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DateResolution {
    pub date: Option<DateTime<Utc>>,
    pub source: Option<DateSource>,
}

impl DateResolution {
    pub fn unresolved() -> Self {
        Self {
            date: None,
            source: None,
        }
    }

    pub fn is_resolved(&self) -> bool {
        self.date.is_some()
    }
}

const DEFAULT_ORDER: &[DateSource] = &[
    DateSource::ExifDateTimeOriginal,
    DateSource::ExifCreateDate,
    DateSource::VideoMetadata,
    DateSource::FileBirthTime,
    DateSource::FileMtime,
    DateSource::Filename,
];

pub fn resolve_date(
    path: &Path,
    kind: MediaKind,
    priority: &[DateSource],
    tools: &ToolProbe,
) -> DateResolution {
    let order: &[DateSource] = if priority.is_empty() {
        DEFAULT_ORDER
    } else {
        priority
    };

    let wants_exif = order.iter().any(|s| {
        matches!(
            s,
            DateSource::ExifDateTimeOriginal | DateSource::ExifCreateDate
        )
    });
    let exif = if wants_exif { read_exif(path) } else { None };

    for source in order {
        let naive = match source {
            DateSource::ExifDateTimeOriginal => exif_datetime(exif.as_ref(), path, tools, true),
            DateSource::ExifCreateDate => exif_datetime(exif.as_ref(), path, tools, false),
            DateSource::VideoMetadata if kind == MediaKind::Video => video_datetime(path, tools),
            DateSource::VideoMetadata => None,
            DateSource::FileBirthTime => file_time(path, true),
            DateSource::FileMtime => file_time(path, false),
            DateSource::Filename => filename_datetime(path),
        };

        if let Some(naive) = naive {
            return DateResolution {
                date: Some(naive.and_utc()),
                source: Some(*source),
            };
        }
    }

    DateResolution::unresolved()
}

pub(crate) fn command_available(cmd: &Path, arg: &str) -> bool {
    Command::new(cmd)
        .arg(arg)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn read_exif(path: &Path) -> Option<exif::Exif> {
    let file = std::fs::File::open(path).ok()?;
    let mut reader = std::io::BufReader::new(file);
    exif::Reader::new().read_from_container(&mut reader).ok()
}

/// `original=true` 取 DateTimeOriginal，否则取 CreateDate 系列。
fn exif_datetime(
    exif: Option<&exif::Exif>,
    path: &Path,
    tools: &ToolProbe,
    original: bool,
) -> Option<NaiveDateTime> {
    if let Some(exif) = exif {
        let tags: &[exif::Tag] = if original {
            &[exif::Tag::DateTimeOriginal]
        } else {
            &[exif::Tag::DateTime, exif::Tag::DateTimeDigitized]
        };
        for tag in tags {
            if let Some(field) = exif.get_field(*tag, exif::In::PRIMARY) {
                if let Some(dt) = parse_exif_field(field) {
                    return Some(dt);
                }
            }
        }
    }

    // HEIC / RAW 等 kamadak-exif 可能读不了的容器，用 exiftool 兜底。
    if let Some(exiftool) = &tools.exiftool {
        let keys: &[&str] = if original {
            &["DateTimeOriginal"]
        } else {
            &["CreateDate", "MediaCreateDate", "QuickTime:CreateDate"]
        };
        return exiftool_datetime(path, keys, exiftool);
    }

    None
}

fn parse_exif_field(field: &exif::Field) -> Option<NaiveDateTime> {
    match &field.value {
        exif::Value::Ascii(values) => {
            let bytes = values.first()?;
            let s = String::from_utf8_lossy(bytes);
            parse_datetime_flexible(&s)
        }
        _ => None,
    }
}

fn exiftool_datetime(path: &Path, keys: &[&str], executable: &Path) -> Option<NaiveDateTime> {
    let mut cmd = Command::new(executable);
    cmd.args(["-j", "-s"]);
    for key in keys {
        cmd.arg(format!("-{key}"));
    }
    let output = cmd.arg(path).output().ok()?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let obj = value.as_array()?.first()?.as_object()?;
    for key in keys {
        if let Some(s) = obj.get(*key).and_then(|v| v.as_str()) {
            if let Some(dt) = parse_datetime_flexible(s) {
                return Some(dt);
            }
        }
    }
    None
}

fn video_datetime(path: &Path, tools: &ToolProbe) -> Option<NaiveDateTime> {
    let ffprobe = tools.ffprobe.as_ref()?;
    let output = Command::new(ffprobe)
        .args(["-v", "quiet", "-print_format", "json", "-show_format"])
        .arg(path)
        .output()
        .ok()?;
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let tags = value.get("format")?.get("tags")?.as_object()?;
    for key in ["creation_time", "com.apple.quicktime.creationdate"] {
        if let Some(s) = tags.get(key).and_then(|v| v.as_str()) {
            if let Some(dt) = parse_datetime_flexible(s) {
                return Some(dt);
            }
        }
    }
    None
}

fn file_time(path: &Path, created: bool) -> Option<NaiveDateTime> {
    let metadata = std::fs::metadata(path).ok()?;
    let time = if created {
        metadata.created().ok()?
    } else {
        metadata.modified().ok()?
    };
    let dt: DateTime<Utc> = time.into();
    Some(dt.naive_utc())
}

pub(crate) fn parse_datetime_flexible(input: &str) -> Option<NaiveDateTime> {
    let s = input.trim().trim_end_matches('\0').trim();
    if s.is_empty() || s.starts_with("0000") {
        return None;
    }

    if let Ok(dt) = DateTime::parse_from_rfc3339(s) {
        return Some(dt.naive_utc());
    }

    const FORMATS: &[&str] = &[
        "%Y:%m:%d %H:%M:%S",
        "%Y-%m-%d %H:%M:%S",
        "%Y-%m-%dT%H:%M:%S%.f",
        "%Y-%m-%dT%H:%M:%S",
        "%Y-%m-%d %H:%M:%S%.f",
    ];
    for fmt in FORMATS {
        if let Ok(dt) = NaiveDateTime::parse_from_str(s, fmt) {
            return Some(dt);
        }
    }

    for fmt in ["%Y:%m:%d", "%Y-%m-%d"] {
        if let Ok(date) = NaiveDate::parse_from_str(s, fmt) {
            return date.and_hms_opt(0, 0, 0);
        }
    }

    None
}

pub(crate) fn filename_datetime(path: &Path) -> Option<NaiveDateTime> {
    let name = path.file_name()?.to_string_lossy();
    let chars: Vec<char> = name.chars().collect();

    for i in 0..chars.len() {
        let Some(year) = digits_at(&chars, i, 4) else {
            continue;
        };
        if !(1900..=2200).contains(&year) {
            continue;
        }

        let mut j = i + 4;
        if j < chars.len() && matches!(chars[j], '-' | '_' | '/' | '.') {
            j += 1;
        }
        let Some(month) = digits_at(&chars, j, 2) else {
            continue;
        };
        if !(1..=12).contains(&month) {
            continue;
        }
        j += 2;
        if j < chars.len() && matches!(chars[j], '-' | '_' | '/' | '.') {
            j += 1;
        }
        let Some(day) = digits_at(&chars, j, 2) else {
            continue;
        };
        if !(1..=31).contains(&day) {
            continue;
        }
        j += 2;

        let Some(date) = NaiveDate::from_ymd_opt(year as i32, month, day) else {
            continue;
        };

        let mut k = j;
        if k < chars.len() && matches!(chars[k], '_' | '-' | 'T' | ' ' | '.') {
            k += 1;
        }
        let time = time_at(&chars, k).unwrap_or_else(|| NaiveTime::from_hms_opt(0, 0, 0).unwrap());

        return Some(date.and_time(time));
    }

    None
}

fn digits_at(chars: &[char], start: usize, n: usize) -> Option<u32> {
    if start + n > chars.len() {
        return None;
    }
    let mut value = 0u32;
    for c in &chars[start..start + n] {
        let d = c.to_digit(10)?;
        value = value * 10 + d;
    }
    Some(value)
}

fn time_at(chars: &[char], start: usize) -> Option<NaiveTime> {
    // 先试紧凑 6 位 HHMMSS
    if let Some(v) = digits_at(chars, start, 6) {
        let (h, m, s) = (v / 10000, (v / 100) % 100, v % 100);
        if let Some(t) = NaiveTime::from_hms_opt(h, m, s) {
            return Some(t);
        }
    }

    let h = digits_at(chars, start, 2)?;
    let mut k = start + 2;
    if k < chars.len() && matches!(chars[k], ':' | '-' | '.') {
        k += 1;
    }
    let m = digits_at(chars, k, 2)?;
    k += 2;
    if k < chars.len() && matches!(chars[k], ':' | '-' | '.') {
        k += 1;
    }
    let s = digits_at(chars, k, 2).unwrap_or(0);
    NaiveTime::from_hms_opt(h, m, s)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe_none() -> ToolProbe {
        ToolProbe::default()
    }

    #[test]
    fn parses_exif_style_datetime() {
        let dt = parse_datetime_flexible("2024:05:01 12:34:56\0").unwrap();
        assert_eq!(dt.to_string(), "2024-05-01 12:34:56");
    }

    #[test]
    fn rejects_placeholder_datetime() {
        assert!(parse_datetime_flexible("0000:00:00 00:00:00").is_none());
        assert!(parse_datetime_flexible("").is_none());
    }

    #[test]
    fn parses_filename_patterns() {
        let a = filename_datetime(Path::new("/x/IMG_20240501_123456.jpg")).unwrap();
        assert_eq!(a.to_string(), "2024-05-01 12:34:56");

        let b = filename_datetime(Path::new("/x/2024-05-01.jpg")).unwrap();
        assert_eq!(b.to_string(), "2024-05-01 00:00:00");

        assert!(filename_datetime(Path::new("/x/random.jpg")).is_none());
    }

    #[test]
    fn resolves_from_filename_when_only_filename_allowed() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("2024-05-01_090000.jpg");
        std::fs::write(&path, b"x").unwrap();

        let res = resolve_date(
            &path,
            MediaKind::Photo,
            &[DateSource::Filename],
            &probe_none(),
        );
        assert!(res.is_resolved());
        assert_eq!(res.source, Some(DateSource::Filename));
        assert_eq!(res.date.unwrap().to_rfc3339(), "2024-05-01T09:00:00+00:00");
    }

    #[test]
    fn unresolved_when_no_source_matches() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("random.jpg");
        std::fs::write(&path, b"x").unwrap();

        let res = resolve_date(
            &path,
            MediaKind::Photo,
            &[DateSource::Filename],
            &probe_none(),
        );
        assert!(!res.is_resolved());
        assert_eq!(res.source, None);
    }
}
