//! The port contracts from loadtop-test-support, run against its fakes. The same functions
//! run against the real adapters in loadtop-platform's tests.

use loadtop_core::{
    ListeningPort, ProcessIdentity, ProcessTable, ProcessTableError, Signal, SignalError,
    SignalOutcome, Signals, SocketError, SocketTable,
};
use loadtop_test_support::{
    FakeProcessTable, FakeProjectFolders, FakeSessionRecords, FakeSignals, FakeSocketTable,
    FixedClock, InMemoryCounterStore, clock_contract, counter_store_contract,
    process_table_contract, process_with, project_folders_contract, sample_of,
    session_records_contract, signals_contract, socket_table_contract,
};

const LIVE: ProcessIdentity = ProcessIdentity {
    pid: 91190,
    start_time: 1_763_647_923,
};

#[test]
fn in_memory_store_meets_the_counter_store_contract() {
    counter_store_contract(|| Box::new(InMemoryCounterStore::default()));
}

#[test]
fn fixed_clock_meets_the_clock_contract() {
    clock_contract(|| Box::new(FixedClock::default()));
}

#[test]
fn fake_process_table_meets_the_process_table_contract() {
    process_table_contract(|| {
        let other = process_with(ProcessIdentity {
            pid: 91204,
            start_time: 1_763_647_990,
        });
        let table = FakeProcessTable::new(sample_of(vec![process_with(LIVE), other]));
        (Box::new(table), LIVE)
    });
}

#[test]
fn fake_signals_meets_the_signals_contract() {
    signals_contract(|| (Box::new(FakeSignals::new([LIVE])), LIVE));

    let signals = FakeSignals::new([ProcessIdentity {
        pid: 91204,
        start_time: 1_763_647_990,
    }]);
    let reused = ProcessIdentity {
        pid: 91204,
        start_time: 1_763_648_000,
    };
    assert_eq!(signals.send(reused, Signal::Stop), Ok(SignalOutcome::Gone));
    assert_eq!(signals.sent(), vec![], "nothing is sent to a reused PID");
}

#[test]
fn fake_socket_table_meets_the_socket_table_contract() {
    let held = ListeningPort {
        pid: 91204,
        port: 5173,
    };
    socket_table_contract(|| {
        let other = ListeningPort {
            pid: 91210,
            port: 8787,
        };
        (
            Box::new(FakeSocketTable::new(vec![other, held])),
            Some(held),
        )
    });
    socket_table_contract(|| (Box::new(FakeSocketTable::unavailable()), Some(held)));
}

#[test]
fn fake_session_records_meets_the_session_records_contract() {
    session_records_contract(|records| {
        Box::new(FakeSessionRecords::new(
            records.map(<[_]>::to_vec).unwrap_or_default(),
        ))
    });
}

#[test]
fn fake_project_folders_meets_the_project_folders_contract() {
    project_folders_contract(|names| {
        let names = names
            .unwrap_or_default()
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        Box::new(FakeProjectFolders::new(names))
    });
}

#[test]
fn the_fake_process_table_walks_its_queue_and_repeats_the_last() {
    let first = sample_of(vec![process_with(LIVE)]);
    let second = sample_of(vec![]);
    let table = FakeProcessTable::queued(vec![Ok(first.clone()), Ok(second.clone())]);
    assert_eq!(table.cwd(LIVE), Ok(first.processes[0].cwd.clone()));
    assert_eq!(table.sample(), Ok(first));
    assert_eq!(table.sample(), Ok(second.clone()));
    assert_eq!(
        table.cwd(LIVE),
        Err(ProcessTableError::Gone),
        "cwd answers from the sample returned last"
    );
    assert_eq!(table.sample(), Ok(second));
}

#[test]
fn an_unreadable_fake_process_table_fails_every_read() {
    let table = FakeProcessTable::unreadable();
    assert_eq!(table.sample(), Err(ProcessTableError::Unreadable));
    assert_eq!(table.cwd(LIVE), Err(ProcessTableError::Unreadable));
    let empty = FakeProcessTable::queued(vec![]);
    assert_eq!(empty.sample(), Err(ProcessTableError::Unreadable));
    assert_eq!(empty.cwd(LIVE), Err(ProcessTableError::Unreadable));
}

#[test]
fn the_fake_signals_records_what_it_delivered_and_forgets_a_killed_process() {
    let signals = FakeSignals::new([LIVE]);
    assert_eq!(signals.send(LIVE, Signal::Term), Ok(SignalOutcome::Sent));
    assert_eq!(signals.send(LIVE, Signal::Kill), Ok(SignalOutcome::Sent));
    assert_eq!(signals.send(LIVE, Signal::Cont), Ok(SignalOutcome::Gone));
    assert_eq!(
        signals.sent(),
        vec![(LIVE, Signal::Term), (LIVE, Signal::Kill)]
    );
}

#[test]
fn the_fake_signals_can_refuse_or_be_unsupported() {
    let denied = FakeSignals::new([LIVE]).denying([LIVE]);
    assert_eq!(
        denied.send(LIVE, Signal::Stop),
        Ok(SignalOutcome::PermissionDenied)
    );
    assert_eq!(denied.sent(), vec![]);
    assert_eq!(
        FakeSignals::unsupported().send(LIVE, Signal::Stop),
        Err(SignalError::Unsupported)
    );
}

#[test]
fn unreadable_fakes_fail_with_their_error() {
    use loadtop_core::{ProjectFolders, ProjectFoldersError, SessionRecords, SessionRecordsError};
    assert_eq!(
        FakeSocketTable::unavailable().listening(),
        Err(SocketError::Unavailable)
    );
    assert_eq!(
        FakeSessionRecords::unreadable().read(),
        Err(SessionRecordsError::Unreadable)
    );
    assert_eq!(
        FakeProjectFolders::unreadable().names(),
        Err(ProjectFoldersError::Unreadable)
    );
}
