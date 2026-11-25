//! The process table through sysinfo: every same-user process, with only its marker
//! variables, and the machine's totals.

use std::collections::HashSet;
use std::ffi::OsString;
use std::path::PathBuf;
use std::sync::{Mutex, PoisonError};

use loadtop_core::{
    EnvRead, MachineTotals, Markers, ProcessIdentity, ProcessInfo, ProcessSample, ProcessTable,
    ProcessTableError, RunState, Terminal,
};
use sysinfo::{
    Pid, Process, ProcessRefreshKind, ProcessStatus, ProcessesToUpdate, System, UpdateKind,
};

/// The executable name of a Claude Code session, whose cwd is re-read every sample.
const SESSION_EXECUTABLE: &str = "claude";

/// The PID of launchd/init: an orphan's parent, whose cwd is re-read every sample.
const ORPHAN_PARENT: u32 = 1;

/// The process table, read through one `sysinfo::System` the adapter owns for its life.
///
/// CPU% is the change since the previous `sample`, so the first sample reports it as
/// unknown. Environment variables other than the five Claude Code markers are dropped
/// before any value is built.
#[derive(Debug)]
pub struct SysinfoProcessTable {
    user_id: u32,
    own_pid: Pid,
    state: Mutex<SamplerState>,
}

#[derive(Debug)]
struct SamplerState {
    system: System,
    /// The identities the previous sample listed; empty before the first.
    previous: HashSet<ProcessIdentity>,
    sampled_before: bool,
}

impl SysinfoProcessTable {
    /// A table over the real OS, filtered to the uid loadtop runs as.
    ///
    /// # Errors
    /// [`ProcessTableError::Unreadable`] when sysinfo cannot be initialised or loadtop's
    /// own process, and so its uid, cannot be read.
    pub fn new() -> Result<Self, ProcessTableError> {
        let mut system = System::new().map_err(|_| ProcessTableError::Unreadable)?;
        let own_pid = Pid::from_u32(std::process::id());
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[own_pid]),
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_user(UpdateKind::OnlyIfNotSet),
        );
        let user_id = system
            .process(own_pid)
            .and_then(Process::user_id)
            .map(|uid| **uid)
            .ok_or(ProcessTableError::Unreadable)?;
        Ok(Self {
            user_id,
            own_pid,
            state: Mutex::new(SamplerState {
                system,
                previous: HashSet::new(),
                sampled_before: false,
            }),
        })
    }

    /// The uid loadtop runs as: the one every listed process has, and the one the socket
    /// adapter asks `lsof` about. Read from loadtop's own process, since the standard
    /// library has no `getuid`.
    #[must_use]
    pub fn user_id(&self) -> u32 {
        self.user_id
    }
}

impl ProcessTable for SysinfoProcessTable {
    fn sample(&self) -> Result<ProcessSample, ProcessTableError> {
        let mut state = self.state.lock().unwrap_or_else(PoisonError::into_inner);
        let SamplerState {
            system,
            previous,
            sampled_before,
        } = &mut *state;

        system.refresh_cpu_usage();
        system.refresh_memory();
        system.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_cpu()
                .with_memory()
                .with_user(UpdateKind::OnlyIfNotSet)
                .with_cmd(UpdateKind::OnlyIfNotSet)
                .with_exe(UpdateKind::OnlyIfNotSet)
                // An environment does not change after exec; re-reading it for every
                // process each sample is wasted work.
                .with_environ(UpdateKind::OnlyIfNotSet)
                .with_cwd(UpdateKind::OnlyIfNotSet),
        );
        if system.process(self.own_pid).is_none() {
            return Err(ProcessTableError::Unreadable);
        }
        // Right after the full refresh: on macOS any refresh, even of a few PIDs, made
        // 200 ms or more after the last restarts the interval CPU% is measured over.
        let fresh_cwd: Vec<Pid> = system
            .processes()
            .values()
            .filter(|process| needs_fresh_cwd(process))
            .map(Process::pid)
            .collect();
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&fresh_cwd),
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_cwd(UpdateKind::Always),
        );

        let same_user: Vec<&Process> = system
            .processes()
            .values()
            .filter(|process| process.user_id().map(|uid| **uid) == Some(self.user_id))
            .collect();
        // On macOS sysinfo 0.40 keeps the run state a process had when its `System`
        // first saw it, so a long-lived one never sees a STOP or a CONT. A throwaway
        // `System` reads each state as it is now.
        let mut now = System::new().map_err(|_| ProcessTableError::Unreadable)?;
        let pids: Vec<Pid> = same_user.iter().map(|process| process.pid()).collect();
        now.refresh_processes_specifics(
            ProcessesToUpdate::Some(&pids),
            true,
            ProcessRefreshKind::nothing().without_tasks(),
        );
        let processes: Vec<ProcessInfo> = same_user
            .into_iter()
            .filter_map(|process| {
                // Gone, reused, or a zombie since the main refresh: left out silently.
                let current = now
                    .process(process.pid())
                    .filter(|current| current.start_time() == process.start_time())?;
                let state = match current.status() {
                    ProcessStatus::Zombie => return None,
                    ProcessStatus::Stop => RunState::Stopped,
                    _ => RunState::Running,
                };
                Some(process_info(process, state, previous))
            })
            .collect();

        let cpu_count = u32::try_from(system.cpus().len()).unwrap_or(u32::MAX);
        let machine = MachineTotals {
            cpu_count,
            cpu_percent: sampled_before
                .then(|| f64::from(system.global_cpu_usage()) * f64::from(cpu_count)),
            memory_used_bytes: system.used_memory(),
            memory_total_bytes: system.total_memory(),
        };
        *previous = processes.iter().map(|process| process.identity).collect();
        *sampled_before = true;
        Ok(ProcessSample { processes, machine })
    }

    fn cwd(&self, identity: ProcessIdentity) -> Result<Option<PathBuf>, ProcessTableError> {
        // A System of its own: a refresh of the sampler's would restart the interval
        // its next CPU% is measured over.
        let mut system = System::new().map_err(|_| ProcessTableError::Unreadable)?;
        let pid = Pid::from_u32(identity.pid);
        system.refresh_processes_specifics(
            ProcessesToUpdate::Some(&[pid]),
            true,
            ProcessRefreshKind::nothing()
                .without_tasks()
                .with_cwd(UpdateKind::Always),
        );
        match system.process(pid) {
            Some(process) if is_live(process, identity) => {
                Ok(process.cwd().map(std::path::Path::to_path_buf))
            }
            _ => Err(ProcessTableError::Gone),
        }
    }
}

