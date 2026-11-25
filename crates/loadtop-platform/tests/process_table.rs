//! What the sysinfo process table does beyond the port's contract: the marker filter, run
//! states, zombies, CPU% across refreshes, and the uid filter. Every process read or
//! signalled is a child the test spawned.

mod support;

use std::process::Command;
use std::time::{Duration, Instant};

use loadtop_core::{
    EnvRead, Markers, ProcessIdentity, ProcessInfo, ProcessTable, RunState, Signal, SignalOutcome,
    Signals,
};
use loadtop_platform::{SysinfoProcessTable, SysinfoSignals};
use support::SpawnedChild;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, UpdateKind};

/// Set on a re-executed copy of this test binary to make `child_role_sleeps` sleep.
const CHILD_ROLE: &str = "LOADTOP_TEST_CHILD_ROLE";
const INHERITED_MARKERS: [&str; 5] = [
    "CLAUDECODE",
    "CLAUDE_CODE_SESSION_ID",
    "CLAUDE_CODE_CHILD_SESSION",
    "CLAUDE_PID",
    "CLAUDE_CODE_ENTRYPOINT",
];
const SESSION_ID: &str = "3f9c2a10-0000-4000-8000-000000000001";
/// How long a signal or an exit may take to show in a sample.
const SETTLE: Duration = Duration::from_secs(5);

/// Not a test of loadtop: the body a re-executed copy of this binary runs. It returns at
/// once in an ordinary run and sleeps only when `CHILD_ROLE` is set.
#[test]
fn child_role_sleeps() {
    if std::env::var_os(CHILD_ROLE).is_some() {
        std::thread::sleep(Duration::from_secs(30));
    }
}

/// A child whose environment another process can read. On macOS an Apple platform
/// binary such as `/bin/sleep` (even a copy, which keeps Apple's signature) returns no
/// environment to another process, so the child is this test binary, re-executed to run
/// only `child_role_sleeps`.
fn readable_child(envs: &[(&str, &str)]) -> SpawnedChild {
    let exe = match std::env::current_exe() {
        Ok(exe) => exe,
        Err(error) => panic!("the test binary's path is unknown: {error}"),
    };
    let mut command = Command::new(exe);
    command
        .args(["child_role_sleeps", "--exact", "--nocapture"])
        .env(CHILD_ROLE, "sleep");
    // Inherited from a Claude Code session running the tests, they would blur what the
    // test set.
    for marker in INHERITED_MARKERS {
        command.env_remove(marker);
    }
    for (key, value) in envs {
        command.env(key, value);
    }
    SpawnedChild::spawn(command)
}

fn table() -> SysinfoProcessTable {
    match SysinfoProcessTable::new() {
        Ok(table) => table,
        Err(error) => panic!("the process table could not be opened: {error:?}"),
    }
}

fn signals() -> SysinfoSignals {
    match SysinfoSignals::new() {
        Ok(signals) => signals,
        Err(error) => panic!("the signal sender could not be opened: {error:?}"),
    }
}

/// The entry for `pid` in a fresh sample, if listed.
fn entry(table: &SysinfoProcessTable, pid: u32) -> Option<ProcessInfo> {
    match table.sample() {
        Ok(sample) => sample
            .processes
            .into_iter()
            .find(|process| process.identity.pid == pid),
        Err(error) => panic!("the table could not be sampled: {error:?}"),
    }
}

