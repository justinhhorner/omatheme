//! JSON files in the shared format (docs/data-format.md): camelCase keys, absent values omitted,
//! UTC dates with milliseconds and "Z", written atomically (temp file + rename).

use std::path::Path;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use anyhow::{Context, Result};
use chrono::{DateTime, NaiveDateTime, TimeDelta, TimeZone, Utc};
use serde::Serialize;
use serde::de::DeserializeOwned;

/// "2026-09-27T09:00:00.123Z": UTC, rounded to the nearest millisecond.
pub fn format_date(date: DateTime<Utc>) -> String {
    let millis = (date.timestamp_micros() as f64 / 1000.0).round() as i64;
    let rounded = Utc.timestamp_millis_opt(millis).single().unwrap_or(date);
    rounded.format("%Y-%m-%dT%H:%M:%S%.3fZ").to_string()
}

/// Any ISO 8601 date-time: any fraction length or none, "Z", a "+hh:mm"/"+hhmm" offset, or no zone
/// at all (older macOS files, which meant UTC). Fractions are read to the millisecond.
pub fn parse_date(text: &str) -> Option<DateTime<Utc>> {
    let b = text.as_bytes();
    let digits = |range: std::ops::Range<usize>| b.get(range.clone()).is_some_and(|s| s.iter().all(u8::is_ascii_digit));
    if b.len() < 19
        || !digits(0..4)
        || b[4] != b'-'
        || !digits(5..7)
        || b[7] != b'-'
        || !digits(8..10)
        || b[10] != b'T'
        || !digits(11..13)
        || b[13] != b':'
        || !digits(14..16)
        || b[16] != b':'
        || !digits(17..19)
    {
        return None;
    }
    let mut i = 19;
    let mut millis = 0i64;
    if b.get(i) == Some(&b'.') {
        let start = i + 1;
        let mut end = start;
        while end < b.len() && b[end].is_ascii_digit() {
            end += 1;
        }
        if end == start {
            return None;
        }
        let fraction: String = text[start..end].chars().take(3).collect();
        millis = format!("{fraction:0<3}").parse().ok()?;
        i = end;
    }
    let offset_seconds = match &text[i..] {
        "" | "Z" => 0,
        zone if zone.starts_with('+') || zone.starts_with('-') => {
            let rest = zone[1..].replace(':', "");
            if rest.len() != 4 || !rest.bytes().all(|c| c.is_ascii_digit()) {
                return None;
            }
            let seconds = rest[..2].parse::<i64>().ok()? * 3600 + rest[2..].parse::<i64>().ok()? * 60;
            if zone.starts_with('-') { -seconds } else { seconds }
        }
        _ => return None,
    };
    let naive = NaiveDateTime::parse_from_str(&text[..19], "%Y-%m-%dT%H:%M:%S").ok()?;
    Some(Utc.from_utc_datetime(&naive) + TimeDelta::milliseconds(millis) - TimeDelta::seconds(offset_seconds))
}

/// Serde adapters for dates in the shared format.
pub mod date {
    use super::*;
    use serde::{Deserialize, Deserializer, Serializer};

    pub fn serialize<S: Serializer>(date: &DateTime<Utc>, s: S) -> Result<S::Ok, S::Error> {
        s.serialize_str(&format_date(*date))
    }

    pub fn deserialize<'de, D: Deserializer<'de>>(d: D) -> Result<DateTime<Utc>, D::Error> {
        let text = String::deserialize(d)?;
        parse_date(&text).ok_or_else(|| serde::de::Error::custom(format!("Invalid date '{text}'.")))
    }
}

/// A 32-hex-digit id for temp files and staging folders (the Windows app uses GUIDs of the same
/// length, and its cleanup recognises names by it).
pub fn unique_id() -> String {
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0) as u64;
    let count = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mixed =
        (u64::from(std::process::id()) << 32) ^ count.wrapping_mul(0x9E37_79B9_7F4A_7C15) ^ nanos.rotate_left(17);
    format!("{nanos:016x}{mixed:016x}")
}

/// Writes via a uniquely named temp file + rename, creating the folder if needed, so a crash never
/// leaves a half-written file and two writers of one path don't collide.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).with_context(|| format!("Couldn't create {}", parent.display()))?;
    }
    let file_name = path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
    let temp = path.with_file_name(format!("{file_name}.{}.tmp", unique_id()));
    let result = std::fs::write(&temp, bytes).and_then(|_| std::fs::rename(&temp, path));
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result.with_context(|| format!("Couldn't write {}", path.display()))
}

