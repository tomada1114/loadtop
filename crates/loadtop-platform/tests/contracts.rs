//! The port contracts from loadtop-test-support, run against the real adapters. Each test
//! gets its own temporary directory, and signals only a child it spawned itself; nothing
//! touches the real `~/Library`, `~/.claude`, or an existing process.

mod support;

use std::net::TcpListener;
use std::path::Path;

use loadtop_core::{ListeningPort, SessionRecord, SocketTable};
use loadtop_platform::{
    JsonFileCounterStore, LSOF_PATH, LsofSocketTable, ProjectFoldersDir, SessionRecordsDir,
    SysinfoProcessTable, SysinfoSignals, SystemClock,
};
use loadtop_test_support::{
    clock_contract, counter_store_contract, process_table_contract, project_folders_contract,
    session_records_contract, signals_contract, socket_table_contract,
};
use serde_json::{Map, Value, json};
use support::SpawnedChild;

#[test]
fn json_file_store_meets_the_counter_store_contract() {
    let mut dirs = Vec::new(); // keeps every TempDir alive until the test ends
    counter_store_contract(|| {
        let dir = tempfile::tempdir().unwrap();
        let store = JsonFileCounterStore::new(dir.path().join("nested").join("counter.json"));
        dirs.push(dir);
        Box::new(store)
    });
}

#[test]
fn system_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(SystemClock));
}

#[test]
fn sysinfo_process_table_meets_the_process_table_contract() {
    let mut children = Vec::new(); // keeps every child alive until the test ends
    process_table_contract(|| {
        let child = SpawnedChild::sleep();
        let identity = child.identity();
        children.push(child);
        (Box::new(SysinfoProcessTable::new().unwrap()), identity)
    });
}

#[test]
fn sysinfo_signals_meet_the_signals_contract() {
    let mut children = Vec::new();
    signals_contract(|| {
        let child = SpawnedChild::sleep();
        let identity = child.identity();
        children.push(child);
        (Box::new(SysinfoSignals::new().unwrap()), identity)
    });
}

#[test]
fn lsof_socket_table_meets_the_socket_table_contract() {
    let uid = SysinfoProcessTable::new().unwrap().user_id();
    let mut listeners = Vec::new(); // keeps every listener open until the test ends
    socket_table_contract(|| {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let held = ListeningPort {
            pid: std::process::id(),
            port: listener.local_addr().unwrap().port(),
        };
        listeners.push(listener);
        (
            Box::new(LsofSocketTable::new(LSOF_PATH.into(), uid)),
            Some(held),
        )
    });
}

// The contract passes on `Err(Unavailable)`, so on macOS, where lsof always ships, this
// pins that the real call answers and lists the held listener.
#[cfg(target_os = "macos")]
#[test]
fn on_macos_the_real_lsof_lists_a_held_listener() {
    let uid = SysinfoProcessTable::new().unwrap().user_id();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let held = ListeningPort {
        pid: std::process::id(),
        port: listener.local_addr().unwrap().port(),
    };
    let ports = LsofSocketTable::new(LSOF_PATH.into(), uid)
        .listening()
        .unwrap();
    assert!(ports.contains(&held), "{held:?} in {ports:?}");
}

#[test]
fn session_records_dir_meets_the_session_records_contract() {
    let mut dirs = Vec::new();
    session_records_contract(|records| {
        let dir = tempfile::tempdir().unwrap();
        let sessions = dir.path().join("sessions");
        if let Some(records) = records {
            std::fs::create_dir(&sessions).unwrap();
            for record in records {
                write_record(&sessions, record);
            }
        }
        dirs.push(dir);
        Box::new(SessionRecordsDir::new(sessions))
    });
}

#[test]
fn project_folders_dir_meets_the_project_folders_contract() {
    let mut dirs = Vec::new();
    project_folders_contract(|names| {
        let dir = tempfile::tempdir().unwrap();
        let projects = dir.path().join("projects");
        if let Some(names) = names {
            std::fs::create_dir(&projects).unwrap();
            for name in names {
                std::fs::create_dir(projects.join(name)).unwrap();
            }
        }
        dirs.push(dir);
        Box::new(ProjectFoldersDir::new(projects))
    });
}

/// `record` as Claude Code writes it: `<file_pid>.json`, camelCase keys, `procStart` as a
/// ctime-style UTC string, and an absent field left out.
fn write_record(dir: &Path, record: &SessionRecord) {
    let mut fields = Map::new();
    let mut put = |key: &str, value: Option<Value>| {
        if let Some(value) = value {
            fields.insert(key.to_owned(), value);
        }
    };
    put("pid", record.pid.map(|pid| json!(pid)));
    put(
        "procStart",
        record.proc_start.map(|start| json!(ctime(start))),
    );
    put("sessionId", record.session_id.as_ref().map(|id| json!(id)));
    put(
        "cwd",
        record.cwd.as_ref().map(|cwd| json!(cwd.to_string_lossy())),
    );
    put("entrypoint", record.entrypoint.as_ref().map(|e| json!(e)));
    put("status", record.status.as_ref().map(|status| json!(status)));
    put("name", record.name.as_ref().map(|name| json!(name)));
    let path = dir.join(format!("{}.json", record.file_pid));
    if let Err(error) = std::fs::write(&path, Value::Object(fields).to_string()) {
        panic!("{} could not be written: {error}", path.display());
    }
}

/// Unix seconds as ctime prints them in UTC (`Thu Nov 20 14:12:03 2025`), by Howard
/// Hinnant's `civil_from_days`, written independently of the adapter's parser.
fn ctime(seconds: u64) -> String {
    const WEEKDAYS: [&str; 7] = ["Thu", "Fri", "Sat", "Sun", "Mon", "Tue", "Wed"];
    const MONTHS: [&str; 12] = [
        "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec", "Jan", "Feb",
    ];
    let days = seconds / 86_400;
    let time = seconds % 86_400;
    let shifted = days + 719_468; // days since 0000-03-01
    let era = shifted / 146_097;
    let day_of_era = shifted % 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_from_march = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_from_march + 2) / 5 + 1;
    let year = era * 400 + year_of_era + u64::from(month_from_march >= 10);
    let (Ok(weekday), Ok(month)) = (usize::try_from(days % 7), usize::try_from(month_from_march))
    else {
        panic!("{seconds} does not fit a usize index");
    };
    format!(
        "{} {} {day:>2} {:02}:{:02}:{:02} {year}",
        WEEKDAYS[weekday],
        MONTHS[month],
        time / 3_600,
        time % 3_600 / 60,
        time % 60
    )
}

#[test]
fn ctime_formats_as_the_records_do() {
    assert_eq!(ctime(1_763_647_923), "Thu Nov 20 14:12:03 2025");
    assert_eq!(ctime(1_793_840_523), "Thu Nov  5 01:02:03 2026");
    assert_eq!(ctime(1_835_395_200), "Tue Feb 29 00:00:00 2028");
    assert_eq!(ctime(0), "Thu Jan  1 00:00:00 1970");
}
