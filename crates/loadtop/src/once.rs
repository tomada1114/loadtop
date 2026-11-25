//! `--once`: one classified sample as indented plain text (ux-flows T1). The full tree,
//! never folded, with indentation instead of box drawing and no color, so it pastes
//! cleanly and a screen reader reads it line by line.

use std::path::Path;

use loadtop_core::{
    ChildKind, Classification, Group, GroupKind, LeftoverTier, Node, PortsView, RunState,
    SessionKind, home_relative,
};

use crate::wording;

/// The column a group line's CPU% field starts at, as on process lines.
const GROUP_CPU_COLUMN: usize = 44;
/// Characters a process's indentation, name, and arguments may take before they are cut.
const COMMAND_TEXT_MAX: usize = 27;
/// The width of the command field, so the kind starts two columns after its text.
const COMMAND_FIELD_WIDTH: usize = 29;
/// Spaces per tree level.
const INDENT: &str = "  ";
/// How many characters of a closed session's ID the group line shows.
const SHORT_ID_LEN: usize = 8;

const MIB: u64 = 1_048_576;
const GIB: u64 = 1_073_741_824;

/// The whole `--once` text: the header line, then every group and its processes, or the
/// empty-sample line; each line ends in `\n`.
pub fn render(classification: &Classification, home: &Path) -> String {
    let mut lines = vec![header(classification)];
    if classification.groups.is_empty() {
        lines.push(wording::NO_SESSION.to_owned());
    }
    for group in &classification.groups {
        lines.push(group_line(group, home));
        match group.kind {
            // The group line is the session's `claude` process, so its children start
            // the tree; a leftover's top is a process of its own.
            GroupKind::Session(_) => {
                for child in &group.root.children {
                    process_lines(child, 0, &mut lines);
                }
            }
            GroupKind::Leftover { .. } => process_lines(&group.root, 0, &mut lines),
        }
    }
    let mut text = String::new();
    for line in lines {
        text.push_str(line.trim_end());
        text.push('\n');
    }
    text
}

fn header(classification: &Classification) -> String {
    let machine = classification.machine;
    let claude = classification.claude_totals;
    format!(
        "CPU Claude {} cores · all {} of {}   MEM Claude {} · used {}",
        cores(claude.cpu_percent),
        cores(machine.cpu_percent),
        machine.cpu_count,
        memory(claude.rss_bytes),
        used_of_total(machine.memory_used_bytes, machine.memory_total_bytes),
    )
}

fn group_line(group: &Group, home: &Path) -> String {
    let project = group
        .project
        .as_deref()
        .map(|path| home_relative(path, home).to_string_lossy().into_owned());
    let (word, parts) = match &group.kind {
        GroupKind::Session(facts) => {
            let word = match facts.kind {
                SessionKind::Terminal => "terminal",
                SessionKind::Desktop => "desktop",
                SessionKind::Headless => "headless",
            };
            (
                word,
                vec![project, facts.name.clone(), facts.status.clone()],
            )
        }
        GroupKind::Leftover {
            tier: LeftoverTier::Confirmed,
            closed_session_id,
        } => {
            let closed = closed_session_id.as_deref().map(|id| {
                let short: String = id.chars().take(SHORT_ID_LEN).collect();
                format!("closed session {short}")
            });
            ("leftover", vec![project, closed])
        }
        GroupKind::Leftover {
            tier: LeftoverTier::Probable,
            ..
        } => ("leftover?", vec![project]),
    };
    let label: Vec<String> = parts.into_iter().flatten().collect();
    // At least one space after the kind word, which `leftover?` fills to 9 on its own.
    let start = format!("{word:<8} {}", label.join(" · "));
    let width = GROUP_CPU_COLUMN.max(start.chars().count() + 2);
    format!(
        "{start:<width$}{:>6}  {:>4}",
        cpu(group.totals.cpu_percent),
        memory(group.totals.rss_bytes),
    )
}

fn process_lines(node: &Node, depth: usize, lines: &mut Vec<String>) {
    let process = &node.process;
    let mut command = INDENT.repeat(depth);
    command.push_str(&process.name);
    for arg in process.argv.iter().skip(1) {
        command.push(' ');
        command.push_str(arg);
    }
    let kind = match node.child_kind {
        ChildKind::Tool => "tool",
        ChildKind::Mcp => "mcp",
        ChildKind::Unmarked | ChildKind::Unreadable => "-",
    };
    let mut line = format!(
        "  {:>5}  {:<COMMAND_FIELD_WIDTH$}{kind:<4}  {:>6}  {:>4}",
        process.identity.pid,
        cut(&command),
        cpu(process.cpu_percent),
        memory(process.rss_bytes),
    );
    let ports = ports(&node.ports);
    let paused = match process.state {
        RunState::Stopped => "paused",
        RunState::Running => "",
    };
    for tail in [ports.as_str(), paused] {
        if !tail.is_empty() {
            line.push_str("  ");
            line.push_str(tail);
        }
    }
    lines.push(line);
    for child in &node.children {
        process_lines(child, depth + 1, lines);
    }
}

