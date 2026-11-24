//! The process table as core sees it: one value per process, the machine's totals, the
//! [`ProcessTable`] port that reads them, and the pure facts core draws from a process's
//! environment markers and argv.

use std::path::PathBuf;

/// Who a process is: its PID plus its start time. A PID alone is reused by the OS, so
/// every target, record check, and signal names both; a PID reused within the same
/// second would be mistaken for the old process, a known and accepted limit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProcessIdentity {
    /// The process ID.
    pub pid: u32,
    /// When the process started, in Unix seconds (the OS reports it at 1 s resolution).
    pub start_time: u64,
}

/// Whether a process is running or was stopped by a signal (SIGSTOP). A zombie never
/// appears: the adapter drops it as gone, since it can no longer be signalled usefully.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RunState {
    /// Running or runnable (including sleeping on I/O).
    Running,
    /// Stopped by a signal; the source of the paused mark.
    Stopped,
}

/// Whether a process has a controlling terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Terminal {
    /// It has a controlling terminal.
    Attached,
    /// It has none.
    Detached,
    /// The OS did not say.
    Unknown,
}

/// The Claude Code marker variables of one process's environment, and nothing else.
///
/// One named field per variable, and no map, so no other variable can be carried out of
/// the adapter: a process's environment can hold tokens and keys, and loadtop shows no
/// environment value but the session ID (requirements §4 Privacy). Each field is `None`
/// when the variable is not set.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Markers {
    /// `CLAUDECODE`: `1` in a process Claude Code started.
    pub claudecode: Option<String>,
    /// `CLAUDE_CODE_SESSION_ID`: the session that started the process.
    pub claude_code_session_id: Option<String>,
    /// `CLAUDE_CODE_CHILD_SESSION`: set in a process a tool call started.
    pub claude_code_child_session: Option<String>,
    /// `CLAUDE_PID`: the PID of the Claude Code process that started it.
    pub claude_pid: Option<String>,
    /// `CLAUDE_CODE_ENTRYPOINT`: how the session was started (terminal, desktop, …).
    pub claude_code_entrypoint: Option<String>,
}

/// What reading a process's environment gave.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum EnvRead {
    /// The environment was read; these are its markers.
    Readable(Markers),
    /// The environment could not be read (another user's process, or the OS refused),
    /// which is not the same as a readable environment without markers.
    Unreadable,
}

/// One process in a sample.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessInfo {
    /// Its PID and start time.
    pub identity: ProcessIdentity,
    /// Its parent's PID; `None` when the OS gave none.
    pub ppid: Option<u32>,
    /// Its process name.
    pub name: String,
    /// Its argument vector, the program first.
    pub argv: Vec<String>,
    /// Its executable's path; `None` when unreadable.
    pub exe: Option<PathBuf>,
    /// Its working directory; `None` when unreadable.
    pub cwd: Option<PathBuf>,
    /// CPU use as a share of one core (100 = one core busy); `None` when not computable
    /// yet, as on the first refresh, which has no earlier reading to compare with.
    pub cpu_percent: Option<f64>,
    /// Resident memory, in bytes.
    pub rss_bytes: u64,
    /// Running or stopped.
    pub state: RunState,
    /// Whether it has a controlling terminal.
    pub terminal: Terminal,
    /// Its marker variables, or that they could not be read.
    pub env: EnvRead,
}

/// The whole machine's load, shown in the header.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MachineTotals {
    /// The number of logical CPUs.
    pub cpu_count: u32,
    /// CPU use summed over all CPUs (100 = one core busy); `None` when not computable yet.
    pub cpu_percent: Option<f64>,
    /// Memory in use, in bytes.
    pub memory_used_bytes: u64,
    /// Physical memory, in bytes.
    pub memory_total_bytes: u64,
}

/// One read of the process table: every same-user process, and the machine's totals.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcessSample {
    /// Every same-user process, each PID once.
    pub processes: Vec<ProcessInfo>,
    /// The machine's totals at the same instant.
    pub machine: MachineTotals,
}

/// Why the process table could not answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum ProcessTableError {
    /// The OS refused or failed to list processes; nothing can be shown.
    #[error("the process table could not be read")]
    Unreadable,
    /// The process asked about is not running, or its PID now belongs to a process with
    /// another start time.
    #[error("the process is gone")]
    Gone,
}

/// The OS's process table. A port: the platform adapter reads the real one, a fake
/// returns a hand-built sample.
pub trait ProcessTable: Send + Sync {
    /// Every same-user process and the machine's totals. A process appears at most once
    /// (each PID once), with its start time; a zombie never appears.
    ///
    /// # Errors
    /// [`ProcessTableError::Unreadable`] when the table cannot be listed.
    fn sample(&self) -> Result<ProcessSample, ProcessTableError>;

    /// Re-read one process's working directory now; `Ok(None)` when it is unreadable.
    ///
    /// # Errors
    /// [`ProcessTableError::Gone`] when no process has that PID, or the one that has it
    /// started at another time; [`ProcessTableError::Unreadable`] when the table cannot
    /// be read.
    fn cwd(&self, identity: ProcessIdentity) -> Result<Option<PathBuf>, ProcessTableError>;
}