/// Whether `process` is the live, non-zombie process `identity` names.
pub(crate) fn is_live(process: &Process, identity: ProcessIdentity) -> bool {
    process.start_time() == identity.start_time && process.status() != ProcessStatus::Zombie
}

/// An orphan (leftover judgement reads its cwd) or a session root.
fn needs_fresh_cwd(process: &Process) -> bool {
    process.parent().map(Pid::as_u32) == Some(ORPHAN_PARENT) || process.name() == SESSION_EXECUTABLE
}

fn process_info(
    process: &Process,
    state: RunState,
    previous: &HashSet<ProcessIdentity>,
) -> ProcessInfo {
    let identity = ProcessIdentity {
        pid: process.pid().as_u32(),
        start_time: process.start_time(),
    };
    ProcessInfo {
        identity,
        ppid: process.parent().map(Pid::as_u32),
        name: process.name().to_string_lossy().into_owned(),
        argv: process
            .cmd()
            .iter()
            .map(|arg| arg.to_string_lossy().into_owned())
            .collect(),
        exe: process.exe().map(std::path::Path::to_path_buf),
        cwd: process.cwd().map(std::path::Path::to_path_buf),
        // A process the previous sample did not list has no earlier reading to compare.
        cpu_percent: previous
            .contains(&identity)
            .then(|| f64::from(process.cpu_usage())),
        rss_bytes: process.memory(),
        state,
        // sysinfo 0.40 has no controlling-terminal accessor.
        terminal: Terminal::Unknown,
        env: env_read(process.environ()),
    }
}

/// The markers of an environment; an empty one is unreadable, since every process
/// exec'd normally has at least one variable and Apple platform binaries return none.
fn env_read(environ: &[OsString]) -> EnvRead {
    if environ.is_empty() {
        return EnvRead::Unreadable;
    }
    let mut markers = Markers::default();
    for entry in environ {
        let bytes = entry.as_encoded_bytes();
        let Some(split) = bytes.iter().position(|&byte| byte == b'=') else {
            continue;
        };
        let (key, value) = bytes.split_at(split);
        let slot = match key {
            b"CLAUDECODE" => &mut markers.claudecode,
            b"CLAUDE_CODE_SESSION_ID" => &mut markers.claude_code_session_id,
            b"CLAUDE_CODE_CHILD_SESSION" => &mut markers.claude_code_child_session,
            b"CLAUDE_PID" => &mut markers.claude_pid,
            b"CLAUDE_CODE_ENTRYPOINT" => &mut markers.claude_code_entrypoint,
            _ => continue,
        };
        *slot = Some(String::from_utf8_lossy(value.get(1..).unwrap_or_default()).into_owned());
    }
    EnvRead::Readable(markers)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn environ(entries: &[&str]) -> Vec<OsString> {
        entries.iter().map(OsString::from).collect()
    }

    #[test]
    fn an_empty_environment_is_unreadable() {
        assert_eq!(env_read(&[]), EnvRead::Unreadable);
    }

    #[test]
    fn the_five_markers_are_kept_and_everything_else_dropped() {
        let env = environ(&[
            "PATH=/usr/bin",
            "CLAUDECODE=1",
            "CLAUDE_CODE_SESSION_ID=3f9c2a10-0000-4000-8000-000000000001",
            "CLAUDE_CODE_CHILD_SESSION=1",
            "CLAUDE_PID=91190",
            "CLAUDE_CODE_ENTRYPOINT=cli",
            "ANTHROPIC_API_KEY=sk-test",
        ]);
        assert_eq!(
            env_read(&env),
            EnvRead::Readable(Markers {
                claudecode: Some("1".to_owned()),
                claude_code_session_id: Some("3f9c2a10-0000-4000-8000-000000000001".to_owned()),
                claude_code_child_session: Some("1".to_owned()),
                claude_pid: Some("91190".to_owned()),
                claude_code_entrypoint: Some("cli".to_owned()),
            })
        );
    }

    #[test]
    fn a_value_keeps_everything_after_the_first_equals_sign() {
        let env = environ(&["CLAUDE_CODE_ENTRYPOINT=a=b", "CLAUDE_PID="]);
        assert_eq!(
            env_read(&env),
            EnvRead::Readable(Markers {
                claude_pid: Some(String::new()),
                claude_code_entrypoint: Some("a=b".to_owned()),
                ..Markers::default()
            })
        );
    }

    #[test]
    fn an_environment_without_markers_is_readable_and_empty() {
        let env = environ(&["HOME=/Users/ann", "NOT_AN_ASSIGNMENT", "CLAUDECODEX=1"]);
        assert_eq!(env_read(&env), EnvRead::Readable(Markers::default()));
    }
}
