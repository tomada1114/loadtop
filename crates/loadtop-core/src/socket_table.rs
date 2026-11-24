//! Listening TCP sockets as core sees them, and the [`SocketTable`] port that reads them.

/// One listening TCP socket: the process that holds it and its port. IPv4 and IPv6
/// sockets on the same port are one entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ListeningPort {
    /// The PID of the process that holds the socket.
    pub pid: u32,
    /// The TCP port it listens on, in 1..=65535.
    pub port: u16,
}

/// Why the socket table could not answer. Not fatal: the rest of the sample stands, and
/// ports show as unknown.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SocketError {
    /// The listening sockets could not be read this time.
    #[error("the listening sockets could not be read")]
    Unavailable,
}

/// The OS's listening TCP sockets for the same user. A port: the platform adapter reads
/// the real ones, a fake returns a hand-built list.
pub trait SocketTable: Send + Sync {
    /// Every same-user listening socket, each `(pid, port)` pair once, every port in
    /// 1..=65535.
    ///
    /// # Errors
    /// [`SocketError::Unavailable`] when the sockets cannot be read.
    fn listening(&self) -> Result<Vec<ListeningPort>, SocketError>;
}
