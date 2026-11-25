//! The `--json` document: one classified sample in the shape a script reads. It is its
//! own type, built from the classification and never from the screen's view, so the
//! screen can change without touching the contract; its keys are in snake case
//! (`designing-clis`' command-line reference).

use std::path::Path;
use std::time::Duration;

use serde::Serialize;

use crate::classify::{
    Classification, Group, GroupKind, LeftoverTier, Node, PortsView, SessionFacts, SessionKind,
    home_relative,
};
use crate::process_table::{ChildKind, RunState};
use crate::time::UnixMillis;

/// The document's schema version. A field removed, renamed, or retyped bumps it; a field
/// added does not.
pub const SCHEMA_VERSION: u32 = 1;

/// The whole `--json` document. Fields serialize in declaration order, so
/// `schema_version` is the first key a script reads.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct JsonReport {
    /// [`SCHEMA_VERSION`].
    pub schema_version: u32,
    /// When the sample was taken, in Unix milliseconds.
    pub sampled_at_ms: i64,
    /// The window every CPU % was measured over, in milliseconds.
    pub cpu_window_ms: u64,
    /// The whole machine's load.
    pub machine: MachineReport,
    /// The load of everything listed, summed.
    pub claude: ClaudeReport,
    /// The live sessions, heaviest first.
    pub sessions: Vec<SessionReport>,
    /// The leftover groups, heaviest first.
    pub leftovers: Vec<LeftoverReport>,
}

/// The machine's load in the document.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct MachineReport {
    /// The number of logical CPUs.
    pub cpu_cores: u32,
    /// CPU use summed over all CPUs (100 = one core); `null` when not computable.
    pub cpu_percent: Option<f64>,
    /// Memory in use, in bytes.
    pub memory_used_bytes: u64,
    /// Physical memory, in bytes.
    pub memory_total_bytes: u64,
}

/// The summed load of every listed group.
#[derive(Debug, Clone, Copy, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ClaudeReport {
    /// The summed CPU %; `null` when none could be computed.
    pub cpu_percent: Option<f64>,
    /// The summed resident memory, in bytes.
    pub rss_bytes: u64,
}

/// How a session was started, as the document spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SessionKindReport {
    /// `"terminal"`.
    Terminal,
    /// `"desktop"`.
    Desktop,
    /// `"headless"`.
    Headless,
}

/// One live session and its tree.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct SessionReport {
    /// How it was started.
    pub kind: SessionKindReport,
    /// Its project directory, home-relative; `null` when unknown.
    pub project: Option<String>,
    /// Its name; `null` when unknown.
    pub name: Option<String>,
    /// Its status; `null` when unknown.
    pub status: Option<String>,
    /// Every session ID it has had, ascending.
    pub session_ids: Vec<String>,
    /// The tree's summed CPU %; `null` when none could be computed.
    pub cpu_percent: Option<f64>,
    /// The tree's summed resident memory, in bytes.
    pub rss_bytes: u64,
    /// The session's `claude` process and its tree.
    pub root: ProcessReport,
}

/// How sure the judgement of a leftover is, as the document spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TierReport {
    /// `"confirmed"`.
    Confirmed,
    /// `"probable"`.
    Probable,
}

/// One leftover group and its tree.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct LeftoverReport {
    /// How sure the judgement is.
    pub tier: TierReport,
    /// The closed session's ID; `null` for a probable leftover.
    pub closed_session_id: Option<String>,
    /// Its project directory, home-relative; `null` when unknown.
    pub project: Option<String>,
    /// The tree's summed CPU %; `null` when none could be computed.
    pub cpu_percent: Option<f64>,
    /// The tree's summed resident memory, in bytes.
    pub rss_bytes: u64,
    /// The top of the orphan subtree and its tree.
    pub root: ProcessReport,
}

/// What a process Claude Code started is, as the document spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProcessKindReport {
    /// `"tool"`: a tool call started it.
    Tool,
    /// `"mcp"`: Claude Code started it outside a tool call.
    Mcp,
}

/// Whether a process runs, as the document spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum StateReport {
    /// `"running"`.
    Running,
    /// `"stopped"`: paused by a signal.
    Stopped,
}

/// A live session nested in another session's tree, on its process.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct NestedSessionReport {
    /// How it was started.
    pub kind: SessionKindReport,
    /// Its name; `null` when unknown.
    pub name: Option<String>,
    /// Its status; `null` when unknown.
    pub status: Option<String>,
    /// Every session ID it has had, ascending.
    pub session_ids: Vec<String>,
}