/// `text` cut to [`COMMAND_TEXT_MAX`] characters, the last one `…` when it was longer.
fn cut(text: &str) -> String {
    if text.chars().count() <= COMMAND_TEXT_MAX {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(COMMAND_TEXT_MAX - 1).collect();
    cut.push('…');
    cut
}

fn ports(ports: &PortsView) -> String {
    match ports {
        PortsView::Read(ports) => ports
            .iter()
            .map(|port| format!(":{port}"))
            .collect::<Vec<_>>()
            .join(" "),
        PortsView::NotRead | PortsView::Unavailable => "-".to_owned(),
    }
}

/// A share of one core with one decimal: `141.2%`; `-` when unknown.
fn cpu(percent: Option<f64>) -> String {
    percent.map_or_else(|| "-".to_owned(), |percent| format!("{percent:.1}%"))
}

/// A share of one core as cores with one decimal: `3.1`; `0` when zero, `-` when unknown.
fn cores(percent: Option<f64>) -> String {
    match percent {
        Some(0.0) => "0".to_owned(),
        Some(percent) => format!("{:.1}", percent / 100.0),
        None => "-".to_owned(),
    }
}

/// Bytes in binary units: whole MiB below 1 GiB (`612M`), else GiB with one decimal and
/// `.0` dropped (`1.6G`, `16G`); `0` when zero.
fn memory(bytes: u64) -> String {
    if bytes == 0 {
        return "0".to_owned();
    }
    let mib = rounded_div(u128::from(bytes), u128::from(MIB));
    if mib < 1024 {
        return format!("{mib}M");
    }
    let tenths = rounded_div(u128::from(bytes) * 10, u128::from(GIB));
    match tenths % 10 {
        0 => format!("{}G", tenths / 10),
        frac => format!("{}.{frac}G", tenths / 10),
    }
}

/// `used/total` with the unit written once when both share it (`14.2/16G`).
fn used_of_total(used: u64, total: u64) -> String {
    let used = memory(used);
    let total = memory(total);
    let shared = |unit: char| used.ends_with(unit) && total.ends_with(unit);
    if shared('G') || shared('M') {
        format!("{}/{total}", &used[..used.len() - 1])
    } else {
        format!("{used}/{total}")
    }
}

fn rounded_div(value: u128, by: u128) -> u128 {
    (value + by / 2) / by
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::path::PathBuf;

    use loadtop_core::{
        EnvRead, MachineTotals, ProcessIdentity, ProcessInfo, SessionFacts, Totals,
    };
    use loadtop_test_support::process_with;

    use super::*;

    const HOME: &str = "/Users/ann";
    const MB: u64 = 1_048_576;

    fn process(pid: u32, name: &str, argv: &[&str], cpu: Option<f64>, rss: u64) -> ProcessInfo {
        ProcessInfo {
            name: name.to_owned(),
            argv: argv.iter().map(|arg| (*arg).to_owned()).collect(),
            cpu_percent: cpu,
            rss_bytes: rss,
            ..process_with(ProcessIdentity {
                pid,
                start_time: 1_763_647_923,
            })
        }
    }

    fn node(process: ProcessInfo, kind: ChildKind, ports: &[u16], children: Vec<Node>) -> Node {
        Node {
            subtree: Totals {
                cpu_percent: process.cpu_percent,
                rss_bytes: process.rss_bytes,
            },
            process,
            child_kind: kind,
            ports: PortsView::Read(ports.to_vec()),
            detached: false,
            nested_session: None,
            guard: None,
            children,
        }
    }

    fn machine() -> MachineTotals {
        MachineTotals {
            cpu_count: 8,
            cpu_percent: Some(520.0),
            memory_used_bytes: 15_246_955_315,
            memory_total_bytes: 17_179_869_184,
        }
    }

    /// The ux-flows T1 example: a desktop session and a confirmed leftover.
    fn t1() -> Classification {
        Classification {
            groups: vec![acme_web(), acme_api()],
            claude_totals: Totals {
                cpu_percent: Some(310.0),
                rss_bytes: 3_113_851_289,
            },
            machine: machine(),
        }
    }

    /// T1's desktop session.
    fn acme_web() -> Group {
        let esbuild = node(
            process(
                91230,
                "esbuild",
                &["esbuild", "--service"],
                Some(38.9),
                96 * MB,
            ),
            ChildKind::Tool,
            &[],
            vec![],
        );
        let vite = node(
            process(91204, "node", &["node", "vite"], Some(141.2), 612 * MB),
            ChildKind::Tool,
            &[5173],
            vec![esbuild],
        );
        let headless = node(
            process(
                81118,
                "node",
                &["node", "cli.js", "--headless"],
                Some(0.3),
                120 * MB,
            ),
            ChildKind::Mcp,
            &[],
            vec![],
        );
        let playwright = node(
            process(
                81006,
                "npm",
                &["npm", "exec", "@playwright/mcp"],
                Some(1.4),
                88 * MB,
            ),
            ChildKind::Mcp,
            &[],
            vec![headless],
        );
        let git = node(
            process(
                90011,
                "zsh",
                &["zsh", "-c", "git", "status"],
                Some(0.0),
                4 * MB,
            ),
            ChildKind::Unmarked,
            &[],
            vec![],
        );
        let claude = node(
            process(91190, "claude", &["claude"], Some(0.4), 100 * MB),
            ChildKind::Unreadable,
            &[],
            vec![vite, playwright, git],
        );
        Group {
            kind: GroupKind::Session(SessionFacts {
                kind: SessionKind::Desktop,
                name: Some("Fix checkout flow".to_owned()),
                status: Some("busy".to_owned()),
                session_ids: BTreeSet::new(),
            }),
            root: claude,
            totals: Totals {
                cpu_percent: Some(182.0),
                rss_bytes: 1_717_986_918,
            },
            project: Some(PathBuf::from("/Users/ann/code/acme-web")),
        }
    }

    /// T1's confirmed leftover.
    fn acme_api() -> Group {
        let vitest = node(
            process(
                52150,
                "node",
                &["node", "vitest", "--watch"],
                Some(96.3),
                736 * MB,
            ),
            ChildKind::Tool,
            &[],
            vec![],
        );
        let watch = node(
            process(
                52143,
                "zsh",
                &["zsh", "-c", "npm", "run", "test:watch"],
                Some(0.0),
                4 * MB,
            ),
            ChildKind::Unmarked,
            &[],
            vec![vitest],
        );
        Group {
            kind: GroupKind::Leftover {
                tier: LeftoverTier::Confirmed,
                closed_session_id: Some("8b41d0e7-0000-4000-8000-000000000002".to_owned()),
            },
            root: watch,
            totals: Totals {
                cpu_percent: Some(96.3),
                rss_bytes: 740 * MB,
            },
            project: Some(PathBuf::from("/Users/ann/code/acme-api")),
        }
    }

    fn lines(classification: &Classification) -> Vec<String> {
        render(classification, Path::new(HOME))
            .lines()
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn t1_process_lines_match_ux_flows() {
        let lines = lines(&t1());
        let processes: Vec<&str> = lines
            .iter()
            .map(String::as_str)
            .filter(|line| line.starts_with("  "))
            .collect();
        assert_eq!(
            processes,
            [
                "  91204  node vite                    tool  141.2%  612M  :5173",
                "  91230    esbuild --service          tool   38.9%   96M",
                "  81006  npm exec @playwright/mcp     mcp     1.4%   88M",
                "  81118    node cli.js --headless     mcp     0.3%  120M",
                "  90011  zsh -c git status            -       0.0%    4M",
                "  52143  zsh -c npm run test:watch    -       0.0%    4M",
                "  52150    node vitest --watch        tool   96.3%  736M",
            ]
        );
    }

    #[test]
    fn t1_header_and_group_lines_follow_the_rule() {
        let lines = lines(&t1());
        assert_eq!(
            lines[0],
            "CPU Claude 3.1 cores · all 5.2 of 8   MEM Claude 2.9G · used 14.2/16G"
        );
        assert_eq!(
            lines[1],
            "desktop  ~/code/acme-web · Fix checkout flow · busy  182.0%  1.6G"
        );
        assert_eq!(
            lines[7],
            "leftover ~/code/acme-api · closed session 8b41d0e7   96.3%  740M"
        );
        assert_eq!(lines.len(), 10);
    }

    #[test]
    fn a_short_group_label_puts_the_cpu_at_column_44() {
        let mut classification = t1();
        classification.groups.truncate(1);
        let group = &mut classification.groups[0];
        group.project = None;
        group.kind = GroupKind::Session(SessionFacts {
            kind: SessionKind::Terminal,
            name: None,
            status: Some("idle".to_owned()),
            session_ids: BTreeSet::new(),
        });
        let line = &lines(&classification)[1];
        assert_eq!(*line, format!("{:<44}182.0%  1.6G", "terminal idle"));
        assert_eq!(line.find("182.0%"), Some(44));
    }

    #[test]
    fn a_probable_leftover_shows_its_project_after_one_space() {
        let mut classification = t1();
        classification.groups.remove(0);
        let group = &mut classification.groups[0];
        group.kind = GroupKind::Leftover {
            tier: LeftoverTier::Probable,
            closed_session_id: None,
        };
        group.project = Some(PathBuf::from("/opt/acme-ops"));
        assert_eq!(
            lines(&classification)[1],
            format!("{:<44} 96.3%  740M", "leftover? /opt/acme-ops")
        );
    }

    #[test]
    fn a_headless_session_says_headless() {
        let mut classification = t1();
        classification.groups.truncate(1);
        classification.groups[0].kind = GroupKind::Session(SessionFacts {
            kind: SessionKind::Headless,
            name: None,
            status: None,
            session_ids: BTreeSet::new(),
        });
        assert!(lines(&classification)[1].starts_with("headless ~/code/acme-web  "));
    }

    #[test]
    fn an_unreadable_kind_and_cpu_print_dashes() {
        let mut classification = t1();
        classification.groups.truncate(1);
        let mut unknown = node(
            process(70001, "node", &["node"], None, 412 * MB),
            ChildKind::Unreadable,
            &[],
            vec![],
        );
        unknown.process.env = EnvRead::Unreadable;
        unknown.ports = PortsView::Unavailable;
        classification.groups[0].root.children = vec![unknown];
        classification.groups[0].totals.cpu_percent = None;
        let lines = lines(&classification);
        assert!(lines[1].ends_with("     -  1.6G"), "{}", lines[1]);
        assert_eq!(
            lines[2],
            "  70001  node                         -          -  412M  -"
        );
    }

    #[test]
    fn a_paused_process_says_paused() {
        let mut classification = t1();
        classification.groups.truncate(1);
        let mut paused = node(
            process(70002, "node", &["node", "vite"], Some(0.0), 612 * MB),
            ChildKind::Tool,
            &[5173, 5174],
            vec![],
        );
        paused.process.state = RunState::Stopped;
        let mut quiet = paused.clone();
        quiet.process.identity.pid = 70003;
        quiet.ports = PortsView::Read(vec![]);
        classification.groups[0].root.children = vec![paused, quiet];
        let lines = lines(&classification);
        assert_eq!(
            lines[2],
            "  70002  node vite                    tool    0.0%  612M  :5173 :5174  paused"
        );
        assert_eq!(
            lines[3],
            "  70003  node vite                    tool    0.0%  612M  paused"
        );
    }

    #[test]
    fn a_long_command_is_cut_to_26_characters_and_an_ellipsis() {
        let mut classification = t1();
        classification.groups.truncate(1);
        let long = node(
            process(
                70004,
                "node",
                &["node", "/Users/ann/code/acme-web/node_modules/.bin/vite"],
                Some(1.0),
                MB,
            ),
            ChildKind::Tool,
            &[],
            vec![],
        );
        let exact = node(
            process(
                70005,
                "node",
                &["node", "abcdefghijklmnopqrstuv"],
                Some(1.0),
                MB,
            ),
            ChildKind::Tool,
            &[],
            vec![],
        );
        classification.groups[0].root.children = vec![long, exact];
        let lines = lines(&classification);
        assert_eq!(
            lines[2],
            "  70004  node /Users/ann/code/acme-…  tool    1.0%    1M"
        );
        assert_eq!(
            lines[3],
            "  70005  node abcdefghijklmnopqrstuv  tool    1.0%    1M"
        );
    }

    #[test]
    fn an_empty_sample_says_no_session_is_running() {
        let empty = Classification {
            groups: vec![],
            claude_totals: Totals {
                cpu_percent: Some(0.0),
                rss_bytes: 0,
            },
            machine: MachineTotals {
                cpu_count: 8,
                cpu_percent: Some(110.0),
                memory_used_bytes: 10_522_669_875,
                memory_total_bytes: 17_179_869_184,
            },
        };
        assert_eq!(
            render(&empty, Path::new(HOME)),
            "CPU Claude 0 cores · all 1.1 of 8   MEM Claude 0 · used 9.8/16G\n\
             No Claude Code session is running.\n"
        );
    }

    #[test]
    fn unknown_machine_cpu_prints_a_dash() {
        assert_eq!(cores(None), "-");
    }

    #[test]
    fn memory_switches_to_gigabytes_at_one_gibibyte() {
        assert_eq!(memory(412 * MB), "412M");
        assert_eq!(memory(1023 * MB + MB / 2), "1G");
        assert_eq!(memory(1_395_864_371), "1.3G");
        assert_eq!(memory(16 * 1024 * MB), "16G");
        assert_eq!(memory(1), "0M");
    }

    #[test]
    fn used_of_total_writes_a_shared_unit_once() {
        assert_eq!(used_of_total(412 * MB, 512 * MB), "412/512M");
        assert_eq!(used_of_total(412 * MB, 16 * 1024 * MB), "412M/16G");
    }
}
