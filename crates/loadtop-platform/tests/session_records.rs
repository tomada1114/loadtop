//! What the session-record reader does with the recorded samples under
//! `tests/fixtures/session-records/`, copied into a temporary directory first. Every value
//! in them is invented.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use loadtop_core::{SessionRecord, SessionRecords, SessionRecordsError};
use loadtop_platform::SessionRecordsDir;
use tempfile::TempDir;

/// The fixtures, copied into a fresh directory, with `61001.key` set to mode `000` there
/// (git cannot store that mode), so reading it would fail.
fn recorded_samples() -> TempDir {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/session-records");
    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => panic!("no temporary directory: {error}"),
    };
    let entries = match std::fs::read_dir(&fixtures) {
        Ok(entries) => entries,
        Err(error) => panic!("{} could not be listed: {error}", fixtures.display()),
    };
    for entry in entries {
        let source = match entry {
            Ok(entry) => entry.path(),
            Err(error) => panic!("a fixture could not be listed: {error}"),
        };
        let Some(name) = source.file_name() else {
            panic!("{} has no file name", source.display());
        };
        if let Err(error) = std::fs::copy(&source, dir.path().join(name)) {
            panic!("{} could not be copied: {error}", source.display());
        }
    }
    let key = dir.path().join("61001.key");
    if let Err(error) = std::fs::set_permissions(&key, std::fs::Permissions::from_mode(0o000)) {
        panic!("{} could not be made unreadable: {error}", key.display());
    }
    dir
}

fn read_sorted(dir: &Path) -> Result<Vec<SessionRecord>, SessionRecordsError> {
    SessionRecordsDir::new(dir.to_path_buf())
        .read()
        .map(|mut records| {
            records.sort_by_key(|record| record.file_pid);
            records
        })
}

#[test]
fn recorded_samples_parse_by_kind() {
    let dir = recorded_samples();
    let records = read_sorted(dir.path()).unwrap();
    let by_pid = |pid: u32| {
        records
            .iter()
            .find(|record| record.file_pid == pid)
            .cloned()
    };
    assert_eq!(
        by_pid(91190),
        Some(SessionRecord {
            file_pid: 91190,
            pid: Some(91190),
            proc_start: Some(1_795_165_923),
            session_id: Some("3f9c2a10-0000-4000-8000-000000000001".to_owned()),
            cwd: Some(PathBuf::from("/Users/ann/code/acme-web")),
            entrypoint: Some("claude-desktop".to_owned()),
            status: Some("busy".to_owned()),
            name: Some("Fix checkout flow".to_owned()),
        }),
        "desktop"
    );
    assert_eq!(
        by_pid(52101),
        Some(SessionRecord {
            file_pid: 52101,
            pid: Some(52101),
            proc_start: Some(1_793_840_523),
            session_id: Some("3f9c2a10-0000-4000-8000-000000000002".to_owned()),
            cwd: Some(PathBuf::from("/Users/ann/code/acme-api")),
            entrypoint: Some("cli".to_owned()),
            status: Some("idle".to_owned()),
            name: None,
        }),
        "terminal"
    );
    assert_eq!(
        by_pid(71001),
        Some(SessionRecord {
            file_pid: 71001,
            pid: Some(71001),
            proc_start: Some(1_835_395_200),
            session_id: Some("3f9c2a10-0000-4000-8000-000000000003".to_owned()),
            cwd: Some(PathBuf::from("/Users/ann/code/acme-jobs")),
            entrypoint: Some("sdk-cli".to_owned()),
            status: Some("busy".to_owned()),
            name: None,
        }),
        "headless"
    );
}

#[test]
fn an_unparsable_proc_start_leaves_the_record_without_one() {
    let dir = recorded_samples();
    let records = read_sorted(dir.path()).unwrap();
    assert_eq!(
        records.iter().find(|record| record.file_pid == 60400),
        Some(&SessionRecord {
            file_pid: 60400,
            pid: Some(60400),
            proc_start: None,
            session_id: Some("3f9c2a10-0000-4000-8000-000000000004".to_owned()),
            cwd: Some(PathBuf::from("/Users/ann/code/acme-docs")),
            entrypoint: Some("cli".to_owned()),
            status: Some("idle".to_owned()),
            name: None,
        })
    );
}

#[test]
fn only_digit_named_json_files_are_read() {
    let dir = recorded_samples();
    let pids: Vec<u32> = read_sorted(dir.path())
        .unwrap()
        .iter()
        .map(|record| record.file_pid)
        .collect();
    // 61000.json is not JSON, 61001.key is unreadable and never opened, and notes.json
    // holds a well-formed record under a name that is not a PID.
    assert_eq!(pids, vec![52101, 60400, 71001, 91190]);
}

#[test]
fn a_missing_directory_holds_no_records() {
    let dir = tempfile::tempdir().unwrap();
    assert_eq!(read_sorted(&dir.path().join("sessions")), Ok(vec![]));
}

#[test]
fn an_unlistable_directory_is_unreadable() {
    let dir = recorded_samples();
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o000)).unwrap();
    let read = read_sorted(dir.path());
    std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700)).unwrap();
    assert_eq!(read, Err(SessionRecordsError::Unreadable));
}

#[test]
fn a_file_where_the_directory_should_be_is_unreadable() {
    let dir = tempfile::tempdir().unwrap();
    let sessions = dir.path().join("sessions");
    std::fs::write(&sessions, "").unwrap();
    assert_eq!(read_sorted(&sessions), Err(SessionRecordsError::Unreadable));
}
