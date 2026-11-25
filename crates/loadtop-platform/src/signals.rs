//! Signals through sysinfo, each sent only after the target's start time is re-checked.

use std::sync::{Mutex, PoisonError};

use loadtop_core::{ProcessIdentity, Signal, SignalError, SignalOutcome, Signals};
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::process_table::is_live;

/// The signal sender, over a `sysinfo::System` of its own that refreshes only the PID it
/// is about to signal, so a send never waits on the sampler's lock.
#[derive(Debug)]
pub struct SysinfoSignals {
    system: Mutex<System>,
}

impl SysinfoSignals {
    /// A sender over the real OS.
    ///
    /// # Errors
    /// [`SignalError::Unsupported`] when sysinfo cannot be initialised, so no signal can
    /// be sent on this platform.
    pub fn new() -> Result<Self, SignalError> {
        let system = System::new().map_err(|_| SignalError::Unsupported)?;
        Ok(Self {
            system: Mutex::new(system),
        })
    }
}

impl Signals for SysinfoSignals {
    fn send(
        &self,
        identity: ProcessIdentity,
        signal: Signal,
    ) -> Result<SignalOutcome, SignalError> {
        let mut system = self.system.lock().unwrap_or_else(PoisonError::into_inner);
        let pid = Pid::from_u32(identity.pid);
        if !refreshed_and_live(&mut system, pid, identity) {
            return Ok(SignalOutcome::Gone);
        }
        let sent = system
            .process(pid)
            .and_then(|process| process.kill_with(sysinfo_signal(signal)));
        match sent {
            None => Err(SignalError::Unsupported),
            Some(true) => Ok(SignalOutcome::Sent),
            // kill(2)'s errno is dropped: a target that vanished is gone, and one still
            // there refused it, the only other reason for a valid same-user signal.
            Some(false) if refreshed_and_live(&mut system, pid, identity) => {
                Ok(SignalOutcome::PermissionDenied)
            }
            Some(false) => Ok(SignalOutcome::Gone),
        }
    }
}

/// Refresh the one PID, then whether it is still the process `identity` names.
fn refreshed_and_live(system: &mut System, pid: Pid, identity: ProcessIdentity) -> bool {
    system.refresh_processes_specifics(
        ProcessesToUpdate::Some(&[pid]),
        true,
        ProcessRefreshKind::nothing().without_tasks(),
    );
    system
        .process(pid)
        .is_some_and(|process| is_live(process, identity))
}

fn sysinfo_signal(signal: Signal) -> sysinfo::Signal {
    match signal {
        Signal::Stop => sysinfo::Signal::Stop,
        Signal::Cont => sysinfo::Signal::Continue,
        Signal::Term => sysinfo::Signal::Term,
        Signal::Kill => sysinfo::Signal::Kill,
    }
}
