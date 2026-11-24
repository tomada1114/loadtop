use std::path::PathBuf;

use loadtop_core::{SessionRecord, SessionRecords, SessionRecordsError};

/// Session records that answer with a fixed list, or fail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeSessionRecords {
    answer: Result<Vec<SessionRecord>, SessionRecordsError>,
}

impl FakeSessionRecords {
    /// Records that read back as `records`; an empty list is a missing directory.
    #[must_use]
    pub const fn new(records: Vec<SessionRecord>) -> Self {
        Self {
            answer: Ok(records),
        }
    }

    /// Records whose every read fails with [`SessionRecordsError::Unreadable`].
    #[must_use]
    pub const fn unreadable() -> Self {
        Self {
            answer: Err(SessionRecordsError::Unreadable),
        }
    }
}

impl SessionRecords for FakeSessionRecords {
    fn read(&self) -> Result<Vec<SessionRecord>, SessionRecordsError> {
        self.answer.clone()
    }
}

fn sorted(mut records: Vec<SessionRecord>) -> Vec<SessionRecord> {
    records.sort_by_key(|record| record.file_pid);
    records
}

/// The behaviour every [`SessionRecords`] must have. `make(None)` returns records over a
/// missing directory; `make(Some(records))` returns records over a directory holding
/// exactly those.
///
/// # Panics
/// When the records break the contract; that is how the calling test fails.
pub fn session_records_contract(
    mut make: impl FnMut(Option<&[SessionRecord]>) -> Box<dyn SessionRecords>,
) {
    assert_eq!(
        make(None).read(),
        Ok(vec![]),
        "a missing directory holds no records"
    );
    assert_eq!(
        make(Some(&[])).read(),
        Ok(vec![]),
        "an empty directory holds no records"
    );

    let written = vec![
        SessionRecord {
            file_pid: 91190,
            pid: Some(91190),
            proc_start: Some(1_763_647_923),
            session_id: Some("3f6c2a1e-0000-4000-8000-000000000001".to_owned()),
            cwd: Some(PathBuf::from("/Users/ann/code/acme-web")),
            entrypoint: Some("cli".to_owned()),
            status: Some("busy".to_owned()),
            name: Some("acme-web".to_owned()),
        },
        SessionRecord {
            file_pid: 91204,
            pid: Some(91204),
            proc_start: Some(1_763_647_990),
            session_id: Some("3f6c2a1e-0000-4000-8000-000000000002".to_owned()),
            cwd: Some(PathBuf::from("/Users/ann/code/acme-api")),
            entrypoint: Some("claude-desktop".to_owned()),
            status: Some("idle".to_owned()),
            name: Some("acme-api".to_owned()),
        },
    ];
    let read = make(Some(&written)).read().map(sorted);
    assert_eq!(
        read,
        Ok(sorted(written)),
        "the records read back as written, in any order"
    );
}
