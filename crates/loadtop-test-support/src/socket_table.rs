use std::collections::HashSet;

use loadtop_core::{ListeningPort, SocketError, SocketTable};

/// A socket table that answers with a fixed list, or fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FakeSocketTable {
    answer: Result<Vec<ListeningPort>, SocketError>,
}

impl FakeSocketTable {
    /// A table that lists `ports` on every read.
    #[must_use]
    pub const fn new(ports: Vec<ListeningPort>) -> Self {
        Self { answer: Ok(ports) }
    }

    /// A table whose every read fails with [`SocketError::Unavailable`].
    #[must_use]
    pub const fn unavailable() -> Self {
        Self {
            answer: Err(SocketError::Unavailable),
        }
    }
}

impl SocketTable for FakeSocketTable {
    fn listening(&self) -> Result<Vec<ListeningPort>, SocketError> {
        self.answer.clone()
    }
}

/// The behaviour every [`SocketTable`] must have. `make` returns a table and, when the
/// caller holds a listener open for the call, its `(pid, port)`.
///
/// # Panics
/// When the table breaks the contract; that is how the calling test fails.
pub fn socket_table_contract(
    mut make: impl FnMut() -> (Box<dyn SocketTable>, Option<ListeningPort>),
) {
    let (table, held) = make();
    let ports = match table.listening() {
        Ok(ports) => ports,
        Err(SocketError::Unavailable) => return,
    };
    let mut seen = HashSet::new();
    for listening in &ports {
        assert_ne!(
            listening.port, 0,
            "every port is in 1..=65535: {listening:?}"
        );
        assert!(
            seen.insert(*listening),
            "no (pid, port) pair repeats: {listening:?}"
        );
    }
    if let Some(held) = held {
        assert!(
            ports.contains(&held),
            "the held listener is listed: {held:?}"
        );
    }
}
