//! One sample classified: the live sessions with their process trees, the leftover
//! groups closed sessions left behind, which rows an action may never hit, and the
//! totals, heaviest first (requirements §3.1–§3.4).

use std::cmp::Ordering;
use std::collections::{BTreeSet, HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::process_table::{
    ChildKind, EnvRead, MachineTotals, Markers, ProcessIdentity, ProcessInfo, Terminal, child_kind,
    is_helper_invocation,
};
use crate::project_folders::is_inside_project;
use crate::sample::Sample;
use crate::session_records::{SessionRecord, record_is_valid};
use crate::socket_table::SocketError;

/// The executable name of a Claude Code session, from the CLI install or the desktop
/// app's bundled copy.
const SESSION_EXECUTABLE: &str = "claude";

/// The directory, relative to any ancestor, the desktop app keeps its bundled `claude`
/// under (`~/Library/Application Support/Claude/…`). Only the kind fallback reads it,
/// so a wrong match mislabels terminal as desktop and never changes a guard or target.
const DESKTOP_EXE_DIR: &str = "Library/Application Support/Claude";

/// The parent PID that makes a process the top of an orphan subtree: launchd's.
const ORPHAN_PPID: u32 = 1;

/// What the groups and siblings are ordered by, heaviest first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum SortKey {
    /// CPU % summed over the tree; the default.
    #[default]
    Cpu,
    /// Resident memory summed over the tree.
    Memory,
}

/// How a session was started.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SessionKind {
    /// An interactive session in a terminal (`entrypoint` `cli`).
    Terminal,
    /// A session of the desktop app (`entrypoint` `claude-desktop`).
    Desktop,
    /// A non-interactive session, such as `claude -p` (`entrypoint` `sdk-*`).
    Headless,
}

/// What is known about one live session, top-level or nested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionFacts {
    /// How it was started: from a valid record, else from argv, executable, and terminal.
    pub kind: SessionKind,
    /// Its name, from a valid record only.
    pub name: Option<String>,
    /// Its status (`busy`, `idle`) as the record spells it, from a valid record only.
    pub status: Option<String>,
    /// Every session ID it has had: the valid record's, and every
    /// `CLAUDE_CODE_SESSION_ID` among its descendants in the tree shown, nested
    /// sessions' and detached subtrees' included. The ID changes on `/clear`, so a
    /// session is a root process with a set of IDs.
    pub session_ids: BTreeSet<String>,
}

/// How sure loadtop is that a closed session left a subtree behind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LeftoverTier {
    /// A member carries a session ID no live session has.
    Confirmed,
    /// Nothing carries a marker, the top's environment is unreadable, and its cwd is
    /// inside a project: ownership is inferred.
    Probable,
}

/// What a group is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GroupKind {
    /// A live session, rooted at its `claude` process.
    Session(SessionFacts),
    /// An orphan subtree a closed session left behind.
    Leftover {
        /// How sure the judgement is.
        tier: LeftoverTier,
        /// The dead session's ID: the first one met in PID-ascending pre-order; `None`
        /// for a probable leftover.
        closed_session_id: Option<String>,
    },
}

/// Why a row can never be the target of a stop or pause. When several apply,
/// [`GuardReason::Loadtop`] wins, since it is the one no action may override.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GuardReason {
    /// loadtop itself, one of its ancestors, or one of its descendants (its own `lsof`).
    Loadtop,
    /// The root of a session group, stopped only by the separate session action.
    SessionRoot,
    /// A live session nested in another session's tree.
    NestedSession,
}

/// A process's listening TCP ports as this sample knows them.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum PortsView {
    /// The sockets were not read this sample.
    NotRead,
    /// The sockets could not be read this sample.
    Unavailable,
    /// The ports it listens on, ascending, each once; empty when it listens on none.
    Read(Vec<u16>),
}

/// The summed load of a tree or a set of groups.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Totals {
    /// The sum of the CPU % that could be computed; `None` only when none could.
    pub cpu_percent: Option<f64>,
    /// The sum of resident memory, in bytes.
    pub rss_bytes: u64,
}

impl Totals {
    fn of(process: &ProcessInfo) -> Self {
        Self {
            cpu_percent: process.cpu_percent,
            rss_bytes: process.rss_bytes,
        }
    }

