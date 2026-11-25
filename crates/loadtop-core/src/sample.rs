//! One sample: everything core computes a refresh from, read at one instant.

use crate::process_table::{ProcessIdentity, ProcessSample, ProcessTable, ProcessTableError};
use crate::project_folders::ProjectFolders;
use crate::session_records::{SessionRecord, SessionRecords};
use crate::socket_table::{ListeningPort, SocketError, SocketTable};
use crate::time::{Clock, UnixMillis};

/// The unit of input: the binary reads the ports and hands core this value, and every
/// row, total, and guard shown is computed from it by pure functions.
#[derive(Debug, Clone, PartialEq)]
pub struct Sample {
    /// The process table and the machine's totals.
    pub processes: ProcessSample,
    /// The listening sockets; `None` when they were not read this sample (they are read
    /// only every few samples, as they cost more).
    pub sockets: Option<Result<Vec<ListeningPort>, SocketError>>,
    /// The session records that parsed.
    pub records: Vec<SessionRecord>,
    /// The project folder names.
    pub project_folders: Vec<String>,
    /// When the sample was taken.
    pub taken_at: UnixMillis,
}

/// The ports one sample is read from.
#[derive(Clone, Copy)]
pub struct SamplePorts<'a> {
    /// The process table.
    pub processes: &'a dyn ProcessTable,
    /// The listening sockets.
    pub sockets: &'a dyn SocketTable,
    /// Claude Code's session records.
    pub records: &'a dyn SessionRecords,
    /// Claude Code's project folder names.
    pub folders: &'a dyn ProjectFolders,
    /// The clock that stamps the sample.
    pub clock: &'a dyn Clock,
}

impl Sample {
    /// Reads one sample with the sockets: the process table first, so the CPU window
    /// ends before `lsof` runs, then the sockets, the records, and the folder names.
    /// Records or folders that cannot be read count as none: they only name sessions
    /// and bound probable leftovers, and the processes alone still say what loads the
    /// machine.
    ///
    /// # Errors
    /// [`ProcessTableError`] when the process table cannot be read; nothing can be
    /// shown without it.
    pub fn read(ports: SamplePorts<'_>) -> Result<Self, ProcessTableError> {
        let processes = ports.processes.sample()?;
        let taken_at = ports.clock.now();
        Ok(Self {
            processes,
            sockets: Some(ports.sockets.listening()),
            records: ports.records.read().unwrap_or_default(),
            project_folders: ports.folders.names().unwrap_or_default(),
            taken_at,
        })
    }

    /// The identity of the process with `pid`, as this sample lists it; when it is not
    /// listed, `pid` with a start time of 0. [`crate::classify`] finds loadtop by its
    /// PID alone, so the start time never changes what is guarded.
    #[must_use]
    pub fn identity_of(&self, pid: u32) -> ProcessIdentity {
        self.processes
            .processes
            .iter()
            .map(|process| process.identity)
            .find(|identity| identity.pid == pid)
            .unwrap_or(ProcessIdentity { pid, start_time: 0 })
    }
}
