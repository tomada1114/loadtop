//! The JSON shapes core serializes, and the one that reaches disk, which is contract
//! (`AGENTS.md` › "What is contract and what is private").
//! These tests pin them with literal JSON, independent of serde's derive.

use loadtop_core::{CounterError, CounterView, StorageErrorKind, StoredCounter, UnixMillis};
use serde_json::json;

#[test]
fn counter_view_is_camel_case_with_a_numeric_time() {
    let view = CounterView {
        value: 3,
        last_changed_at: Some(UnixMillis(1_700_000_000_000)),
        revision: 4,
    };
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        json!({ "value": 3, "lastChangedAt": 1_700_000_000_000_i64, "revision": 4 })
    );
}

#[test]
fn a_counter_view_never_changed_has_a_null_time() {
    let view = CounterView {
        value: 0,
        last_changed_at: None,
        revision: 0,
    };
    assert_eq!(
        serde_json::to_value(&view).unwrap(),
        json!({ "value": 0, "lastChangedAt": null, "revision": 0 })
    );
}

#[test]
fn counter_errors_serialize_as_codes() {
    assert_eq!(
        serde_json::to_value(CounterError::AtMaximum).unwrap(),
        json!({ "code": "atMaximum" })
    );
    assert_eq!(
        serde_json::to_value(CounterError::AtMinimum).unwrap(),
        json!({ "code": "atMinimum" })
    );
    assert_eq!(
        serde_json::to_value(CounterError::Storage {
            kind: StorageErrorKind::Corrupt
        })
        .unwrap(),
        json!({ "code": "storage", "kind": "corrupt" })
    );
    assert_eq!(
        serde_json::to_value(CounterError::Storage {
            kind: StorageErrorKind::Unavailable
        })
        .unwrap(),
        json!({ "code": "storage", "kind": "unavailable" })
    );
}

#[test]
fn a_stored_counter_round_trips_through_its_file_format() {
    let stored = StoredCounter {
        value: 4,
        last_changed_at: Some(UnixMillis(12)),
    };
    let text = json!({ "value": 4, "lastChangedAt": 12 });
    assert_eq!(serde_json::to_value(&stored).unwrap(), text);
    assert_eq!(
        serde_json::from_value::<StoredCounter>(text).unwrap(),
        stored
    );
}

#[test]
fn error_messages_carry_no_data() {
    assert_eq!(
        CounterError::AtMaximum.to_string(),
        "the counter is at its maximum"
    );
    assert_eq!(
        CounterError::AtMinimum.to_string(),
        "the counter is at its minimum"
    );
    assert_eq!(
        CounterError::Storage {
            kind: StorageErrorKind::Corrupt
        }
        .to_string(),
        "counter storage failed: Corrupt"
    );
}

mod json_report {
    use std::collections::BTreeSet;
    use std::path::{Path, PathBuf};
    use std::time::Duration;

    use loadtop_core::{
        ChildKind, Classification, EnvRead, Group, GroupKind, JsonReport, LeftoverTier,
        MachineTotals, Markers, Node, PortsView, ProcessIdentity, ProcessInfo, RunState,
        SessionFacts, SessionKind, Terminal, Totals, UnixMillis,
    };
    use serde_json::json;

    const HOME: &str = "/Users/ann";
    const LIVE_ID: &str = "3f9c2a10-0000-4000-8000-000000000001";
    const DEAD_ID: &str = "8b41d0e7-0000-4000-8000-000000000002";
    const NESTED_ID: &str = "5d2e7b44-0000-4000-8000-000000000003";

    fn process(id: u32, parent: u32, name: &str, argv: &[&str]) -> ProcessInfo {
        ProcessInfo {
            identity: ProcessIdentity {
                pid: id,
                start_time: 1_763_647_923,
            },
            ppid: Some(parent),
            name: name.to_owned(),
            argv: argv.iter().map(|arg| (*arg).to_owned()).collect(),
            exe: None,
            cwd: None,
            cpu_percent: Some(0.0),
            rss_bytes: 0,
            state: RunState::Running,
            terminal: Terminal::Detached,
            env: EnvRead::Readable(Markers::default()),
        }
    }

    fn node(process: ProcessInfo, child_kind: ChildKind, ports: PortsView) -> Node {
        Node {
            subtree: Totals {
                cpu_percent: process.cpu_percent,
                rss_bytes: process.rss_bytes,
            },
            process,
            child_kind,
            ports,
            detached: false,
            nested_session: None,
            guard: None,
            children: Vec::new(),
        }
    }