    fn plus(self, other: Self) -> Self {
        let cpu_percent = match (self.cpu_percent, other.cpu_percent) {
            (Some(a), Some(b)) => Some(a + b),
            (Some(a), None) | (None, Some(a)) => Some(a),
            (None, None) => None,
        };
        Self {
            cpu_percent,
            rss_bytes: self.rss_bytes.saturating_add(other.rss_bytes),
        }
    }
}

/// One process in a group's tree, with everything a row shows about it.
#[derive(Debug, Clone, PartialEq)]
pub struct Node {
    /// The process as sampled.
    pub process: ProcessInfo,
    /// What it is by its environment markers.
    pub child_kind: ChildKind,
    /// Its listening ports.
    pub ports: PortsView,
    /// It moved to ppid 1 (or its parent is not in the table) and is kept in this
    /// session's group by its markers; set on the top of that subtree only.
    pub detached: bool,
    /// Set when this process is a live session nested in another session's tree.
    pub nested_session: Option<SessionFacts>,
    /// Why it can never be targeted; `None` when it can.
    pub guard: Option<GuardReason>,
    /// The load of this process and everything under it.
    pub subtree: Totals,
    /// Its children, heaviest subtree first, ties by PID ascending.
    pub children: Vec<Node>,
}

/// One row of the list: a live session or a leftover, with its tree.
#[derive(Debug, Clone, PartialEq)]
pub struct Group {
    /// Session or leftover.
    pub kind: GroupKind,
    /// The root process and its tree.
    pub root: Node,
    /// The whole tree's load (the root's subtree).
    pub totals: Totals,
    /// The project directory, absolute; [`home_relative`] is how a front end shows it.
    pub project: Option<PathBuf>,
}

/// Everything shown for one sample.
#[derive(Debug, Clone, PartialEq)]
pub struct Classification {
    /// Sessions and leftovers in one list, heaviest first by the sort key, ties by the
    /// root's PID ascending.
    pub groups: Vec<Group>,
    /// The sum over every listed group; zero when none is listed.
    pub claude_totals: Totals,
    /// The whole machine's load, as sampled.
    pub machine: MachineTotals,
}

/// `path` as a person reads it: `~` for `home`, `~/…` under it, unchanged elsewhere.
#[must_use]
pub fn home_relative(path: &Path, home: &Path) -> PathBuf {
    if home.as_os_str().is_empty() {
        return path.to_path_buf();
    }
    match path.strip_prefix(home) {
        Ok(rest) if rest.as_os_str().is_empty() => PathBuf::from("~"),
        Ok(rest) => Path::new("~").join(rest),
        Err(_) => path.to_path_buf(),
    }
}

