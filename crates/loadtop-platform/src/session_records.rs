//! Claude Code's session records, read from `~/.claude/sessions/<pid>.json`.

use std::fs::DirEntry;
use std::io::{ErrorKind, Read};
use std::path::PathBuf;

use loadtop_core::{SessionRecord, SessionRecords, SessionRecordsError};
use serde_json::{Map, Value};

/// The extension of a session record; the `*.key` files beside them are never opened.
const RECORD_EXTENSION: &str = ".json";

/// The largest record read; a larger file is skipped, so a stray file cannot grow memory.
const MAX_RECORD_BYTES: u64 = 64 * 1024;

/// ctime's weekday names, Sunday first; 1970-01-01 was a Thursday.
const WEEKDAYS: [&str; 7] = ["Sun", "Mon", "Tue", "Wed", "Thu", "Fri", "Sat"];

/// ctime's month names.
const MONTHS: [&str; 12] = [
    "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
];

/// Session records, read from one directory on every call.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionRecordsDir {
    dir: PathBuf,
}

impl SessionRecordsDir {
    /// Records read from `dir` (the binary passes [`crate::claude_sessions_dir`]).
    #[must_use]
    pub const fn new(dir: PathBuf) -> Self {
        Self { dir }
    }
}

impl SessionRecords for SessionRecordsDir {
    fn read(&self) -> Result<Vec<SessionRecord>, SessionRecordsError> {
        let entries = match std::fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(vec![]),
            Err(_) => return Err(SessionRecordsError::Unreadable),
        };
        let mut records = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|_| SessionRecordsError::Unreadable)?;
            let Some(file_pid) = entry.file_name().to_str().and_then(record_file_pid) else {
                continue;
            };
            let Some(bytes) = read_record_file(&entry) else {
                continue;
            };
            if let Some(record) = parse_record(file_pid, &bytes) {
                records.push(record);
            }
        }
        Ok(records)
    }
}

/// A record file's bytes; `None` for anything but a regular file of at most
/// [`MAX_RECORD_BYTES`] (a symlink, a FIFO that would block every sample, a device) or a
/// file removed since the listing, which is a session that just ended.
fn read_record_file(entry: &DirEntry) -> Option<Vec<u8>> {
    // `DirEntry::metadata` does not follow a symlink.
    let metadata = entry.metadata().ok()?;
    if !metadata.is_file() || metadata.len() > MAX_RECORD_BYTES {
        return None;
    }
    let file = std::fs::File::open(entry.path()).ok()?;
    let mut bytes = Vec::new();
    // Capped again while reading, in case the file grew since its size was read.
    file.take(MAX_RECORD_BYTES + 1)
        .read_to_end(&mut bytes)
        .ok()?;
    (u64::try_from(bytes.len()).ok()? <= MAX_RECORD_BYTES).then_some(bytes)
}

/// The PID a record's file is named after: `<digits>.json`, nothing else.
fn record_file_pid(name: &str) -> Option<u32> {
    let stem = name.strip_suffix(RECORD_EXTENSION)?;
    if stem.is_empty() || !stem.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    stem.parse().ok()
}

/// A record from a file's bytes, each field read on its own so one of the wrong type is
/// `None` rather than losing the record; `None` when the file is not a JSON object.
fn parse_record(file_pid: u32, bytes: &[u8]) -> Option<SessionRecord> {
    let Ok(Value::Object(fields)) = serde_json::from_slice::<Value>(bytes) else {
        return None;
    };
    let text = |key: &str| string_field(&fields, key);
    Some(SessionRecord {
        file_pid,
        pid: fields
            .get("pid")
            .and_then(Value::as_u64)
            .and_then(|pid| u32::try_from(pid).ok()),
        proc_start: fields
            .get("procStart")
            .and_then(Value::as_str)
            .and_then(parse_proc_start),
        session_id: text("sessionId"),
        cwd: text("cwd").map(PathBuf::from),
        entrypoint: text("entrypoint"),
        status: text("status"),
        name: text("name"),
    })
}

fn string_field(fields: &Map<String, Value>, key: &str) -> Option<String> {
    fields.get(key).and_then(Value::as_str).map(str::to_owned)
}

/// `procStart` as Unix seconds: a ctime-style UTC string,
/// `<Day> <Mon> <D> <HH>:<MM>:<SS> <YYYY>`, the day of month possibly space-padded
/// (`Nov  5`). Any other shape, a date that does not exist, or a weekday that is not
/// that date's is `None`.
fn parse_proc_start(value: &str) -> Option<u64> {
    let mut parts = value.split_ascii_whitespace();
    let (Some(weekday), Some(month), Some(day), Some(time), Some(year), None) = (
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
        parts.next(),
    ) else {
        return None;
    };
    let month = MONTHS.iter().position(|name| *name == month)? + 1;
    let month = u64::try_from(month).ok()?;
    let day = number(day, 1..=2)?;
    let year = number(year, 4..=4)?;
    let mut clock = time.split(':');
    let (Some(hours), Some(minutes), Some(seconds), None) =
        (clock.next(), clock.next(), clock.next(), clock.next())
    else {
        return None;
    };
    let (hours, minutes, seconds) = (
        number(hours, 2..=2)?,
        number(minutes, 2..=2)?,
        number(seconds, 2..=2)?,
    );
    if year < 1970
        || day == 0
        || day > days_in_month(year, month)
        || hours > 23
        || minutes > 59
        || seconds > 59
    {
        return None;
    }
    let days = days_from_civil(year, month, day);
    let weekday_of_date = usize::try_from((days + 4) % 7).ok()?;
    if WEEKDAYS.get(weekday_of_date) != Some(&weekday) {
        return None;
    }
    Some(days * 86_400 + hours * 3_600 + minutes * 60 + seconds)
}