/// Pretty JSON (UTF-8, no byte-order mark), written atomically.
pub fn write_json<T: Serialize + ?Sized>(path: &Path, value: &T) -> Result<()> {
    let mut bytes = serde_json::to_vec_pretty(value)?;
    bytes.push(b'\n');
    write_atomic(path, &bytes)
}

/// Parses JSON, ignoring a leading byte-order mark.
pub fn from_slice<T: DeserializeOwned>(bytes: &[u8]) -> serde_json::Result<T> {
    serde_json::from_slice(bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes))
}

/// Reads `path`, or None if it's missing or unreadable.
pub fn read_json<T: DeserializeOwned>(path: &Path) -> Option<T> {
    from_slice(&std::fs::read(path).ok()?).ok()
}

/// Deletes `path` if it exists. A missing file isn't an error.
pub fn remove_file_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_file(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

/// Deletes the folder `path` and everything in it if it exists.
pub fn remove_dir_if_present(path: &Path) -> std::io::Result<()> {
    match std::fs::remove_dir_all(path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        other => other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn downloaded_at() -> DateTime<Utc> {
        Utc.timestamp_millis_opt(1_790_499_600_123).unwrap() // 2026-09-27T09:00:00.123Z
    }

    #[test]
    fn dates_read_in_any_iso_form_and_write_as_utc_milliseconds() {
        for text in [
            "2026-09-27T09:00:00.123Z",
            "2026-09-27T09:00:00.1230000+00:00",
            "2026-09-27T11:00:00.123+02:00",
            "2026-09-27T05:00:00.123-0400",
            "2026-09-27T09:00:00.123", // older macOS files: no zone meant UTC
        ] {
            let date = parse_date(text).unwrap_or_else(|| panic!("{text}"));
            assert_eq!(date, downloaded_at(), "{text}");
            assert_eq!(format_date(date), "2026-09-27T09:00:00.123Z");
        }
    }

    #[test]
    fn dates_without_a_fraction_and_bad_dates() {
        assert_eq!(parse_date("2026-09-27T09:00:00Z"), Utc.timestamp_opt(1_790_499_600, 0).single());
        assert_eq!(parse_date("yesterday"), None);
        assert_eq!(parse_date("2026-09-27"), None);
        assert_eq!(parse_date("2026-09-27T09:00:00.Z"), None);
        assert_eq!(parse_date("2026-09-27T09:00:00+2"), None);
    }

    #[test]
    fn format_rounds_to_the_nearest_millisecond() {
        let date = Utc.timestamp_opt(1_790_499_600, 122_999_700).unwrap();
        assert_eq!(format_date(date), "2026-09-27T09:00:00.123Z");
    }

    #[test]
    fn write_atomic_creates_the_folder_and_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("a/b/file.json");

        write_json(&file, &serde_json::json!({ "k": "v" })).unwrap();

        assert_eq!(read_json::<serde_json::Value>(&file).unwrap()["k"], "v");
        let names: Vec<_> =
            std::fs::read_dir(dir.path().join("a/b")).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["file.json"]);
        let bytes = std::fs::read(&file).unwrap();
        assert!(!bytes.starts_with(b"\xEF\xBB\xBF"));
    }

    #[test]
    fn a_failed_write_leaves_no_temp_file() {
        let dir = tempfile::tempdir().unwrap();
        // A non-empty folder where the file should go makes the rename fail.
        let blocked = dir.path().join("blocked");
        std::fs::create_dir_all(blocked.join("x")).unwrap();

        assert!(write_atomic(&blocked, b"new").is_err());
        let names: Vec<_> = std::fs::read_dir(dir.path()).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(names, ["blocked"]);
    }

    #[test]
    fn unique_ids_are_32_hex_digits_and_differ() {
        let a = unique_id();
        let b = unique_id();
        assert_eq!(a.len(), 32);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
        assert_ne!(a, b);
    }

    #[test]
    fn reading_ignores_a_byte_order_mark() {
        let value: serde_json::Value = from_slice(b"\xEF\xBB\xBF{\"a\":1}").unwrap();
        assert_eq!(value["a"], 1);
    }
}
