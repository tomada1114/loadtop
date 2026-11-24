//! One sample: everything core computes a refresh from, read at one instant.

use crate::process_table::ProcessSample;
use crate::session_records::SessionRecord;
use crate::socket_table::{ListeningPort, SocketError};
use crate::time::UnixMillis;

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