/// ASCII digits whose count is in `width`, as a number.
fn number(value: &str, width: std::ops::RangeInclusive<usize>) -> Option<u64> {
    if !width.contains(&value.len()) || !value.bytes().all(|byte| byte.is_ascii_digit()) {
        return None;
    }
    value.parse().ok()
}

const fn is_leap_year(year: u64) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

const fn days_in_month(year: u64, month: u64) -> u64 {
    match month {
        2 if is_leap_year(year) => 29,
        2 => 28,
        4 | 6 | 9 | 11 => 30,
        _ => 31,
    }
}

/// Days from 1970-01-01 to `year-month-day` (proleptic Gregorian, `year >= 1970`), by
/// Howard Hinnant's `days_from_civil`
/// (<https://howardhinnant.github.io/date_algorithms.html#days_from_civil>), with the
/// year counted from March so the leap day falls at its end.
const fn days_from_civil(year: u64, month: u64, day: u64) -> u64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = year / 400;
    let year_of_era = year - era * 400;
    let month_from_march = if month > 2 { month - 3 } else { month + 9 };
    let day_of_year = (153 * month_from_march + 2) / 5 + day - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    // 719_468 is the number of days from 0000-03-01 to 1970-01-01.
    era * 146_097 + day_of_era - 719_468
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proc_start_parses_ctime_in_utc() {
        assert_eq!(
            parse_proc_start("Fri Nov 20 09:12:03 2026"),
            Some(1_795_165_923)
        );
    }

    #[test]
    fn proc_start_takes_a_space_padded_day() {
        assert_eq!(
            parse_proc_start("Thu Nov  5 01:02:03 2026"),
            Some(1_793_840_523)
        );
    }

    #[test]
    fn proc_start_takes_a_leap_day() {
        assert_eq!(
            parse_proc_start("Tue Feb 29 00:00:00 2028"),
            Some(1_835_395_200)
        );
    }

    #[test]
    fn proc_start_bounds_of_a_day_and_the_epoch() {
        for (value, expected) in [
            ("Thu Jan  1 00:00:00 1970", 0),
            ("Wed Dec 31 23:59:59 2025", 1_767_225_599),
            ("Thu Jan  1 00:00:00 2026", 1_767_225_600),
            ("Wed Mar  1 00:00:00 2000", 951_868_800),
        ] {
            assert_eq!(parse_proc_start(value), Some(expected), "{value}");
        }
    }

    #[test]
    fn a_weekday_that_is_not_the_dates_is_none() {
        // 2026-11-20 is a Friday.
        assert_eq!(parse_proc_start("Mon Nov 20 09:12:03 2026"), None);
    }

    #[test]
    fn proc_start_of_any_other_shape_is_none() {
        for value in [
            "",
            "yesterday",
            "2026-11-20T09:12:03Z",
            "Fri Nov 20 09:12:03",
            "Fri Nov 20 09:12:03 2026 UTC",
            "Fry Nov 20 09:12:03 2026",
            "Mon Nov 20 09:12:03 2026",
            "Sat Nov 20 09:12:03 2026",
            "Fri November 20 09:12:03 2026",
            "Fri Nov 020 09:12:03 2026",
            "Fri Nov 0 09:12:03 2026",
            "Fri Nov 31 09:12:03 2026",
            "Sun Feb 29 00:00:00 2026",
            "Mon Feb 29 00:00:00 2100",
            "Fri Nov 20 24:00:00 2026",
            "Fri Nov 20 09:60:00 2026",
            "Fri Nov 20 09:12:60 2026",
            "Fri Nov 20 9:12:03 2026",
            "Fri Nov 20 09:12 2026",
            "Fri Nov 20 09:12:03:00 2026",
            "Fri Nov 20 09:12:+3 2026",
            "Fri Nov 20 09:12:03 26",
            "Wed Dec 31 23:59:59 1969",
        ] {
            assert_eq!(parse_proc_start(value), None, "{value:?}");
        }
    }

    #[test]
    fn only_digit_named_json_files_name_a_pid() {
        for (name, expected) in [
            ("91190.json", Some(91190)),
            ("0.json", Some(0)),
            ("91190.key", None),
            ("notes.json", None),
            (".json", None),
            ("+1.json", None),
            ("91190.json.tmp", None),
            ("99999999999.json", None),
        ] {
            assert_eq!(record_file_pid(name), expected, "{name}");
        }
    }

    #[test]
    fn a_field_of_the_wrong_type_is_none_and_the_record_stays() {
        let record = parse_record(
            91190,
            br#"{"pid": "91190", "procStart": 1795165923, "cwd": 7, "status": "busy"}"#,
        );
        assert_eq!(
            record,
            Some(SessionRecord {
                file_pid: 91190,
                pid: None,
                proc_start: None,
                session_id: None,
                cwd: None,
                entrypoint: None,
                status: Some("busy".to_owned()),
                name: None,
            })
        );
    }

    #[test]
    fn a_pid_beyond_u32_is_none() {
        let record = parse_record(1, br#"{"pid": 4294967296}"#);
        assert_eq!(record.and_then(|record| record.pid), None);
    }

    #[test]
    fn a_file_that_is_not_a_json_object_is_skipped() {
        for bytes in [&b"[]"[..], b"null", b"\"text\"", b"{", b""] {
            assert_eq!(
                parse_record(1, bytes),
                None,
                "{:?}",
                String::from_utf8_lossy(bytes)
            );
        }
    }
}
