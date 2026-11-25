//! `classify` over hand-built samples: sessions, nested and detached processes,
//! leftover tiers, guards, totals, and order (requirements §3.1–§3.4).

mod classify {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};

    use loadtop_core::{
        ChildKind, Classification, EnvRead, Group, GroupKind, GuardReason, LeftoverTier,
        ListeningPort, Markers, Node, PortsView, ProcessIdentity, ProcessInfo, Sample, SessionKind,
        SessionRecord, SocketError, SortKey, Terminal, Totals, UnixMillis, classify, home_relative,
    };
    use loadtop_test_support::{process_with, sample_of};

    const HOME: &str = "/Users/ann";
    const START: u64 = 1_763_647_923;
    const DEAD_ID: &str = "8b41d0e7-0000-4000-8000-000000000002";
    const LIVE_ID: &str = "3f9c2a10-0000-4000-8000-000000000001";
    const OWN: ProcessIdentity = ProcessIdentity {
        pid: 99001,
        start_time: START,
    };

    fn id(pid: u32) -> ProcessIdentity {
        ProcessIdentity {
            pid,
            start_time: START,
        }
    }

    fn argv(args: &[&str]) -> Vec<String> {
        args.iter().map(|arg| (*arg).to_owned()).collect()
    }

    /// A process under `parent` with a readable, unmarked environment.
    fn proc(pid: u32, parent: u32, cpu: f64) -> ProcessInfo {
        ProcessInfo {
            ppid: Some(parent),
            cpu_percent: Some(cpu),
            ..process_with(id(pid))
        }
    }

    /// A `claude` session process at the CLI install's path, with no terminal evidence.
    fn claude(pid: u32, parent: u32, args: &[&str]) -> ProcessInfo {
        let mut full = vec!["claude"];
        full.extend_from_slice(args);
        ProcessInfo {
            name: "claude".to_owned(),
            argv: argv(&full),
            exe: Some(PathBuf::from("/Users/ann/.local/bin/claude")),
            terminal: Terminal::Unknown,
            env: EnvRead::Unreadable,
            ..proc(pid, parent, 0.0)
        }
    }

    fn marked(mut process: ProcessInfo, markers: Markers) -> ProcessInfo {
        process.env = EnvRead::Readable(markers);
        process
    }

    fn session_marker(session_id: &str) -> Markers {
        Markers {
            claudecode: Some("1".to_owned()),
            claude_code_session_id: Some(session_id.to_owned()),
            ..Markers::default()
        }
    }

    fn tool_marker(session_id: &str) -> Markers {
        Markers {
            claude_code_child_session: Some("1".to_owned()),
            ..session_marker(session_id)
        }
    }

    fn unreadable(mut process: ProcessInfo) -> ProcessInfo {
        process.env = EnvRead::Unreadable;
        process
    }

    fn at(mut process: ProcessInfo, cwd: Option<&str>) -> ProcessInfo {
        process.cwd = cwd.map(PathBuf::from);
        process
    }

    fn record(pid: u32, entrypoint: &str, session_id: &str, cwd: &str) -> SessionRecord {
        SessionRecord {
            file_pid: pid,
            pid: Some(pid),
            proc_start: Some(START),
            session_id: Some(session_id.to_owned()),
            cwd: Some(PathBuf::from(cwd)),
            entrypoint: Some(entrypoint.to_owned()),
            status: Some("busy".to_owned()),
            name: Some("Fix checkout flow".to_owned()),
        }
    }

    fn sample(processes: Vec<ProcessInfo>) -> Sample {
        Sample {
            processes: sample_of(processes),
            sockets: None,
            records: Vec::new(),
            project_folders: Vec::new(),
            taken_at: UnixMillis(1_763_650_000_000),
        }
    }

    fn with_records(mut sample: Sample, records: Vec<SessionRecord>) -> Sample {
        sample.records = records;
        sample
    }

    fn run(sample: &Sample) -> Classification {
        classify(sample, OWN, Path::new(HOME), SortKey::Cpu)
    }

    fn run_by(sample: &Sample, sort: SortKey) -> Classification {
        classify(sample, OWN, Path::new(HOME), sort)
    }

    fn only_group(classification: &Classification) -> &Group {
        assert_eq!(classification.groups.len(), 1, "{classification:#?}");
        &classification.groups[0]
    }

    fn pids(nodes: &[Node]) -> Vec<u32> {
        nodes.iter().map(|node| node.process.identity.pid).collect()
    }

    fn group_pids(classification: &Classification) -> Vec<u32> {
        classification
            .groups
            .iter()
            .map(|group| group.root.process.identity.pid)
            .collect()
    }

    fn find(node: &Node, pid: u32) -> Option<&Node> {
        if node.process.identity.pid == pid {
            return Some(node);
        }
        node.children.iter().find_map(|child| find(child, pid))
    }

    fn node_in(classification: &Classification, pid: u32) -> &Node {
        classification
            .groups
            .iter()
            .find_map(|group| find(&group.root, pid))
            .unwrap_or_else(|| panic!("{pid} is listed: {classification:#?}"))
    }

    fn session(group: &Group) -> &loadtop_core::SessionFacts {
        match &group.kind {
            GroupKind::Session(facts) => facts,
            GroupKind::Leftover { .. } => panic!("a session group: {group:#?}"),
        }
    }

    fn leftover(group: &Group) -> (LeftoverTier, Option<&str>) {
        match &group.kind {
            GroupKind::Leftover {
                tier,
                closed_session_id,
            } => (*tier, closed_session_id.as_deref()),
            GroupKind::Session(_) => panic!("a leftover group: {group:#?}"),
        }
    }

    fn totals(cpu: Option<f64>, rss_bytes: u64) -> Totals {
        Totals {
            cpu_percent: cpu,
            rss_bytes,
        }
    }

    // REQ-001

    #[test]
    fn a_claude_process_is_a_session() {
        let classification = run(&sample(vec![claude(91190, 600, &["--resume"])]));
        let group = only_group(&classification);
        assert_eq!(session(group).kind, SessionKind::Terminal);
        assert_eq!(group.root.process.identity.pid, 91190);
    }

    #[test]
    fn a_process_whose_executable_is_claude_is_a_session() {
        let process = ProcessInfo {
            name: "2.1.4".to_owned(),
            ..claude(91190, 600, &[])
        };
        let classification = run(&sample(vec![process]));
        assert_eq!(group_pids(&classification), vec![91190]);
    }

    #[test]
    fn a_chrome_native_host_is_not_a_session() {
        let host = claude(
            91300,
            600,
            &[
                "--chrome-native-host",
                "chrome-extension://abcdefghijklmnop/",
            ],
        );
        let classification = run(&sample(vec![host]));
        assert!(classification.groups.is_empty(), "{classification:#?}");
    }

    // REQ-002, and the first concrete example

    #[test]
    fn a_valid_record_names_the_desktop_session() {
        let root = ProcessInfo {
            cpu_percent: Some(0.5),
            ..claude(91190, 91100, &[])
        };
        let vite = ProcessInfo {
            argv: argv(&["node", "vite"]),
            ..marked(
                proc(91204, 91190, 141.25),
                Markers {
                    claude_code_child_session: Some("1".to_owned()),
                    ..Markers::default()
                },
            )
        };
        let mcp = ProcessInfo {
            argv: argv(&["npm", "exec", "@playwright/mcp"]),
            ..marked(proc(81006, 91190, 1.5), session_marker(LIVE_ID))
        };
        let sample = with_records(
            sample(vec![root, mcp, vite]),
            vec![record(
                91190,
                "claude-desktop",
                LIVE_ID,
                "/Users/ann/code/acme-web",
            )],
        );
        let classification = run(&sample);
        let group = only_group(&classification);
        let facts = session(group);
        assert_eq!(facts.kind, SessionKind::Desktop);
        assert_eq!(facts.name.as_deref(), Some("Fix checkout flow"));
        assert_eq!(facts.status.as_deref(), Some("busy"));
        assert_eq!(
            group.project.as_deref(),
            Some(Path::new("/Users/ann/code/acme-web"))
        );
        assert_eq!(group.totals, totals(Some(143.25), 0));
        assert_eq!(group.root.guard, Some(GuardReason::SessionRoot));
        assert_eq!(pids(&group.root.children), vec![91204, 81006]);
        assert_eq!(group.root.children[0].child_kind, ChildKind::Tool);
        assert_eq!(group.root.children[1].child_kind, ChildKind::Mcp);
    }

    #[test]
    fn record_entrypoints_name_the_kind() {
        let cases = [
            ("cli", SessionKind::Terminal),
            ("claude-desktop", SessionKind::Desktop),
            ("sdk-cli", SessionKind::Headless),
            ("sdk-ts", SessionKind::Headless),
        ];
        for (entrypoint, expected) in cases {
            let sample = with_records(
                sample(vec![claude(91190, 600, &["-p", "hi"])]),
                vec![record(91190, entrypoint, LIVE_ID, "/Users/ann/code/acme")],
            );
            let classification = run(&sample);
            assert_eq!(
                session(only_group(&classification)).kind,
                expected,
                "entrypoint {entrypoint}"
            );
        }
    }

    #[test]
    fn an_unknown_entrypoint_falls_back_to_argv_but_keeps_the_name() {
        let sample = with_records(
            sample(vec![claude(91190, 600, &["--print", "hi"])]),
            vec![record(91190, "vscode", LIVE_ID, "/Users/ann/code/acme")],
        );
        let classification = run(&sample);
        let facts = session(only_group(&classification));
        assert_eq!(facts.kind, SessionKind::Headless);
        assert_eq!(facts.name.as_deref(), Some("Fix checkout flow"));
    }

    #[test]
    fn a_stale_record_falls_back_to_argv() {
        let root = at(
            claude(91190, 600, &["-p", "summarize"]),
            Some("/Users/ann/code/a"),
        );
        let mut stale = record(91190, "claude-desktop", LIVE_ID, "/Users/ann/code/acme-web");
        stale.proc_start = Some(START - 60);
        let classification = run(&with_records(sample(vec![root]), vec![stale]));
        let group = only_group(&classification);
        let facts = session(group);
        assert_eq!(facts.kind, SessionKind::Headless);
        assert_eq!(facts.name, None);
        assert_eq!(facts.status, None);
        assert!(facts.session_ids.is_empty());
        assert_eq!(
            group.project.as_deref(),
            Some(Path::new("/Users/ann/code/a"))
        );
    }

    // REQ-003 and the rest of the fallback

    #[test]
    fn claude_dash_p_without_a_record_is_headless() {
        let classification = run(&sample(vec![claude(91190, 600, &["-p", "fix it"])]));
        assert_eq!(
            session(only_group(&classification)).kind,
            SessionKind::Headless
        );
    }

    #[test]
    fn the_desktop_bundled_claude_without_a_record_is_desktop() {
        let root = ProcessInfo {
            exe: Some(PathBuf::from(
                "/Users/ann/Library/Application Support/Claude/claude-code/2.1.4/claude",
            )),
            ..claude(91190, 600, &[])
        };
        let classification = run(&sample(vec![root]));
        assert_eq!(
            session(only_group(&classification)).kind,
            SessionKind::Desktop
        );
    }

    #[test]
    fn the_terminal_decides_the_last_fallback() {
        let cases = [
            (Terminal::Detached, SessionKind::Headless),
            (Terminal::Attached, SessionKind::Terminal),
            (Terminal::Unknown, SessionKind::Terminal),
        ];
        for (terminal, expected) in cases {
            let root = ProcessInfo {
                terminal,
                ..claude(91190, 600, &[])
            };
            let classification = run(&sample(vec![root]));
            assert_eq!(
                session(only_group(&classification)).kind,
                expected,
                "{terminal:?}"
            );
        }
    }

    // REQ-004

    #[test]
    fn a_nested_claude_p_is_inside_its_parents_tree() {
        let parent = claude(91190, 600, &[]);
        let bash = marked(proc(91250, 91190, 0.0), tool_marker(LIVE_ID));
        let nested = marked(
            claude(91260, 91250, &["-p", "review"]),
            tool_marker(LIVE_ID),
        );
        let grandchild = marked(proc(91270, 91260, 2.0), session_marker(DEAD_ID));
        let sample = with_records(
            sample(vec![parent, bash, nested, grandchild]),
            vec![record(91190, "cli", LIVE_ID, "/Users/ann/code/acme-web")],
        );
        let classification = run(&sample);
        let group = only_group(&classification);
        let nested = node_in(&classification, 91260);
        let facts = nested
            .nested_session
            .as_ref()
            .unwrap_or_else(|| panic!("nested facts: {nested:#?}"));
        assert_eq!(facts.kind, SessionKind::Headless);
        assert_eq!(facts.session_ids, BTreeSet::from([DEAD_ID.to_owned()]));
        assert_eq!(nested.guard, Some(GuardReason::NestedSession));
        assert_eq!(
            session(group).session_ids,
            BTreeSet::from([LIVE_ID.to_owned()])
        );
        assert!(
            classification
                .groups
                .iter()
                .all(|g| !matches!(g.kind, GroupKind::Leftover { .. })),
            "the nested session's ID is live"
        );
    }

    // REQ-005

    #[test]
    fn a_reparented_child_of_a_live_session_is_detached() {
        let root = claude(91190, 600, &[]);
        let server = marked(proc(70001, 1, 12.0), tool_marker(LIVE_ID));
        let worker = proc(70002, 70001, 4.0);
        let sample = with_records(
            sample(vec![root, server, worker]),
            vec![record(91190, "cli", LIVE_ID, "/Users/ann/code/acme-web")],
        );
        let classification = run(&sample);
        let group = only_group(&classification);
        assert_eq!(pids(&group.root.children), vec![70001]);
        let server = &group.root.children[0];
        assert!(server.detached);
        assert_eq!(pids(&server.children), vec![70002]);
        assert!(!server.children[0].detached);
        assert_eq!(group.totals, totals(Some(16.0), 0));
    }

    #[test]
    fn claude_pid_alone_attaches_a_reparented_process() {
        let root = claude(91190, 600, &[]);
        let child = marked(
            proc(70001, 1, 1.0),
            Markers {
                claude_pid: Some("91190".to_owned()),
                ..Markers::default()
            },
        );
        let classification = run(&sample(vec![root, child]));
        assert!(node_in(&classification, 70001).detached);
    }

    #[test]
    fn a_reparented_child_of_a_nested_session_stays_under_it() {
        let parent = claude(91190, 600, &[]);
        let nested = claude(91260, 91190, &["-p", "x"]);
        let nested_child = marked(proc(91270, 91260, 0.0), session_marker("nested-id"));
        let moved = marked(proc(70001, 1, 1.0), session_marker("nested-id"));
        let classification = run(&sample(vec![parent, nested, nested_child, moved]));
        let nested = node_in(&classification, 91260);
        assert_eq!(pids(&nested.children), vec![70001, 91270]);
        assert!(nested.children[0].detached);
    }

    // REQ-006, and the second concrete example

    #[test]
    fn an_orphan_with_a_dead_session_id_is_a_confirmed_leftover() {
        let zsh = ProcessInfo {
            name: "zsh".to_owned(),
            argv: argv(&["/bin/zsh", "-c", "npm run test:watch"]),
            ..unreadable(at(proc(52143, 1, 0.0), Some("/Users/ann/code/acme-api")))
        };
        let vitest = ProcessInfo {
            argv: argv(&["node", "vitest", "--watch"]),
            ..marked(proc(52150, 52143, 96.25), tool_marker(DEAD_ID))
        };
        let classification = run(&sample(vec![zsh, vitest]));
        let group = only_group(&classification);
        assert_eq!(leftover(group), (LeftoverTier::Confirmed, Some(DEAD_ID)));
        assert_eq!(group.totals, totals(Some(96.25), 0));
        assert_eq!(
            group.project.as_deref(),
            Some(Path::new("/Users/ann/code/acme-api"))
        );
        assert_eq!(group.root.guard, None);
    }

    #[test]
    fn a_confirmed_leftover_without_a_top_cwd_takes_the_marked_members() {
        let zsh = unreadable(at(proc(52143, 1, 0.0), None));
        let vitest = at(
            marked(proc(52150, 52143, 1.0), tool_marker(DEAD_ID)),
            Some("/Users/ann/code/acme-api"),
        );
        let classification = run(&sample(vec![zsh, vitest]));
        assert_eq!(
            only_group(&classification).project.as_deref(),
            Some(Path::new("/Users/ann/code/acme-api"))
        );
    }

    #[test]
    fn the_closed_session_id_is_the_first_in_pre_order() {
        let zsh = unreadable(proc(52143, 1, 0.0));
        let low = proc(52150, 52143, 0.0);
        let low_child = marked(proc(52190, 52150, 0.0), session_marker("first-id"));
        let high = marked(proc(52160, 52143, 50.0), session_marker("second-id"));
        let processes = vec![zsh, high, low, low_child];
        for sort in [SortKey::Cpu, SortKey::Memory] {
            let classification = run_by(&sample(processes.clone()), sort);
            assert_eq!(
                leftover(only_group(&classification)),
                (LeftoverTier::Confirmed, Some("first-id")),
                "{sort:?}"
            );
        }
    }

    // REQ-007, and the third and fourth concrete examples

    fn watch_script(cwd: &str) -> Sample {
        let sh = ProcessInfo {
            name: "sh".to_owned(),
            argv: argv(&["sh", "-c", "./watch.sh"]),
            ..unreadable(at(proc(60311, 1, 3.0), Some(cwd)))
        };
        let mut sample = sample(vec![sh]);
        sample.project_folders = vec!["-Users-ann-code-acme-api".to_owned()];
        sample
    }

    #[test]
    fn an_unmarked_orphan_in_a_project_is_a_probable_leftover() {
        let classification = run(&watch_script("/Users/ann/code/acme-api/scripts"));
        let group = only_group(&classification);
        assert_eq!(leftover(group), (LeftoverTier::Probable, None));
        assert_eq!(
            group.project.as_deref(),
            Some(Path::new("/Users/ann/code/acme-api/scripts"))
        );
    }

    #[test]
    fn an_unmarked_orphan_outside_projects_is_not_listed() {
        let classification = run(&watch_script("/"));
        assert!(classification.groups.is_empty(), "{classification:#?}");
    }

    #[test]
    fn an_orphan_inside_a_live_sessions_project_is_a_probable_leftover() {
        let root = claude(91190, 600, &[]);
        let sh = unreadable(at(
            proc(60311, 1, 1.0),
            Some("/Users/ann/code/acme-web/bin"),
        ));
        let sample = with_records(
            sample(vec![root, sh]),
            vec![record(91190, "cli", LIVE_ID, "/Users/ann/code/acme-web")],
        );
        let classification = run(&sample);
        let group = classification
            .groups
            .iter()
            .find(|group| group.root.process.identity.pid == 60311)
            .unwrap_or_else(|| panic!("listed: {classification:#?}"));
        assert_eq!(leftover(group).0, LeftoverTier::Probable);
    }

    #[test]
    fn an_orphan_with_a_readable_unmarked_top_is_not_listed() {
        let sh = at(proc(60311, 1, 1.0), Some("/Users/ann/code/acme-api"));
        let mut sample = sample(vec![sh]);
        sample.project_folders = vec!["-Users-ann-code-acme-api".to_owned()];
        assert!(run(&sample).groups.is_empty());
    }

    #[test]
    fn a_marked_orphan_without_a_session_id_is_not_listed() {
        let sh = unreadable(at(proc(60311, 1, 1.0), Some("/Users/ann/code/acme-api")));
        let node = marked(
            proc(60320, 60311, 1.0),
            Markers {
                claudecode: Some("1".to_owned()),
                ..Markers::default()
            },
        );
        let mut sample = sample(vec![sh, node]);
        sample.project_folders = vec!["-Users-ann-code-acme-api".to_owned()];
        assert!(run(&sample).groups.is_empty());
    }

    // REQ-008

    #[test]
    fn a_subtree_holding_a_live_session_is_never_a_leftover() {
        let zsh = unreadable(proc(52143, 1, 0.0));
        let tool = marked(proc(52150, 52143, 0.0), tool_marker(DEAD_ID));
        let nested = marked(claude(52160, 52150, &["-p", "x"]), tool_marker(DEAD_ID));
        let child = proc(52170, 52160, 5.0);
        let classification = run(&sample(vec![zsh, tool, nested, child]));
        let group = only_group(&classification);
        assert_eq!(group.root.process.identity.pid, 52160);
        assert_eq!(group.root.guard, Some(GuardReason::SessionRoot));
        assert_eq!(session(group).kind, SessionKind::Headless);
        assert_eq!(pids(&group.root.children), vec![52170]);
    }

    // REQ-009

    #[test]
    fn claude_pid_of_a_live_session_keeps_it_out_of_leftovers() {
        let root = claude(91190, 600, &[]);
        let zsh = unreadable(proc(52143, 1, 0.0));
        let node = marked(
            proc(52150, 52143, 7.0),
            Markers {
                claude_pid: Some("91190".to_owned()),
                ..tool_marker("after-clear-id")
            },
        );
        let classification = run(&sample(vec![root, zsh, node]));
        let group = only_group(&classification);
        assert!(matches!(group.kind, GroupKind::Session(_)));
        assert!(node_in(&classification, 52143).detached);
    }

    #[test]
    fn claude_pid_of_a_dead_process_still_confirms_a_leftover() {
        let zsh = unreadable(proc(52143, 1, 0.0));
        let node = marked(
            proc(52150, 52143, 7.0),
            Markers {
                claude_pid: Some("40404".to_owned()),
                ..tool_marker(DEAD_ID)
            },
        );
        let classification = run(&sample(vec![zsh, node]));
        assert_eq!(
            leftover(only_group(&classification)),
            (LeftoverTier::Confirmed, Some(DEAD_ID))
        );
    }

    // REQ-010

    #[test]
    fn loadtops_own_chain_is_never_targetable() {
        let root = claude(91190, 600, &[]);
        let zsh = marked(proc(91250, 91190, 0.0), tool_marker(LIVE_ID));
        let loadtop = ProcessInfo {
            identity: ProcessIdentity {
                pid: OWN.pid,
                start_time: START + 5,
            },
            ..marked(proc(OWN.pid, 91250, 1.0), tool_marker(LIVE_ID))
        };
        let lsof = proc(99002, OWN.pid, 0.0);
        let sibling = marked(proc(91300, 91190, 3.0), tool_marker(LIVE_ID));
        let classification = run(&sample(vec![root, zsh, loadtop, lsof, sibling]));
        for pid in [91190, 91250, OWN.pid, 99002] {
            assert_eq!(
                node_in(&classification, pid).guard,
                Some(GuardReason::Loadtop),
                "{pid}"
            );
        }
        assert_eq!(node_in(&classification, 91300).guard, None);
    }

    #[test]
    fn a_session_root_is_guarded() {
        let root = claude(91190, 600, &[]);
        let child = marked(proc(91204, 91190, 1.0), tool_marker(LIVE_ID));
        let classification = run(&sample(vec![root, child]));
        assert_eq!(
            node_in(&classification, 91190).guard,
            Some(GuardReason::SessionRoot)
        );
        assert_eq!(node_in(&classification, 91204).guard, None);
    }

    #[test]
    fn loadtop_above_a_nested_session_wins_over_the_nested_guard() {
        let root = claude(91190, 600, &[]);
        let loadtop = proc(OWN.pid, 91190, 0.0);
        let nested_under_loadtop = claude(91260, OWN.pid, &["-p", "x"]);
        let classification = run(&sample(vec![root, loadtop, nested_under_loadtop]));
        assert_eq!(
            node_in(&classification, 91260).guard,
            Some(GuardReason::Loadtop)
        );
    }

    // REQ-011

    #[test]
    fn totals_cover_the_whole_tree() {
        let root = ProcessInfo {
            rss_bytes: 100,
            ..ProcessInfo {
                cpu_percent: Some(0.5),
                ..claude(91190, 600, &[])
            }
        };
        let a = ProcessInfo {
            rss_bytes: 1_000,
            ..proc(91204, 91190, 2.0)
        };
        let a1 = ProcessInfo {
            rss_bytes: 10_000,
            ..proc(91205, 91204, 4.0)
        };
        let b = ProcessInfo {
            rss_bytes: 50,
            cpu_percent: None,
            ..proc(91206, 91190, 0.0)
        };
        let classification = run(&sample(vec![root, a, a1, b]));
        let group = only_group(&classification);
        assert_eq!(group.totals, totals(Some(6.5), 11_150));
        assert_eq!(group.root.subtree, group.totals);
        assert_eq!(
            node_in(&classification, 91204).subtree,
            totals(Some(6.0), 11_000)
        );
        assert_eq!(node_in(&classification, 91206).subtree, totals(None, 50));
    }

    #[test]
    fn the_claude_total_sums_every_group() {
        let one = ProcessInfo {
            rss_bytes: 1_000,
            ..ProcessInfo {
                cpu_percent: Some(10.0),
                ..claude(91190, 600, &[])
            }
        };
        let two = ProcessInfo {
            rss_bytes: 2_000,
            ..ProcessInfo {
                cpu_percent: Some(20.5),
                ..claude(92190, 600, &[])
            }
        };
        let zsh = ProcessInfo {
            rss_bytes: 500,
            ..unreadable(proc(52143, 1, 0.0))
        };
        let node = ProcessInfo {
            rss_bytes: 250,
            ..marked(proc(52150, 52143, 1.0), tool_marker(DEAD_ID))
        };
        let outside = ProcessInfo {
            rss_bytes: 9_999,
            ..proc(40000, 1, 99.0)
        };
        let sample = sample(vec![one, two, zsh, node, outside]);
        let classification = run(&sample);
        assert_eq!(classification.groups.len(), 3);
        assert_eq!(classification.claude_totals, totals(Some(31.5), 3_750));
        assert_eq!(classification.machine, sample.processes.machine);
    }

    // REQ-012

    #[test]
    fn groups_are_heaviest_first_by_cpu() {
        let light = claude(91190, 600, &[]);
        let light_child = proc(91204, 91190, 1.0);
        let heavy = claude(92190, 600, &[]);
        let heavy_low = proc(92210, 92190, 5.0);
        let heavy_high = proc(92220, 92190, 50.0);
        let tie_a = proc(92230, 92190, 5.0);
        let zsh = unreadable(proc(52143, 1, 0.0));
        let leftover = marked(proc(52150, 52143, 20.0), tool_marker(DEAD_ID));
        let classification = run(&sample(vec![
            light,
            light_child,
            heavy,
            heavy_low,
            heavy_high,
            tie_a,
            zsh,
            leftover,
        ]));
        assert_eq!(group_pids(&classification), vec![92190, 52143, 91190]);
        assert_eq!(
            pids(&classification.groups[0].root.children),
            vec![92220, 92210, 92230]
        );
    }

    #[test]
    fn the_memory_key_orders_by_rss() {
        let small = ProcessInfo {
            rss_bytes: 10,
            ..ProcessInfo {
                cpu_percent: Some(90.0),
                ..claude(91190, 600, &[])
            }
        };
        let big = ProcessInfo {
            rss_bytes: 5_000,
            ..claude(92190, 600, &[])
        };
        let big_a = ProcessInfo {
            rss_bytes: 100,
            ..proc(92210, 92190, 80.0)
        };
        let big_b = ProcessInfo {
            rss_bytes: 900,
            ..proc(92220, 92190, 1.0)
        };
        let classification = run_by(&sample(vec![small, big, big_a, big_b]), SortKey::Memory);
        assert_eq!(group_pids(&classification), vec![92190, 91190]);
        assert_eq!(
            pids(&classification.groups[0].root.children),
            vec![92220, 92210]
        );
    }

    #[test]
    fn subtree_totals_order_siblings_not_their_own_load() {
        let root = claude(91190, 600, &[]);
        let shell = proc(91204, 91190, 0.0);
        let busy = proc(91205, 91204, 70.0);
        let direct = proc(91210, 91190, 30.0);
        let classification = run(&sample(vec![root, shell, busy, direct]));
        assert_eq!(
            pids(&only_group(&classification).root.children),
            vec![91204, 91210]
        );
    }

    // REQ-013

    #[test]
    fn a_session_collects_ids_from_before_clear() {
        let root = claude(91190, 600, &[]);
        let old_mcp = marked(proc(81006, 91190, 0.0), session_marker("before-clear-id"));
        let new_tool = marked(proc(91204, 91190, 0.0), tool_marker(LIVE_ID));
        let grandchild = marked(proc(91205, 91204, 0.0), tool_marker("older-id"));
        let sample = with_records(
            sample(vec![root, old_mcp, new_tool, grandchild]),
            vec![record(91190, "cli", LIVE_ID, "/Users/ann/code/acme-web")],
        );
        let classification = run(&sample);
        assert_eq!(
            session(only_group(&classification)).session_ids,
            BTreeSet::from([
                LIVE_ID.to_owned(),
                "before-clear-id".to_owned(),
                "older-id".to_owned()
            ])
        );
    }

    #[test]
    fn an_orphan_carrying_a_pre_clear_id_is_not_a_leftover() {
        let root = claude(91190, 600, &[]);
        let old_mcp = marked(proc(81006, 91190, 0.0), session_marker("before-clear-id"));
        let zsh = unreadable(proc(52143, 1, 0.0));
        let moved = marked(proc(52150, 52143, 0.0), tool_marker("before-clear-id"));
        let classification = run(&sample(vec![root, old_mcp, zsh, moved]));
        let group = only_group(&classification);
        assert!(matches!(group.kind, GroupKind::Session(_)));
        assert!(node_in(&classification, 52143).detached);
    }

    // Boundary conditions

    #[test]
    fn an_empty_sample_has_no_groups_and_zero_totals() {
        let classification = run(&sample(Vec::new()));
        assert!(classification.groups.is_empty());
        assert_eq!(classification.claude_totals, totals(Some(0.0), 0));
    }

    #[test]
    fn leftovers_alone_are_listed_without_a_session() {
        let zsh = unreadable(proc(52143, 1, 0.0));
        let node = marked(proc(52150, 52143, 9.0), tool_marker(DEAD_ID));
        let classification = run(&sample(vec![zsh, node]));
        assert_eq!(
            leftover(only_group(&classification)).0,
            LeftoverTier::Confirmed
        );
    }

    #[test]
    fn a_first_sample_has_no_cpu_and_orders_by_pid() {
        let first = ProcessInfo {
            cpu_percent: None,
            ..claude(92190, 600, &[])
        };
        let second = ProcessInfo {
            cpu_percent: None,
            ..claude(91190, 600, &[])
        };
        let child_b = ProcessInfo {
            cpu_percent: None,
            ..proc(91210, 91190, 0.0)
        };
        let child_a = ProcessInfo {
            cpu_percent: None,
            ..proc(91204, 91190, 0.0)
        };
        let classification = run(&sample(vec![first, second, child_b, child_a]));
        assert_eq!(group_pids(&classification), vec![91190, 92190]);
        assert_eq!(
            pids(&classification.groups[0].root.children),
            vec![91204, 91210]
        );
        for group in &classification.groups {
            assert_eq!(group.totals.cpu_percent, None);
        }
        assert_eq!(classification.claude_totals.cpu_percent, None);
    }

    #[test]
    fn a_root_without_a_cwd_takes_the_first_marked_descendants() {
        let root = at(claude(91190, 600, &[]), None);
        let unmarked = at(proc(91204, 91190, 0.0), Some("/tmp"));
        let marked_child = at(
            marked(proc(91210, 91190, 0.0), tool_marker(LIVE_ID)),
            Some("/Users/ann/code/acme-web"),
        );
        let classification = run(&sample(vec![root.clone(), unmarked, marked_child]));
        assert_eq!(
            only_group(&classification).project.as_deref(),
            Some(Path::new("/Users/ann/code/acme-web"))
        );
        let alone = run(&sample(vec![root]));
        assert_eq!(only_group(&alone).project, None);
    }

    #[test]
    fn a_process_whose_parent_is_absent_attaches_only_by_markers() {
        let root = claude(91190, 600, &[]);
        let attached = marked(proc(70001, 4242, 1.0), tool_marker(LIVE_ID));
        let unmarked = unreadable(at(proc(70010, 4243, 1.0), Some("/Users/ann/code/acme-api")));
        let dead = marked(proc(70020, 4244, 1.0), tool_marker(DEAD_ID));
        let mut sample = with_records(
            sample(vec![root, attached, unmarked, dead]),
            vec![record(91190, "cli", LIVE_ID, "/Users/ann/code/acme-web")],
        );
        sample.project_folders = vec!["-Users-ann-code-acme-api".to_owned()];
        let classification = run(&sample);
        let group = only_group(&classification);
        assert_eq!(pids(&group.root.children), vec![70001]);
        assert!(group.root.children[0].detached);
    }

    #[test]
    fn loadtop_inside_a_session_leaves_the_other_children_targetable() {
        let root = claude(91190, 600, &[]);
        let loadtop = proc(OWN.pid, 91190, 0.5);
        let lsof = proc(99002, OWN.pid, 0.0);
        let vite = proc(91204, 91190, 40.0);
        let vite_child = proc(91205, 91204, 1.0);
        let classification = run(&sample(vec![root, loadtop, lsof, vite, vite_child]));
        assert_eq!(
            node_in(&classification, 91190).guard,
            Some(GuardReason::Loadtop)
        );
        assert_eq!(
            node_in(&classification, OWN.pid).guard,
            Some(GuardReason::Loadtop)
        );
        assert_eq!(
            node_in(&classification, 99002).guard,
            Some(GuardReason::Loadtop)
        );
        assert_eq!(node_in(&classification, 91204).guard, None);
        assert_eq!(node_in(&classification, 91205).guard, None);
    }

    #[test]
    fn five_hundred_children_keep_exact_totals() {
        let mut processes = vec![claude(91190, 600, &[])];
        processes.extend((0..500).map(|n| ProcessInfo {
            rss_bytes: 1_024,
            ..proc(100_000 + n, 91190, 0.25)
        }));
        let classification = run(&sample(processes));
        let group = only_group(&classification);
        assert_eq!(group.root.children.len(), 500);
        assert_eq!(group.totals, totals(Some(125.0), 512_000));
    }

    #[test]
    fn unavailable_ports_show_on_every_node() {
        let root = claude(91190, 600, &[]);
        let child = proc(91204, 91190, 0.0);
        let mut sample = sample(vec![root, child]);
        sample.sockets = Some(Err(SocketError::Unavailable));
        let classification = run(&sample);
        assert_eq!(
            node_in(&classification, 91190).ports,
            PortsView::Unavailable
        );
        assert_eq!(
            node_in(&classification, 91204).ports,
            PortsView::Unavailable
        );
    }

    #[test]
    fn ports_not_read_this_sample_say_so() {
        let classification = run(&sample(vec![claude(91190, 600, &[])]));
        assert_eq!(node_in(&classification, 91190).ports, PortsView::NotRead);
    }

    #[test]
    fn read_ports_are_listed_per_process_in_order() {
        let root = claude(91190, 600, &[]);
        let vite = proc(91204, 91190, 0.0);
        let mut sample = sample(vec![root, vite]);
        sample.sockets = Some(Ok(vec![
            ListeningPort {
                pid: 91204,
                port: 5173,
            },
            ListeningPort {
                pid: 91204,
                port: 24678,
            },
            ListeningPort {
                pid: 91204,
                port: 5173,
            },
            ListeningPort {
                pid: 40000,
                port: 8080,
            },
        ]));
        let classification = run(&sample);
        assert_eq!(
            node_in(&classification, 91204).ports,
            PortsView::Read(vec![5173, 24678])
        );
        assert_eq!(
            node_in(&classification, 91190).ports,
            PortsView::Read(Vec::new())
        );
    }

    #[test]
    fn a_malformed_parent_chain_does_not_hang() {
        let own_parent = proc(500, 500, 1.0);
        let cycle_a = claude(600, 601, &[]);
        let cycle_b = proc(601, 600, 1.0);
        let init = ProcessInfo {
            ppid: None,
            ..proc(1, 0, 0.0)
        };
        let classification = run(&sample(vec![own_parent, cycle_a, cycle_b, init]));
        assert!(classification.groups.is_empty(), "{classification:#?}");
    }

    // home_relative

    #[test]
    fn paths_are_shown_home_relative() {
        let home = Path::new(HOME);
        let cases = [
            ("/Users/ann", "~"),
            ("/Users/ann/code/acme-web", "~/code/acme-web"),
            ("/Users/annex/code", "/Users/annex/code"),
            ("/opt/acme", "/opt/acme"),
        ];
        for (path, expected) in cases {
            assert_eq!(
                home_relative(Path::new(path), home),
                PathBuf::from(expected),
                "{path}"
            );
        }
    }

    #[test]
    fn an_empty_home_leaves_the_path_unchanged() {
        assert_eq!(
            home_relative(Path::new("/opt/acme"), Path::new("")),
            PathBuf::from("/opt/acme")
        );
    }
}
