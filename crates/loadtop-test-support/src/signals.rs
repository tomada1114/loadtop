use std::collections::HashSet;
use std::sync::{Mutex, PoisonError};

use loadtop_core::{ProcessIdentity, Signal, SignalError, SignalOutcome, Signals};

/// A signal sender over a table of live identities. It records every signal it
/// delivers; a signal to an identity it does not know (a missing PID, or another start
/// time) is `Gone` and recorded nowhere. `Kill` removes the identity from the table.
#[derive(Debug, Default)]
pub struct FakeSignals {
    live: Mutex<HashSet<ProcessIdentity>>,
    denied: HashSet<ProcessIdentity>,
    unsupported: bool,
    sent: Mutex<Vec<(ProcessIdentity, Signal)>>,
}

impl FakeSignals {
    /// A sender that knows `live` as running processes.
    #[must_use]
    pub fn new(live: impl IntoIterator<Item = ProcessIdentity>) -> Self {
        Self {
            live: Mutex::new(live.into_iter().collect()),
            ..Self::default()
        }
    }

    /// The same sender, with the OS refusing every signal to `denied` (each must also be
    /// live to be refused rather than gone).
    #[must_use]
    pub fn denying(mut self, denied: impl IntoIterator<Item = ProcessIdentity>) -> Self {
        self.denied = denied.into_iter().collect();
        self
    }

    /// A sender on a platform that cannot send any signal: every send fails with
    /// [`SignalError::Unsupported`].
    #[must_use]
    pub fn unsupported() -> Self {
        Self {
            unsupported: true,
            ..Self::default()
        }
    }

    /// Every signal delivered so far, in order.
    #[must_use]
    pub fn sent(&self) -> Vec<(ProcessIdentity, Signal)> {
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .clone()
    }
}

impl Signals for FakeSignals {
    fn send(
        &self,
        identity: ProcessIdentity,
        signal: Signal,
    ) -> Result<SignalOutcome, SignalError> {
        if self.unsupported {
            return Err(SignalError::Unsupported);
        }
        let mut live = self.live.lock().unwrap_or_else(PoisonError::into_inner);
        if !live.contains(&identity) {
            return Ok(SignalOutcome::Gone);
        }
        if self.denied.contains(&identity) {
            return Ok(SignalOutcome::PermissionDenied);
        }
        if signal == Signal::Kill {
            live.remove(&identity);
        }
        self.sent
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .push((identity, signal));
        Ok(SignalOutcome::Sent)
    }
}

/// The behaviour every [`Signals`] must have. `make` returns a sender and the identity
/// of a process the caller started and may signal; the contract kills it.
///
/// # Panics
/// When the sender breaks the contract; that is how the calling test fails.
pub fn signals_contract(mut make: impl FnMut() -> (Box<dyn Signals>, ProcessIdentity)) {
    let (signals, identity) = make();
    assert_eq!(
        signals.send(identity, Signal::Stop),
        Ok(SignalOutcome::Sent),
        "STOP reaches a live process"
    );
    assert_eq!(
        signals.send(identity, Signal::Cont),
        Ok(SignalOutcome::Sent),
        "CONT reaches a stopped process"
    );
    let other_start = ProcessIdentity {
        start_time: identity.start_time + 1,
        ..identity
    };
    assert_eq!(
        signals.send(other_start, Signal::Stop),
        Ok(SignalOutcome::Gone),
        "the same PID with another start time is gone, and nothing is sent"
    );
    assert_eq!(
        signals.send(identity, Signal::Kill),
        Ok(SignalOutcome::Sent),
        "KILL reaches the live process"
    );
}