/// What a process Claude Code started is, by its environment markers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ChildKind {
    /// A process a tool call started (`CLAUDE_CODE_CHILD_SESSION` is set).
    Tool,
    /// A process Claude Code started outside a tool call, such as an MCP server: the
    /// session marker without the child marker.
    Mcp,
    /// A readable environment with no session marker.
    Unmarked,
    /// The environment could not be read. Shown like [`ChildKind::Unmarked`], but kept
    /// apart so a leftover whose environment is unreadable is told from a readable,
    /// unmarked one (requirements §3.3).
    Unreadable,
}

/// The kind of a process, from its environment: [`ChildKind::Tool`] when
/// `CLAUDE_CODE_CHILD_SESSION` is set; [`ChildKind::Mcp`] when `CLAUDE_CODE_SESSION_ID` is
/// set or `CLAUDECODE` is `1` without it; otherwise [`ChildKind::Unmarked`], or
/// [`ChildKind::Unreadable`] when the environment could not be read.
#[must_use]
pub fn child_kind(env: &EnvRead) -> ChildKind {
    let markers = match env {
        EnvRead::Readable(markers) => markers,
        EnvRead::Unreadable => return ChildKind::Unreadable,
    };
    if markers.claude_code_child_session.is_some() {
        return ChildKind::Tool;
    }
    let session_marked =
        markers.claude_code_session_id.is_some() || markers.claudecode.as_deref() == Some("1");
    if session_marked {
        ChildKind::Mcp
    } else {
        ChildKind::Unmarked
    }
}

/// The argv entries that mark a `claude` process as a helper rather than a session.
/// Each entry was observed on a real invocation and has its own test; a helper seen
/// later is a new entry, not a heuristic. `--chrome-native-host`: the Chrome extension's
/// native messaging host (observed 2026-11-20).
pub const HELPER_ARGS: &[&str] = &["--chrome-native-host"];

/// Whether an argv holds an entry of [`HELPER_ARGS`], so the process is a helper and
/// not a session.
#[must_use]
pub fn is_helper_invocation(argv: &[String]) -> bool {
    argv.iter().any(|arg| HELPER_ARGS.contains(&arg.as_str()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn readable(markers: Markers) -> EnvRead {
        EnvRead::Readable(markers)
    }

    mod child_kind {
        use super::*;
        use crate::process_table::child_kind;

        #[test]
        fn a_tool_child_carries_the_child_session_marker() {
            let env = readable(Markers {
                claudecode: Some("1".to_owned()),
                claude_code_session_id: Some("3f6c2a1e-0000-4000-8000-000000000001".to_owned()),
                claude_code_child_session: Some("1".to_owned()),
                ..Markers::default()
            });
            assert_eq!(child_kind(&env), ChildKind::Tool);
        }

        #[test]
        fn the_child_marker_alone_makes_a_tool_child() {
            let env = readable(Markers {
                claude_code_child_session: Some("1".to_owned()),
                ..Markers::default()
            });
            assert_eq!(child_kind(&env), ChildKind::Tool);
        }

        #[test]
        fn an_mcp_server_has_the_session_marker_without_the_child_marker() {
            let env = readable(Markers {
                claude_code_session_id: Some("3f6c2a1e-0000-4000-8000-000000000001".to_owned()),
                ..Markers::default()
            });
            assert_eq!(child_kind(&env), ChildKind::Mcp);
        }

        #[test]
        fn claudecode_one_without_the_child_marker_is_an_mcp_server() {
            let env = readable(Markers {
                claudecode: Some("1".to_owned()),
                ..Markers::default()
            });
            assert_eq!(child_kind(&env), ChildKind::Mcp);
        }

        #[test]
        fn claudecode_zero_alone_is_unmarked() {
            let env = readable(Markers {
                claudecode: Some("0".to_owned()),
                ..Markers::default()
            });
            assert_eq!(child_kind(&env), ChildKind::Unmarked);
        }

        #[test]
        fn a_readable_env_without_markers_is_unmarked() {
            let env = readable(Markers {
                claude_pid: Some("91190".to_owned()),
                claude_code_entrypoint: Some("cli".to_owned()),
                ..Markers::default()
            });
            assert_eq!(child_kind(&env), ChildKind::Unmarked);
        }

        #[test]
        fn an_unreadable_env_has_no_kind() {
            assert_eq!(child_kind(&EnvRead::Unreadable), ChildKind::Unreadable);
        }
    }

    mod helper {
        use super::super::is_helper_invocation;

        fn argv(args: &[&str]) -> Vec<String> {
            args.iter().map(|arg| (*arg).to_owned()).collect()
        }

        #[test]
        fn chrome_native_host_is_a_helper_invocation() {
            let argv = argv(&[
                "/Users/ann/.local/bin/claude",
                "--chrome-native-host",
                "chrome-extension://abcdefghijklmnop/",
            ]);
            assert!(is_helper_invocation(&argv));
        }

        #[test]
        fn a_plain_claude_invocation_is_not_a_helper() {
            let argv = argv(&["/Users/ann/.local/bin/claude", "--resume"]);
            assert!(!is_helper_invocation(&argv));
        }

        #[test]
        fn an_empty_argv_is_not_a_helper() {
            assert!(!is_helper_invocation(&[]));
        }
    }
}