/// Sample until `pid`'s entry satisfies `condition` or the settle time passes; whether it
/// did.
fn eventually(
    table: &SysinfoProcessTable,
    pid: u32,
    condition: impl Fn(Option<&ProcessInfo>) -> bool,
) -> bool {
    let deadline = Instant::now() + SETTLE;
    loop {
        if condition(entry(table, pid).as_ref()) {
            return true;
        }
        if Instant::now() >= deadline {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn state_is(state: RunState) -> impl Fn(Option<&ProcessInfo>) -> bool {
    move |process| process.map(|process| process.state) == Some(state)
}

fn send(signals: &SysinfoSignals, identity: ProcessIdentity, signal: Signal) {
    assert_eq!(
        signals.send(identity, signal),
        Ok(SignalOutcome::Sent),
        "{signal:?} reaches the spawned child"
    );
}

#[test]
fn only_marker_variables_leave_the_adapter() {
    let child = readable_child(&[
        ("CLAUDE_CODE_SESSION_ID", SESSION_ID),
        ("LOADTOP_TEST_SECRET", "hunter2"),
    ]);
    let table = table();

    let sample = match table.sample() {
        Ok(sample) => sample,
        Err(error) => panic!("the table could not be sampled: {error:?}"),
    };
    let process = sample
        .processes
        .iter()
        .find(|process| process.identity.pid == child.child.id());

    assert_eq!(
        process.map(|process| &process.env),
        Some(&EnvRead::Readable(Markers {
            claude_code_session_id: Some(SESSION_ID.to_owned()),
            ..Markers::default()
        }))
    );
    assert!(
        !format!("{sample:?}").contains("hunter2"),
        "no non-marker value is anywhere in the sample"
    );
}

#[test]
fn a_stopped_child_reads_as_stopped() {
    let child = SpawnedChild::sleep();
    let identity = child.identity();
    let table = table();
    let signals = signals();
    assert!(eventually(
        &table,
        identity.pid,
        state_is(RunState::Running)
    ));

    send(&signals, identity, Signal::Stop);
    assert!(
        eventually(&table, identity.pid, state_is(RunState::Stopped)),
        "a SIGSTOPped child reads as stopped"
    );

    send(&signals, identity, Signal::Cont);
    assert!(
        eventually(&table, identity.pid, state_is(RunState::Running)),
        "a SIGCONTed child reads as running again"
    );
}

#[test]
fn a_child_first_seen_stopped_reads_as_running_after_cont() {
    let child = SpawnedChild::sleep();
    let identity = child.identity();
    let signals = signals();
    send(&signals, identity, Signal::Stop);
    let table = table();
    assert!(
        eventually(&table, identity.pid, state_is(RunState::Stopped)),
        "a child stopped before the table first saw it reads as stopped"
    );

    send(&signals, identity, Signal::Cont);
    assert!(
        eventually(&table, identity.pid, state_is(RunState::Running)),
        "the same table reads it as running once continued"
    );
}

#[test]
fn a_zombie_is_not_in_the_sample() {
    let mut child = SpawnedChild::sleep();
    let pid = child.child.id();
    let table = table();
    assert!(eventually(&table, pid, |process| process.is_some()));

    // Killed through the standard library and deliberately not reaped: a zombie.
    if let Err(error) = child.child.kill() {
        panic!("the test child could not be killed: {error}");
    }
    assert!(
        eventually(&table, pid, |process| process.is_none()),
        "an exited, unreaped child is not listed"
    );
}

#[test]
fn cpu_is_unknown_until_the_second_refresh() {
    let child = SpawnedChild::sleep();
    let pid = child.child.id();
    let table = table();

    let first = match table.sample() {
        Ok(sample) => sample,
        Err(error) => panic!("the table could not be sampled: {error:?}"),
    };
    std::thread::sleep(Duration::from_millis(300));
    let second = match table.sample() {
        Ok(sample) => sample,
        Err(error) => panic!("the table could not be sampled: {error:?}"),
    };

    assert_eq!(
        first.machine.cpu_percent, None,
        "machine CPU% on the first sample"
    );
    assert!(first.machine.cpu_count > 0);
    assert!(first.machine.memory_total_bytes > 0);
    assert!(
        first
            .processes
            .iter()
            .all(|process| process.cpu_percent.is_none()),
        "no process has a CPU% on the first sample"
    );
    assert!(
        second.machine.cpu_percent.is_some(),
        "machine CPU% on the second sample"
    );
    let child_cpu = second
        .processes
        .iter()
        .find(|process| process.identity.pid == pid)
        .and_then(|process| process.cpu_percent);
    assert!(
        child_cpu.is_some(),
        "a child listed in both samples has a CPU% on the second"
    );
}

#[test]
fn every_process_belongs_to_the_current_user() {
    let table = table();
    let sample = match table.sample() {
        Ok(sample) => sample,
        Err(error) => panic!("the table could not be sampled: {error:?}"),
    };
    let mut system = support::system();
    let uid_of = |system: &mut sysinfo::System, pid: u32| {
        let pid = Pid::from_u32(pid);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_user(UpdateKind::Always),
        );
        system
            .process(pid)
            .and_then(sysinfo::Process::user_id)
            .map(|uid| **uid)
    };
    let own_uid = uid_of(&mut system, std::process::id());
    assert_eq!(
        own_uid,
        Some(table.user_id()),
        "the adapter reports the test's uid"
    );

    for process in &sample.processes {
        let uid = uid_of(&mut system, process.identity.pid);
        // A process that exited since the sample has no uid left to compare.
        if uid.is_some() {
            assert_eq!(uid, own_uid, "pid {} has another uid", process.identity.pid);
        }
    }
    if own_uid != Some(0) {
        assert!(
            sample
                .processes
                .iter()
                .all(|process| process.identity.pid != 1),
            "root's pid 1 is not listed"
        );
    }
}