/// Classifies one sample into session and leftover groups, guards, and totals.
///
/// A `claude` process that is not a helper invocation is a session; a session under
/// another session is a nested node in that one's tree. Every other process belongs to
/// its nearest session ancestor. A tree top (ppid 1, or a parent missing from the
/// table) that holds no session is kept, `detached`, in the live session its markers
/// name; failing that, a ppid-1 top is judged as a confirmed or probable leftover, and
/// anything else is not listed. `own` is loadtop's identity: its PID marks it, its
/// ancestors, and its descendants [`GuardReason::Loadtop`]. `home` bounds what counts
/// as a project. Pure: the same arguments give the same value.
#[must_use]
pub fn classify(
    sample: &Sample,
    own: ProcessIdentity,
    home: &Path,
    sort: SortKey,
) -> Classification {
    let index = Index::new(&sample.processes.processes);
    let mut sessions = Sessions::find(&index, &sample.records);
    let guarded = loadtop_chain(&index, own);

    let live_cwds: Vec<PathBuf> = sessions
        .info
        .values()
        .filter_map(|info| info.project.clone())
        .collect();

    let mut pending: Vec<(usize, Vec<usize>)> = index
        .tops
        .iter()
        .map(|&top| (top, index.preorder(top)))
        .filter(|(_, members)| {
            !members
                .iter()
                .any(|&member| sessions.info.contains_key(&member))
        })
        .collect();
    // An attached subtree can carry an ID its session had before `/clear`, which may
    // name another orphan's owner: attach until a pass attaches nothing. Each pass
    // removes at least one top or ends the loop.
    let mut detached: HashMap<usize, Vec<usize>> = HashMap::new();
    loop {
        let mut attached_any = false;
        let mut unattached = Vec::new();
        for (top, members) in pending {
            match sessions.owner_by_markers(&index, &members) {
                Some(owner) => {
                    sessions.learn_ids(&index, owner, &members);
                    detached.entry(owner).or_default().push(top);
                    attached_any = true;
                }
                None => unattached.push((top, members)),
            }
        }
        pending = unattached;
        if !attached_any {
            break;
        }
    }
    for children in detached.values_mut() {
        children.sort_unstable();
    }

    let mut leftovers: Vec<(usize, GroupKind, Option<PathBuf>)> = Vec::new();
    for (top, members) in pending {
        if index.process(top).ppid != Some(ORPHAN_PPID) {
            continue;
        }
        if let Some((kind, project)) =
            judge_leftover(&index, &members, home, &live_cwds, &sample.project_folders)
        {
            leftovers.push((top, kind, project));
        }
    }

    let builder = Builder {
        index: &index,
        sample,
        sessions: &sessions,
        detached: &detached,
        guarded: &guarded,
        sort,
    };
    let mut groups: Vec<Group> = sessions
        .top_level
        .iter()
        .filter_map(|&root| {
            let info = sessions.info.get(&root)?;
            Some(group(
                GroupKind::Session(info.facts.clone()),
                builder.node(root, true, false),
                info.project.clone(),
            ))
        })
        .chain(
            leftovers
                .into_iter()
                .map(|(top, kind, project)| group(kind, builder.node(top, true, false), project)),
        )
        .collect();
    groups.sort_by(|a, b| heavier_first(&a.root, &b.root, sort));

    let claude_totals = groups.iter().fold(
        Totals {
            cpu_percent: if groups.is_empty() { Some(0.0) } else { None },
            rss_bytes: 0,
        },
        |sum, group| sum.plus(group.totals),
    );
    Classification {
        groups,
        claude_totals,
        machine: sample.processes.machine,
    }
}

fn group(kind: GroupKind, root: Node, project: Option<PathBuf>) -> Group {
    Group {
        kind,
        totals: root.subtree,
        root,
        project,
    }
}

/// The process table as a forest: each PID once, children in PID-ascending order, and
/// the tops of the trees. Malformed ancestry (a ppid equal to the pid, a cycle) never
/// loops: every walk is bounded or tracks what it visited.
struct Index<'a> {
    processes: Vec<&'a ProcessInfo>,
    parent: Vec<Option<usize>>,
    children: Vec<Vec<usize>>,
    tops: Vec<usize>,
}

impl<'a> Index<'a> {
    fn new(all: &'a [ProcessInfo]) -> Self {
        let mut seen = HashSet::new();
        let mut processes: Vec<&ProcessInfo> = all
            .iter()
            .filter(|process| seen.insert(process.identity.pid))
            .collect();
        processes.sort_by_key(|process| process.identity.pid);
        let by_pid: HashMap<u32, usize> = processes
            .iter()
            .enumerate()
            .map(|(at, process)| (process.identity.pid, at))
            .collect();
        let parent: Vec<Option<usize>> = processes
            .iter()
            .enumerate()
            .map(|(at, process)| {
                let ppid = process.ppid.filter(|&ppid| ppid != ORPHAN_PPID)?;
                by_pid.get(&ppid).copied().filter(|&up| up != at)
            })
            .collect();
        let mut children = vec![Vec::new(); processes.len()];
        let mut tops = Vec::new();
        for (at, up) in parent.iter().enumerate() {
            match up {
                Some(up) => children[*up].push(at),
                None => tops.push(at),
            }
        }
        Self {
            processes,
            parent,
            children,
            tops,
        }
    }

    fn process(&self, at: usize) -> &'a ProcessInfo {
        self.processes[at]
    }

    /// `at` and everything under it, parents before children, siblings by PID.
    fn preorder(&self, at: usize) -> Vec<usize> {
        let mut visited = vec![false; self.processes.len()];
        let mut order = Vec::new();
        let mut stack = vec![at];
        while let Some(next) = stack.pop() {
            if std::mem::replace(&mut visited[next], true) {
                continue;
            }
            order.push(next);
            stack.extend(self.children[next].iter().rev());
        }
        order
    }

    /// The ancestors of `at`, nearest first, stopping at a tree top or a repeat.
    fn ancestors(&self, at: usize) -> Vec<usize> {
        let mut chain = Vec::new();
        let mut current = self.parent[at];
        while let Some(up) = current {
            if up == at || chain.len() >= self.processes.len() {
                break;
            }
            chain.push(up);
            current = self.parent[up];
        }
        chain
    }
}

