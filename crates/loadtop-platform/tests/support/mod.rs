//! The children the process and signal tests spawn, own, and reap. No test signals or
//! reads a process it did not start through this module.

use std::process::{Child, Command, Stdio};

use loadtop_core::ProcessIdentity;
use sysinfo::{Pid, ProcessRefreshKind, ProcessesToUpdate, System};

/// A child this test spawned. Dropping it kills and reaps it, on the failure path too.
pub struct SpawnedChild {
    /// The child itself, for a test that kills it without reaping or checks its exit.
    pub child: Child,
}

impl SpawnedChild {
    /// Spawn `command` with no standard streams.
    pub fn spawn(mut command: Command) -> Self {
        let spawned = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn();
        match spawned {
            Ok(child) => Self { child },
            Err(error) => panic!("the test child could not be spawned: {error}"),
        }
    }

    /// `sleep 30`.
    pub fn sleep() -> Self {
        let mut command = Command::new("sleep");
        command.arg("30");
        Self::spawn(command)
    }

    /// The child's identity, its start time read through a `System` of the test's own
    /// rather than through the adapter under test.
    pub fn identity(&self) -> ProcessIdentity {
        let mut system = system();
        let pid = Pid::from_u32(self.child.id());
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing().without_tasks(),
        );
        match system.process(pid) {
            Some(process) => ProcessIdentity {
                pid: self.child.id(),
                start_time: process.start_time(),
            },
            None => panic!("the spawned child {pid} is not in the process table"),
        }
    }
}

impl Drop for SpawnedChild {
    fn drop(&mut self) {
        // Already exited or reaped is fine: the point is that nothing outlives the test.
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A fresh `sysinfo::System` of the test's own.
pub fn system() -> System {
    match System::new() {
        Ok(system) => system,
        Err(error) => panic!("sysinfo could not be initialised: {error}"),
    }
}
