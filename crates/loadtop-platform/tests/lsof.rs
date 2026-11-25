//! What the lsof socket table does beyond the port's contract, each case against a
//! script in a temporary directory standing in for `lsof`, so none needs lsof installed.

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use loadtop_core::{ListeningPort, SocketError, SocketTable};
use loadtop_platform::LsofSocketTable;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

const UID: u32 = 501;

/// An executable `lsof` in `dir` whose body is `body`, run by `/bin/sh`.
fn script(dir: &Path, body: &str) -> PathBuf {
    let path = dir.join("lsof");
    if let Err(error) = std::fs::write(&path, format!("#!/bin/sh\n{body}\n")) {
        panic!("the fake lsof could not be written: {error}");
    }
    if let Err(error) = std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)) {
        panic!("the fake lsof could not be made executable: {error}");
    }
    path
}

fn listening_with(body: &str) -> Result<Vec<ListeningPort>, SocketError> {
    let dir = match tempfile::tempdir() {
        Ok(dir) => dir,
        Err(error) => panic!("no temporary directory: {error}"),
    };
    LsofSocketTable::new(script(dir.path(), body), UID).listening()
}

fn sorted(mut ports: Vec<ListeningPort>) -> Vec<ListeningPort> {
    ports.sort();
    ports
}

#[test]
fn ipv4_and_ipv6_on_one_port_are_one_port() {
    assert_eq!(
        listening_with(r"printf 'p91204\nf23\nn*:5173\nf24\nn[::1]:5173\n'"),
        Ok(vec![ListeningPort {
            pid: 91204,
            port: 5173
        }])
    );
    assert_eq!(
        listening_with(
            r"printf 'p91204\nf23\nn127.0.0.1:5173\np81006\nf7\nn*:8000\nf8\nn[::]:8000\n'"
        )
        .map(sorted),
        Ok(vec![
            ListeningPort {
                pid: 81006,
                port: 8000
            },
            ListeningPort {
                pid: 91204,
                port: 5173
            },
        ])
    );
}

#[test]
fn the_arguments_ask_for_this_uids_listening_tcp_sockets() {
    let dir = tempfile::tempdir().unwrap();
    let args = dir.path().join("args");
    let body = format!(
        "for arg in \"$@\"; do printf '%s\\n' \"$arg\"; done > '{}'",
        args.display()
    );
    let table = LsofSocketTable::new(script(dir.path(), &body), UID);
    assert_eq!(table.listening(), Ok(vec![]));
    assert_eq!(
        std::fs::read_to_string(&args).unwrap(),
        "-a\n-nP\n-iTCP\n-sTCP:LISTEN\n-u\n501\n-Fpn\n"
    );
}

#[test]
fn exit_zero_with_a_warning_on_stderr_still_reads_the_ports() {
    assert_eq!(
        listening_with(r"printf 'p100\nn*:3000\n'; echo 'lsof: WARNING: stat' >&2"),
        Ok(vec![ListeningPort {
            pid: 100,
            port: 3000
        }])
    );
}

#[test]
fn exit_one_with_no_output_means_no_listener() {
    assert_eq!(listening_with("exit 1"), Ok(vec![]));
}

#[test]
fn a_missing_lsof_is_unavailable() {
    let started = Instant::now();
    let table = LsofSocketTable::new(PathBuf::from("/nonexistent/lsof"), UID);
    assert_eq!(table.listening(), Err(SocketError::Unavailable));
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "returned at once, not after the timeout"
    );
}

#[test]
fn a_nonzero_exit_is_unavailable() {
    for body in [
        "exit 2",
        "echo 'lsof: unknown option' >&2; exit 1",
        r"printf 'p100\nn*:3000\n'; exit 1",
        "kill -KILL $$",
    ] {
        assert_eq!(
            listening_with(body),
            Err(SocketError::Unavailable),
            "{body}"
        );
    }
}

#[test]
fn unparsable_output_is_unavailable() {
    for body in [
        r"printf 'p100\nn*:http\n'",
        r"printf 'p100\nn*:0\n'",
        r"printf 'p100\nn*:65536\n'",
        r"printf 'n*:3000\n'",
    ] {
        assert_eq!(
            listening_with(body),
            Err(SocketError::Unavailable),
            "{body}"
        );
    }
}

#[test]
fn a_hung_lsof_is_killed_after_two_seconds() {
    let dir = tempfile::tempdir().unwrap();
    let pid_file = dir.path().join("pid");
    // `exec` makes the sleeper the child itself, so killing it orphans nothing.
    let body = format!("echo $$ > '{}'\nexec sleep 30", pid_file.display());
    let table = LsofSocketTable::new(script(dir.path(), &body), UID);

    let started = Instant::now();
    assert_eq!(table.listening(), Err(SocketError::Unavailable));
    let elapsed = started.elapsed();
    assert!(elapsed >= Duration::from_secs(2), "waited {elapsed:?}");
    assert!(elapsed < Duration::from_secs(5), "waited {elapsed:?}");

    let pid: u32 = std::fs::read_to_string(&pid_file)
        .unwrap()
        .trim()
        .parse()
        .unwrap();
    let mut system = System::new().unwrap();
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[Pid::from_u32(pid)]),
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    assert!(
        system.process(Pid::from_u32(pid)).is_none(),
        "the hung child {pid} was killed and reaped"
    );
}