struct SessionInfo {
    facts: SessionFacts,
    project: Option<PathBuf>,
    /// The record's ID and those of the descendants this is the nearest session of: a
    /// re-parented process carrying one hangs under this session, not an outer one.
    own_ids: BTreeSet<String>,
}

/// Every live session in the sample, by index, and which are top-level.
struct Sessions {
    info: HashMap<usize, SessionInfo>,
    top_level: Vec<usize>,
    owner_of_id: HashMap<String, usize>,
    session_at_pid: HashMap<u32, usize>,
}

impl Sessions {
    fn find(index: &Index<'_>, records: &[SessionRecord]) -> Self {
        let is_session: Vec<bool> = index.processes.iter().map(|p| is_session(p)).collect();
        let nearest_session =
            |at: usize| index.ancestors(at).into_iter().find(|&up| is_session[up]);
        let mut top_level = Vec::new();
        let mut info = HashMap::new();
        for at in (0..index.processes.len()).filter(|&at| is_session[at]) {
            // A walk that comes back to `at` means `at` sits in a ppid cycle, which no
            // real table holds: with no session ancestor it is dropped, with whatever
            // would attach to it, rather than listed as a top-level session.
            let cyclic = index
                .ancestors(at)
                .last()
                .is_some_and(|&up| index.parent[up] == Some(at));
            if nearest_session(at).is_none() && !cyclic {
                top_level.push(at);
            }
            let process = index.process(at);
            let record = records
                .iter()
                .find(|record| record_is_valid(record, process));
            let descendants = &index.preorder(at)[1..];
            let mut own_ids: BTreeSet<String> = record
                .and_then(|record| record.session_id.clone())
                .into_iter()
                .collect();
            let mut session_ids = own_ids.clone();
            for &below in descendants {
                let Some(id) = session_id(&index.process(below).env) else {
                    continue;
                };
                if nearest_session(below) == Some(at) {
                    own_ids.insert(id.to_owned());
                }
                session_ids.insert(id.to_owned());
            }
            let project = record
                .and_then(|record| record.cwd.clone())
                .or_else(|| process.cwd.clone())
                .or_else(|| first_marked_cwd(index, descendants));
            let facts = SessionFacts {
                kind: record
                    .and_then(|record| record.entrypoint.as_deref())
                    .and_then(kind_from_entrypoint)
                    .unwrap_or_else(|| fallback_kind(process)),
                name: record.and_then(|record| record.name.clone()),
                status: record.and_then(|record| record.status.clone()),
                session_ids,
            };
            info.insert(
                at,
                SessionInfo {
                    facts,
                    project,
                    own_ids,
                },
            );
        }
        let mut owner_of_id = HashMap::new();
        let mut session_at_pid = HashMap::new();
        let mut ordered: Vec<&usize> = info.keys().collect();
        ordered.sort_unstable();
        for &at in ordered {
            session_at_pid.insert(index.process(at).identity.pid, at);
            for id in &info[&at].own_ids {
                owner_of_id.entry(id.clone()).or_insert(at);
            }
        }
        Self {
            info,
            top_level,
            owner_of_id,
            session_at_pid,
        }
    }

    /// The live session the first marked member (pre-order) names, by its session ID
    /// or by `CLAUDE_PID` (only a session started no later than the member).
    fn owner_by_markers(&self, index: &Index<'_>, members: &[usize]) -> Option<usize> {
        members.iter().find_map(|&member| {
            let markers = readable(&index.process(member).env)?;
            let owner_named = markers
                .claude_code_session_id
                .as_ref()
                .and_then(|id| self.owner_of_id.get(id));
            let owner_launched = markers
                .claude_pid
                .as_deref()
                .and_then(|pid| pid.parse::<u32>().ok())
                .and_then(|pid| self.session_at_pid.get(&pid))
                // A process cannot predate the session that started it: a later
                // session at that PID is a reused PID, not the owner.
                .filter(|&&owner| {
                    index.process(owner).identity.start_time
                        <= index.process(member).identity.start_time
                });
            owner_named.or(owner_launched).copied()
        })
    }