    fn ids(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(|id| (*id).to_owned()).collect()
    }

    /// A desktop session holding a tool child, which holds a stopped nested session
    /// whose ports could not be read; a confirmed leftover whose argv is unreadable; and
    /// a probable leftover outside the home directory.
    fn classification() -> Classification {
        let mut nested = node(
            ProcessInfo {
                cpu_percent: None,
                rss_bytes: 52_428_800,
                state: RunState::Stopped,
                env: EnvRead::Unreadable,
                ..process(91300, 91204, "claude", &["claude", "-p", "fix"])
            },
            ChildKind::Unreadable,
            PortsView::Unavailable,
        );
        nested.nested_session = Some(SessionFacts {
            kind: SessionKind::Headless,
            name: None,
            status: None,
            session_ids: ids(&[NESTED_ID]),
        });
        let mut vite = node(
            ProcessInfo {
                cpu_percent: Some(141.2),
                rss_bytes: 641_728_512,
                ..process(91204, 91190, "node", &["node", "vite"])
            },
            ChildKind::Tool,
            PortsView::Read(vec![5173]),
        );
        vite.children = vec![nested];
        let mut root = node(
            ProcessInfo {
                cpu_percent: Some(0.4),
                rss_bytes: 104_857_600,
                ..process(91190, 1, "claude", &["claude"])
            },
            ChildKind::Unmarked,
            PortsView::Read(vec![]),
        );
        root.children = vec![vite];
        let session = Group {
            kind: GroupKind::Session(SessionFacts {
                kind: SessionKind::Desktop,
                name: Some("Fix checkout flow".to_owned()),
                status: Some("busy".to_owned()),
                session_ids: ids(&[LIVE_ID, NESTED_ID]),
            }),
            root,
            totals: Totals {
                cpu_percent: Some(182.0),
                rss_bytes: 1_717_986_918,
            },
            project: Some(PathBuf::from("/Users/ann/code/acme-web")),
        };
        let confirmed = Group {
            kind: GroupKind::Leftover {
                tier: LeftoverTier::Confirmed,
                closed_session_id: Some(DEAD_ID.to_owned()),
            },
            root: node(
                ProcessInfo {
                    cpu_percent: Some(96.3),
                    rss_bytes: 775_946_240,
                    ..process(52143, 1, "zsh", &[])
                },
                ChildKind::Mcp,
                PortsView::NotRead,
            ),
            totals: Totals {
                cpu_percent: Some(96.3),
                rss_bytes: 775_946_240,
            },
            project: Some(PathBuf::from("/Users/ann/code/acme-api")),
        };
        let probable = Group {
            kind: GroupKind::Leftover {
                tier: LeftoverTier::Probable,
                closed_session_id: None,
            },
            root: node(
                process(60001, 1, "node", &["node", "server.js"]),
                ChildKind::Unreadable,
                PortsView::Read(vec![3000, 3001]),
            ),
            totals: Totals {
                cpu_percent: Some(0.0),
                rss_bytes: 0,
            },
            project: Some(PathBuf::from("/opt/work/acme-ops")),
        };
        Classification {
            groups: vec![session, confirmed, probable],
            claude_totals: Totals {
                cpu_percent: Some(310.0),
                rss_bytes: 3_113_851_289,
            },
            machine: MachineTotals {
                cpu_count: 8,
                cpu_percent: Some(520.0),
                memory_used_bytes: 15_246_955_315,
                memory_total_bytes: 17_179_869_184,
            },
        }
    }

    fn report(classification: &Classification) -> JsonReport {
        JsonReport::new(
            classification,
            UnixMillis(1_763_650_000_000),
            Duration::from_millis(300),
            Path::new(HOME),
        )
    }

