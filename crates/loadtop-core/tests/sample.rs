//! Reading one sample through the ports: what each port's answer becomes, and which
//! failures stop the sample.

use std::path::PathBuf;

use loadtop_core::{
    ListeningPort, ProcessIdentity, ProcessTableError, Sample, SamplePorts, SessionRecord,
    SocketError, UnixMillis,
};
use loadtop_test_support::{
    FakeProcessTable, FakeProjectFolders, FakeSessionRecords, FakeSocketTable, FixedClock,
    process_with, sample_of,
};

const NOW: UnixMillis = UnixMillis(1_763_650_000_000);

fn identity(pid: u32) -> ProcessIdentity {
    ProcessIdentity {
        pid,
        start_time: 1_763_647_923,
    }
}

fn record() -> SessionRecord {
    SessionRecord {
        file_pid: 91190,
        pid: Some(91190),
        proc_start: Some(1_763_647_923),
        session_id: Some("3f9c2a10-0000-4000-8000-000000000001".to_owned()),
        cwd: Some(PathBuf::from("/Users/ann/code/acme-web")),
        entrypoint: Some("cli".to_owned()),
        status: Some("busy".to_owned()),
        name: Some("Fix checkout flow".to_owned()),
    }
}

fn read(
    processes: &FakeProcessTable,
    sockets: &FakeSocketTable,
    records: &FakeSessionRecords,
    folders: &FakeProjectFolders,
) -> Result<Sample, ProcessTableError> {
    Sample::read(SamplePorts {
        processes,
        sockets,
        records,
        folders,
        clock: &FixedClock::at(NOW),
    })
}

#[test]
fn a_sample_holds_every_ports_answer_and_the_clocks_time() {
    let table = sample_of(vec![process_with(identity(91204))]);
    let port = ListeningPort {
        pid: 91204,
        port: 5173,
    };
    let sample = read(
        &FakeProcessTable::new(table.clone()),
        &FakeSocketTable::new(vec![port]),
        &FakeSessionRecords::new(vec![record()]),
        &FakeProjectFolders::new(vec!["-Users-ann-code-acme-web".to_owned()]),
    );
    assert_eq!(
        sample,
        Ok(Sample {
            processes: table,
            sockets: Some(Ok(vec![port])),
            records: vec![record()],
            project_folders: vec!["-Users-ann-code-acme-web".to_owned()],
            taken_at: NOW,
        })
    );
}

#[test]
fn unavailable_sockets_are_kept_as_unavailable() {
    let sample = read(
        &FakeProcessTable::new(sample_of(vec![])),
        &FakeSocketTable::unavailable(),
        &FakeSessionRecords::new(vec![]),
        &FakeProjectFolders::new(vec![]),
    )
    .unwrap();
    assert_eq!(sample.sockets, Some(Err(SocketError::Unavailable)));
}

#[test]
fn unreadable_records_and_folders_count_as_none() {
    let sample = read(
        &FakeProcessTable::new(sample_of(vec![])),
        &FakeSocketTable::new(vec![]),
        &FakeSessionRecords::unreadable(),
        &FakeProjectFolders::unreadable(),
    )
    .unwrap();
    assert_eq!(sample.records, vec![]);
    assert_eq!(sample.project_folders, Vec::<String>::new());
}

#[test]
fn an_unreadable_process_table_stops_the_sample() {
    let sample = read(
        &FakeProcessTable::unreadable(),
        &FakeSocketTable::new(vec![]),
        &FakeSessionRecords::new(vec![]),
        &FakeProjectFolders::new(vec![]),
    );
    assert_eq!(sample, Err(ProcessTableError::Unreadable));
}

#[test]
fn identity_of_a_listed_pid_has_its_start_time() {
    let sample = read(
        &FakeProcessTable::new(sample_of(vec![process_with(identity(4242))])),
        &FakeSocketTable::new(vec![]),
        &FakeSessionRecords::new(vec![]),
        &FakeProjectFolders::new(vec![]),
    )
    .unwrap();
    assert_eq!(sample.identity_of(4242), identity(4242));
}

#[test]
fn identity_of_an_unlisted_pid_has_a_zero_start_time() {
    let sample = read(
        &FakeProcessTable::new(sample_of(vec![process_with(identity(4242))])),
        &FakeSocketTable::new(vec![]),
        &FakeSessionRecords::new(vec![]),
        &FakeProjectFolders::new(vec![]),
    )
    .unwrap();
    assert_eq!(
        sample.identity_of(7),
        ProcessIdentity {
            pid: 7,
            start_time: 0
        }
    );
}