    /// Adds the session IDs a subtree attached to `owner` carries: to `owner`'s own
    /// set, so a later orphan carrying one hangs under it, and to the public set of
    /// `owner` and every session above it, whose descendant the subtree now is.
    fn learn_ids(&mut self, index: &Index<'_>, owner: usize, members: &[usize]) {
        let ids: Vec<&str> = members
            .iter()
            .filter_map(|&member| session_id(&index.process(member).env))
            .collect();
        let holders: Vec<usize> = std::iter::once(owner)
            .chain(index.ancestors(owner))
            .filter(|at| self.info.contains_key(at))
            .collect();
        for id in ids {
            self.owner_of_id.entry(id.to_owned()).or_insert(owner);
            if let Some(info) = self.info.get_mut(&owner) {
                info.own_ids.insert(id.to_owned());
            }
            for holder in &holders {
                if let Some(info) = self.info.get_mut(holder) {
                    info.facts.session_ids.insert(id.to_owned());
                }
            }
        }
    }
}

fn is_session(process: &ProcessInfo) -> bool {
    let named = process.name == SESSION_EXECUTABLE
        || process
            .exe
            .as_deref()
            .and_then(Path::file_name)
            .is_some_and(|name| name == SESSION_EXECUTABLE);
    named && !is_helper_invocation(&process.argv)
}

fn kind_from_entrypoint(entrypoint: &str) -> Option<SessionKind> {
    match entrypoint {
        "cli" => Some(SessionKind::Terminal),
        "claude-desktop" => Some(SessionKind::Desktop),
        other if other.starts_with("sdk-") => Some(SessionKind::Headless),
        _ => None,
    }
}

fn fallback_kind(process: &ProcessInfo) -> SessionKind {
    if process
        .argv
        .iter()
        .skip(1)
        .any(|arg| arg == "-p" || arg == "--print")
    {
        return SessionKind::Headless;
    }
    if process.exe.as_deref().is_some_and(is_desktop_exe) {
        return SessionKind::Desktop;
    }
    match process.terminal {
        Terminal::Detached => SessionKind::Headless,
        Terminal::Attached | Terminal::Unknown => SessionKind::Terminal,
    }
}

fn is_desktop_exe(exe: &Path) -> bool {
    exe.parent()
        .is_some_and(|dir| dir.ancestors().any(|up| up.ends_with(DESKTOP_EXE_DIR)))
}

fn readable(env: &EnvRead) -> Option<&Markers> {
    match env {
        EnvRead::Readable(markers) => Some(markers),
        EnvRead::Unreadable => None,
    }
}

fn session_id(env: &EnvRead) -> Option<&str> {
    readable(env)?.claude_code_session_id.as_deref()
}

/// Whether the environment carries any Claude Code marker variable.
fn has_marker(env: &EnvRead) -> bool {
    readable(env).is_some_and(|markers| *markers != Markers::default())
}

fn first_marked_cwd(index: &Index<'_>, members: &[usize]) -> Option<PathBuf> {
    members.iter().find_map(|&member| {
        let process = index.process(member);
        if has_marker(&process.env) {
            process.cwd.clone()
        } else {
            None
        }
    })
}

/// The leftover tier of an orphan subtree that holds no session and names no live one,
/// with its project; `None` when it is not a leftover.
fn judge_leftover(
    index: &Index<'_>,
    members: &[usize],
    home: &Path,
    live_cwds: &[PathBuf],
    folders: &[String],
) -> Option<(GroupKind, Option<PathBuf>)> {
    let top = index.process(*members.first()?);
    let closed = members
        .iter()
        .find_map(|&member| session_id(&index.process(member).env));
    if let Some(closed) = closed {
        let project = top.cwd.clone().or_else(|| first_marked_cwd(index, members));
        let kind = GroupKind::Leftover {
            tier: LeftoverTier::Confirmed,
            closed_session_id: Some(closed.to_owned()),
        };
        return Some((kind, project));
    }
    let unmarked = !members
        .iter()
        .any(|&member| has_marker(&index.process(member).env));
    let cwd = top.cwd.as_deref()?;
    let probable = unmarked
        && child_kind(&top.env) == ChildKind::Unreadable
        && is_inside_project(cwd, home, live_cwds, folders);
    probable.then(|| {
        let kind = GroupKind::Leftover {
            tier: LeftoverTier::Probable,
            closed_session_id: None,
        };
        (kind, Some(cwd.to_path_buf()))
    })
}

