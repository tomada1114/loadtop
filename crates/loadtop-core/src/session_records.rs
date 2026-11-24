//! Claude Code's session records as core sees them, the [`SessionRecords`] port that
//! reads them, and when a record may be believed.

use std::path::PathBuf;

use crate::process_table::ProcessInfo;

/// One session record Claude Code wrote for a live session. Every field but `file_pid`
/// is optional, because the record is not a documented format and any Claude Code
/// release may drop a field.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct SessionRecord {
    /// The PID the record's file is named after.
    pub file_pid: u32,
    /// `pid`: the session process's PID.
    pub pid: Option<u32>,
    /// `procStart`: the session process's start time, in Unix seconds (the adapter
    /// parses Claude Code's string; an unparsable one is `None`).
    pub proc_start: Option<u64>,
    /// `sessionId`.
    pub session_id: Option<String>,
    /// `cwd`: the session's working directory.
    pub cwd: Option<PathBuf>,
    /// `entrypoint`: how the session was started.
    pub entrypoint: Option<String>,
    /// `status`.
    pub status: Option<String>,
    /// `name`.
    pub name: Option<String>,
}

/// Why the session records could not be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SessionRecordsError {
    /// The records directory exists but could not be listed.
    #[error("the session records could not be read")]
    Unreadable,
}

/// Claude Code's session records. A port: the platform adapter reads the real
/// directory, a fake returns hand-built records.
pub trait SessionRecords: Send + Sync {
    /// Every record that parsed, in no particular order; a missing directory is
    /// `Ok(vec![])`, since no session has written one yet.
    ///
    /// # Errors
    /// [`SessionRecordsError::Unreadable`] when the directory cannot be listed.
    fn read(&self) -> Result<Vec<SessionRecord>, SessionRecordsError>;
}

/// Whether `record` describes `process`: both its `pid` and its `proc_start` equal the
/// process's PID and start time. A record whose PID was reused by a later process, or
/// that lacks either field, is not believed (requirements §3.1).
#[must_use]
pub fn record_is_valid(record: &SessionRecord, process: &ProcessInfo) -> bool {
    record.pid == Some(process.identity.pid)
        && record.proc_start == Some(process.identity.start_time)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::process_table::{EnvRead, ProcessIdentity, RunState, Terminal};

    fn process(pid: u32, start_time: u64) -> ProcessInfo {
        ProcessInfo {
            identity: ProcessIdentity { pid, start_time },
            ppid: Some(1),
            name: "claude".to_owned(),
            argv: vec!["/Users/ann/.local/bin/claude".to_owned()],
            exe: None,
            cwd: None,
            cpu_percent: None,
            rss_bytes: 0,
            state: RunState::Running,
            terminal: Terminal::Attached,
            env: EnvRead::Unreadable,
        }
    }

    fn record(pid: Option<u32>, proc_start: Option<u64>) -> SessionRecord {
        SessionRecord {
            file_pid: 91190,
            pid,
            proc_start,
            session_id: Some("3f6c2a1e-0000-4000-8000-000000000001".to_owned()),
            cwd: Some(PathBuf::from("/Users/ann/code/acme-web")),
            entrypoint: Some("cli".to_owned()),
            status: None,
            name: None,
        }
    }

    #[test]
    fn a_record_is_valid_only_when_its_proc_start_matches() {
        let record = record(Some(91190), Some(1_763_647_923));
        assert!(record_is_valid(&record, &process(91190, 1_763_647_923)));
        assert!(
            !record_is_valid(&record, &process(91190, 1_763_647_924)),
            "the PID was reused one second later"
        );
    }

    #[test]
    fn a_record_without_proc_start_is_invalid() {
        let record = record(Some(91190), None);
        assert!(!record_is_valid(&record, &process(91190, 1_763_647_923)));
    }

    #[test]
    fn a_record_for_another_pid_is_invalid() {
        let record = record(Some(91204), Some(1_763_647_923));
        assert!(!record_is_valid(&record, &process(91190, 1_763_647_923)));
    }

    #[test]
    fn a_record_without_pid_is_invalid() {
        let record = record(None, Some(1_763_647_923));
        assert!(!record_is_valid(&record, &process(91190, 1_763_647_923)));
    }
}