    #[test]
    fn json_report_matches_the_documented_shape() {
        // Built bottom-up: one literal this deep passes `json!`'s recursion limit.
        let nested = json!({
            "pid": 91300, "ppid": 91204, "start_time": 1_763_647_923, "name": "claude",
            "command": "claude -p fix", "kind": null, "cpu_percent": null,
            "rss_bytes": 52_428_800, "ports": null, "state": "stopped",
            "session": { "kind": "headless", "name": null, "status": null,
                         "session_ids": [NESTED_ID] },
            "children": []
        });
        let vite = json!({
            "pid": 91204, "ppid": 91190, "start_time": 1_763_647_923, "name": "node",
            "command": "node vite", "kind": "tool", "cpu_percent": 141.2,
            "rss_bytes": 641_728_512, "ports": [5173], "state": "running", "session": null,
            "children": [nested]
        });
        let claude = json!({
            "pid": 91190, "ppid": 1, "start_time": 1_763_647_923, "name": "claude",
            "command": "claude", "kind": null, "cpu_percent": 0.4, "rss_bytes": 104_857_600,
            "ports": [], "state": "running", "session": null, "children": [vite]
        });
        let zsh = json!({
            "pid": 52143, "ppid": 1, "start_time": 1_763_647_923, "name": "zsh",
            "command": null, "kind": "mcp", "cpu_percent": 96.3, "rss_bytes": 775_946_240,
            "ports": null, "state": "running", "session": null, "children": []
        });
        let server = json!({
            "pid": 60001, "ppid": 1, "start_time": 1_763_647_923, "name": "node",
            "command": "node server.js", "kind": null, "cpu_percent": 0.0, "rss_bytes": 0,
            "ports": [3000, 3001], "state": "running", "session": null, "children": []
        });
        assert_eq!(
            serde_json::to_value(report(&classification())).unwrap(),
            json!({
                "schema_version": 1,
                "sampled_at_ms": 1_763_650_000_000_i64,
                "cpu_window_ms": 300,
                "machine": { "cpu_cores": 8, "cpu_percent": 520.0,
                             "memory_used_bytes": 15_246_955_315_u64,
                             "memory_total_bytes": 17_179_869_184_u64 },
                "claude": { "cpu_percent": 310.0, "rss_bytes": 3_113_851_289_u64 },
                "sessions": [
                    { "kind": "desktop", "project": "~/code/acme-web",
                      "name": "Fix checkout flow", "status": "busy",
                      "session_ids": [LIVE_ID, NESTED_ID], "cpu_percent": 182.0,
                      "rss_bytes": 1_717_986_918_u64, "root": claude }
                ],
                "leftovers": [
                    { "tier": "confirmed", "closed_session_id": DEAD_ID,
                      "project": "~/code/acme-api", "cpu_percent": 96.3,
                      "rss_bytes": 775_946_240, "root": zsh },
                    { "tier": "probable", "closed_session_id": null,
                      "project": "/opt/work/acme-ops", "cpu_percent": 0.0, "rss_bytes": 0,
                      "root": server }
                ]
            })
        );
    }

    #[test]
    fn schema_version_is_the_first_key_of_the_compact_document() {
        let text = serde_json::to_string(&report(&classification())).unwrap();
        assert!(text.starts_with(r#"{"schema_version":1,"#), "{text}");
        assert!(!text.contains('\n'), "{text}");
    }

    #[test]
    fn an_empty_classification_has_empty_lists_and_unknown_fields_stay_null() {
        let empty = Classification {
            groups: Vec::new(),
            claude_totals: Totals {
                cpu_percent: Some(0.0),
                rss_bytes: 0,
            },
            machine: MachineTotals {
                cpu_count: 4,
                cpu_percent: None,
                memory_used_bytes: 1,
                memory_total_bytes: 2,
            },
        };
        assert_eq!(
            serde_json::to_value(report(&empty)).unwrap(),
            json!({
                "schema_version": 1,
                "sampled_at_ms": 1_763_650_000_000_i64,
                "cpu_window_ms": 300,
                "machine": { "cpu_cores": 4, "cpu_percent": null,
                             "memory_used_bytes": 1, "memory_total_bytes": 2 },
                "claude": { "cpu_percent": 0.0, "rss_bytes": 0 },
                "sessions": [],
                "leftovers": []
            })
        );
    }

    #[test]
    fn a_session_without_a_project_or_record_has_nulls_and_its_kind() {
        let mut classification = classification();
        classification.groups.truncate(1);
        let group = &mut classification.groups[0];
        group.project = None;
        group.kind = GroupKind::Session(SessionFacts {
            kind: SessionKind::Terminal,
            name: None,
            status: None,
            session_ids: BTreeSet::new(),
        });
        let value = serde_json::to_value(report(&classification)).unwrap();
        let session = &value["sessions"][0];
        assert_eq!(session["kind"], json!("terminal"));
        assert_eq!(session["project"], json!(null));
        assert_eq!(session["name"], json!(null));
        assert_eq!(session["status"], json!(null));
        assert_eq!(session["session_ids"], json!([]));
    }

    #[test]
    fn a_project_equal_to_home_is_a_tilde() {
        let mut classification = classification();
        classification.groups.truncate(1);
        classification.groups[0].project = Some(PathBuf::from(HOME));
        let value = serde_json::to_value(report(&classification)).unwrap();
        assert_eq!(value["sessions"][0]["project"], json!("~"));
    }
}