/// One process and its children.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct ProcessReport {
    /// Its PID.
    pub pid: u32,
    /// Its parent's PID; `null` when the OS gave none.
    pub ppid: Option<u32>,
    /// When it started, in Unix seconds.
    pub start_time: u64,
    /// Its process name.
    pub name: String,
    /// Its full command line, arguments joined by spaces; `null` when unreadable.
    pub command: Option<String>,
    /// `tool` or `mcp`; `null` when its environment is unreadable or unmarked.
    pub kind: Option<ProcessKindReport>,
    /// Its CPU %; `null` when not computable.
    pub cpu_percent: Option<f64>,
    /// Its resident memory, in bytes.
    pub rss_bytes: u64,
    /// Its listening TCP ports, ascending; `[]` when none, `null` when unknown.
    pub ports: Option<Vec<u16>>,
    /// Running or stopped.
    pub state: StateReport,
    /// Set when it is a live session nested in another session's tree.
    pub session: Option<NestedSessionReport>,
    /// Its children, heaviest subtree first.
    pub children: Vec<ProcessReport>,
}

impl JsonReport {
    /// The document for `classification`, sampled at `sampled_at`, its CPU % measured
    /// over `cpu_window`. Projects are shown relative to `home`. Groups keep the
    /// classification's order, split into sessions and leftovers.
    #[must_use]
    pub fn new(
        classification: &Classification,
        sampled_at: UnixMillis,
        cpu_window: Duration,
        home: &Path,
    ) -> Self {
        let mut sessions = Vec::new();
        let mut leftovers = Vec::new();
        for group in &classification.groups {
            let project = project(group, home);
            let root = process_report(&group.root);
            match &group.kind {
                GroupKind::Session(facts) => sessions.push(SessionReport {
                    kind: session_kind(facts.kind),
                    project,
                    name: facts.name.clone(),
                    status: facts.status.clone(),
                    session_ids: facts.session_ids.iter().cloned().collect(),
                    cpu_percent: group.totals.cpu_percent,
                    rss_bytes: group.totals.rss_bytes,
                    root,
                }),
                GroupKind::Leftover {
                    tier,
                    closed_session_id,
                } => leftovers.push(LeftoverReport {
                    tier: match tier {
                        LeftoverTier::Confirmed => TierReport::Confirmed,
                        LeftoverTier::Probable => TierReport::Probable,
                    },
                    closed_session_id: closed_session_id.clone(),
                    project,
                    cpu_percent: group.totals.cpu_percent,
                    rss_bytes: group.totals.rss_bytes,
                    root,
                }),
            }
        }
        let machine = classification.machine;
        Self {
            schema_version: SCHEMA_VERSION,
            sampled_at_ms: sampled_at.0,
            cpu_window_ms: u64::try_from(cpu_window.as_millis()).unwrap_or(u64::MAX),
            machine: MachineReport {
                cpu_cores: machine.cpu_count,
                cpu_percent: machine.cpu_percent,
                memory_used_bytes: machine.memory_used_bytes,
                memory_total_bytes: machine.memory_total_bytes,
            },
            claude: ClaudeReport {
                cpu_percent: classification.claude_totals.cpu_percent,
                rss_bytes: classification.claude_totals.rss_bytes,
            },
            sessions,
            leftovers,
        }
    }
}

fn project(group: &Group, home: &Path) -> Option<String> {
    group
        .project
        .as_deref()
        .map(|path| home_relative(path, home).to_string_lossy().into_owned())
}

fn session_kind(kind: SessionKind) -> SessionKindReport {
    match kind {
        SessionKind::Terminal => SessionKindReport::Terminal,
        SessionKind::Desktop => SessionKindReport::Desktop,
        SessionKind::Headless => SessionKindReport::Headless,
    }
}

fn nested_session(facts: &SessionFacts) -> NestedSessionReport {
    NestedSessionReport {
        kind: session_kind(facts.kind),
        name: facts.name.clone(),
        status: facts.status.clone(),
        session_ids: facts.session_ids.iter().cloned().collect(),
    }
}

fn process_report(node: &Node) -> ProcessReport {
    let process = &node.process;
    ProcessReport {
        pid: process.identity.pid,
        ppid: process.ppid,
        start_time: process.identity.start_time,
        name: process.name.clone(),
        command: (!process.argv.is_empty()).then(|| process.argv.join(" ")),
        kind: match node.child_kind {
            ChildKind::Tool => Some(ProcessKindReport::Tool),
            ChildKind::Mcp => Some(ProcessKindReport::Mcp),
            ChildKind::Unmarked | ChildKind::Unreadable => None,
        },
        cpu_percent: process.cpu_percent,
        rss_bytes: process.rss_bytes,
        ports: match &node.ports {
            PortsView::Read(ports) => Some(ports.clone()),
            PortsView::NotRead | PortsView::Unavailable => None,
        },
        state: match process.state {
            RunState::Running => StateReport::Running,
            RunState::Stopped => StateReport::Stopped,
        },
        session: node.nested_session.as_ref().map(nested_session),
        children: node.children.iter().map(process_report).collect(),
    }
}