/// loadtop's own process, its ancestors, and its descendants, by index. Found by PID
/// alone: while loadtop runs its PID cannot be reused, so a start time read another way
/// cannot un-guard it.
fn loadtop_chain(index: &Index<'_>, own: ProcessIdentity) -> HashSet<usize> {
    let Some(at) = (0..index.processes.len()).find(|&at| index.process(at).identity.pid == own.pid)
    else {
        return HashSet::new();
    };
    index
        .ancestors(at)
        .into_iter()
        .chain(index.preorder(at))
        .collect()
}

struct Builder<'a> {
    index: &'a Index<'a>,
    sample: &'a Sample,
    sessions: &'a Sessions,
    detached: &'a HashMap<usize, Vec<usize>>,
    guarded: &'a HashSet<usize>,
    sort: SortKey,
}

impl Builder<'_> {
    /// The node for `at` and its tree. Totals are summed in PID order before the
    /// children are sorted, so they are the same whatever the sort key.
    fn node(&self, at: usize, is_root: bool, detached: bool) -> Node {
        let process = self.index.process(at);
        let session = self.sessions.info.get(&at);
        let mut children: Vec<Node> = self.index.children[at]
            .iter()
            .map(|&child| self.node(child, false, false))
            .chain(
                self.detached
                    .get(&at)
                    .into_iter()
                    .flatten()
                    .map(|&top| self.node(top, false, true)),
            )
            .collect();
        let subtree = children
            .iter()
            .fold(Totals::of(process), |sum, child| sum.plus(child.subtree));
        children.sort_by(|a, b| heavier_first(a, b, self.sort));
        let guard = if self.guarded.contains(&at) {
            Some(GuardReason::Loadtop)
        } else {
            match (session.is_some(), is_root) {
                (true, true) => Some(GuardReason::SessionRoot),
                (true, false) => Some(GuardReason::NestedSession),
                (false, _) => None,
            }
        };
        Node {
            process: process.clone(),
            child_kind: child_kind(&process.env),
            ports: self.ports(process.identity.pid),
            detached,
            nested_session: session.filter(|_| !is_root).map(|info| info.facts.clone()),
            guard,
            subtree,
            children,
        }
    }

    fn ports(&self, pid: u32) -> PortsView {
        match &self.sample.sockets {
            None => PortsView::NotRead,
            Some(Err(SocketError::Unavailable)) => PortsView::Unavailable,
            Some(Ok(listening)) => {
                let ports: BTreeSet<u16> = listening
                    .iter()
                    .filter(|socket| socket.pid == pid)
                    .map(|socket| socket.port)
                    .collect();
                PortsView::Read(ports.into_iter().collect())
            }
        }
    }
}

/// Heavier subtree first by `sort`; a CPU total that could not be computed sorts after
/// every one that could; ties by PID ascending.
fn heavier_first(a: &Node, b: &Node, sort: SortKey) -> Ordering {
    let by_load = match sort {
        SortKey::Cpu => match (a.subtree.cpu_percent, b.subtree.cpu_percent) {
            (Some(x), Some(y)) => y.total_cmp(&x),
            (Some(_), None) => Ordering::Less,
            (None, Some(_)) => Ordering::Greater,
            (None, None) => Ordering::Equal,
        },
        SortKey::Memory => b.subtree.rss_bytes.cmp(&a.subtree.rss_bytes),
    };
    by_load.then(a.process.identity.pid.cmp(&b.process.identity.pid))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_desktop_bundled_claude_lives_under_application_support() {
        assert_eq!(DESKTOP_EXE_DIR, "Library/Application Support/Claude");
        assert!(is_desktop_exe(Path::new(
            "/Users/ann/Library/Application Support/Claude/claude-code/2.1.4/claude"
        )));
    }

    #[test]
    fn the_cli_install_is_not_the_desktop_bundle() {
        for exe in [
            "/Users/ann/.local/bin/claude",
            "/Users/ann/Library/Application Support/Claude Helper/claude",
            "/Applications/Claude.app/Contents/MacOS/Claude",
        ] {
            assert!(!is_desktop_exe(Path::new(exe)), "{exe}");
        }
    }
}
