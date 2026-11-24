use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use loadtop_core::{
    EnvRead, MachineTotals, ProcessIdentity, ProcessInfo, ProcessSample, ProcessTable,
    ProcessTableError, RunState, Terminal,
};

/// A process table that answers from hand-built samples: one fixed sample, a queue the
/// fake walks through (the last one repeats), or a failure.
#[derive(Debug)]
pub struct FakeProcessTable {
    samples: Vec<Result<ProcessSample, ProcessTableError>>,
    next: Mutex<usize>,
}

impl FakeProcessTable {
    /// A table that returns `sample` on every read.
    #[must_use]
    pub fn new(sample: ProcessSample) -> Self {
        Self::queued(vec![Ok(sample)])
    }

    /// A table that returns each entry in turn, then repeats the last. An empty queue
    /// reads as [`ProcessTableError::Unreadable`].
    #[must_use]
    pub fn queued(samples: Vec<Result<ProcessSample, ProcessTableError>>) -> Self {
        Self {
            samples,
            next: Mutex::new(0),
        }
    }

    /// A table whose every read fails with [`ProcessTableError::Unreadable`].
    #[must_use]
    pub fn unreadable() -> Self {
        Self::queued(vec![Err(ProcessTableError::Unreadable)])
    }

    /// The entry `cwd` answers from: the one `sample` returned last, or the first before
    /// any read.
    fn current(&self) -> Result<&ProcessSample, ProcessTableError> {
        let next = *self.next.lock().unwrap_or_else(PoisonError::into_inner);
        let last = self.samples.len().saturating_sub(1);
        match self.samples.get(next.saturating_sub(1).min(last)) {
            Some(Ok(sample)) => Ok(sample),
            Some(Err(error)) => Err(*error),
            None => Err(ProcessTableError::Unreadable),
        }
    }
}

impl ProcessTable for FakeProcessTable {
    fn sample(&self) -> Result<ProcessSample, ProcessTableError> {
        let mut next = self.next.lock().unwrap_or_else(PoisonError::into_inner);
        let index = (*next).min(self.samples.len().saturating_sub(1));
        *next = next.saturating_add(1);
        self.samples
            .get(index)
            .cloned()
            .unwrap_or(Err(ProcessTableError::Unreadable))
    }

    fn cwd(&self, identity: ProcessIdentity) -> Result<Option<PathBuf>, ProcessTableError> {
        self.current()?
            .processes
            .iter()
            .find(|process| process.identity == identity)
            .map(|process| process.cwd.clone())
            .ok_or(ProcessTableError::Gone)
    }
}

/// A running, detached process with the given identity, a readable environment without
/// markers, and no load: a starting point a test changes field by field.
#[must_use]
pub fn process_with(identity: ProcessIdentity) -> ProcessInfo {
    ProcessInfo {
        identity,
        ppid: Some(1),
        name: "node".to_owned(),
        argv: vec!["node".to_owned()],
        exe: Some(PathBuf::from("/usr/local/bin/node")),
        cwd: Some(PathBuf::from("/Users/ann/code/acme-web")),
        cpu_percent: Some(0.0),
        rss_bytes: 0,
        state: RunState::Running,
        terminal: Terminal::Detached,
        env: EnvRead::Readable(loadtop_core::Markers::default()),
    }
}

/// A sample holding `processes`, on an idle 8-CPU machine with 16 GiB of memory.
#[must_use]
pub fn sample_of(processes: Vec<ProcessInfo>) -> ProcessSample {
    ProcessSample {
        processes,
        machine: MachineTotals {
            cpu_count: 8,
            cpu_percent: Some(0.0),
            memory_used_bytes: 0,
            memory_total_bytes: 17_179_869_184,
        },
    }
}

/// The behaviour every [`ProcessTable`] must have. `make` returns a table and the
/// identity of a live process the caller keeps alive for the call.
///
/// # Panics
/// When the table breaks the contract; that is how the calling test fails.
pub fn process_table_contract(mut make: impl FnMut() -> (Box<dyn ProcessTable>, ProcessIdentity)) {
    let (table, identity) = make();
    let sample = match table.sample() {
        Ok(sample) => sample,
        Err(error) => panic!("a live table can be sampled: {error:?}"),
    };
    let matching: Vec<&ProcessInfo> = sample
        .processes
        .iter()
        .filter(|process| process.identity.pid == identity.pid)
        .collect();
    assert_eq!(
        matching.len(),
        1,
        "the sample holds exactly one entry with the live PID"
    );
    assert_eq!(
        matching.first().map(|process| process.identity),
        Some(identity),
        "the live process's entry carries its start time"
    );
    let mut seen = HashSet::new();
    for process in &sample.processes {
        assert!(
            seen.insert(process.identity.pid),
            "no PID appears twice: {}",
            process.identity.pid
        );
    }

    if let Err(error) = table.cwd(identity) {
        panic!("the live process's cwd can be asked for: {error:?}");
    }
    let other_start = ProcessIdentity {
        start_time: identity.start_time + 1,
        ..identity
    };
    assert_eq!(
        table.cwd(other_start),
        Err(ProcessTableError::Gone),
        "the same PID with another start time is gone"
    );
}
