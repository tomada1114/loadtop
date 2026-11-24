//! Signals as core sees them, and the [`Signals`] port that sends them.

use crate::process_table::ProcessIdentity;

/// The signals loadtop sends: pause, resume, stop, and the escalation after the grace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Signal {
    /// SIGSTOP: pause.
    Stop,
    /// SIGCONT: resume.
    Cont,
    /// SIGTERM: ask to stop.
    Term,
    /// SIGKILL: stop now, after the grace.
    Kill,
}

/// What happened to one signal. Each is an answer, not a failure: the caller reports it
/// per target.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SignalOutcome {
    /// The signal was delivered.
    Sent,
    /// No process has that PID, or the one that has it started at another time; nothing
    /// was sent.
    Gone,
    /// The process is there but the OS refused the signal.
    PermissionDenied,
}

/// Why a signal could not be attempted at all.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SignalError {
    /// The platform cannot send this signal.
    #[error("the platform does not support this signal")]
    Unsupported,
}

/// The signal sender. A port: the platform adapter signals real processes, a fake
/// records what it was asked to send.
pub trait Signals: Send + Sync {
    /// Send `signal` to the process `identity` names, re-checking its start time first:
    /// a missing PID or another start time is `Ok(Gone)` and nothing is sent.
    ///
    /// # Errors
    /// [`SignalError::Unsupported`] when the platform cannot send `signal`.
    fn send(&self, identity: ProcessIdentity, signal: Signal)
    -> Result<SignalOutcome, SignalError>;
}
