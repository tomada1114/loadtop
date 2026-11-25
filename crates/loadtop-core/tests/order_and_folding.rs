//! The 3-sample order window and folding over hand-built samples (requirements §3.2
//! Collapse, Order stability, Default order, Sort key; ux-flows S1).

mod order_and_folding {
    use std::path::{Path, PathBuf};

    use loadtop_core::{
        AppTuning, Classification, EnvRead, FoldState, FoldTuning, Folds, History, Markers,
        OrderTuning, ProcessIdentity, ProcessInfo, Row, RowId, RunState, Sample, SortKey, Terminal,
        UnixMillis, classify, order_by_window, rows,
    };
    use loadtop_test_support::{process_with, sample_of};

    const HOME: &str = "/Users/ann";
    const START: u64 = 1_763_647_923;
    const DEAD_ID: &str = "8b41d0e7-0000-4000-8000-000000000002";
    const OWN: ProcessIdentity = ProcessIdentity {
        pid: 99001,
        start_time: START,
    };
    const MIB: u64 = 1_048_576;

    fn id(pid: u32) -> ProcessIdentity {
        ProcessIdentity {
            pid,
            start_time: START,
        }
    }

    /// A running process under `parent` with a readable, unmarked environment.
    fn proc(pid: u32, parent: u32, cpu: f64) -> ProcessInfo {
        ProcessInfo {
            ppid: Some(parent),
            cpu_percent: Some(cpu),
            ..process_with(id(pid))
        }
    }

    fn with_rss(mut process: ProcessInfo, rss_bytes: u64) -> ProcessInfo {
        process.rss_bytes = rss_bytes;
        process
    }

    fn stopped(mut process: ProcessInfo) -> ProcessInfo {
        process.state = RunState::Stopped;
        process
    }

    /// A `claude` session process whose parent (a shell) is not in the table.
    fn claude(pid: u32) -> ProcessInfo {
        ProcessInfo {
            name: "claude".to_owned(),
            argv: vec!["claude".to_owned()],
            exe: Some(PathBuf::from("/Users/ann/.local/bin/claude")),
            terminal: Terminal::Unknown,
            env: EnvRead::Unreadable,
            ..proc(pid, 500, 0.0)
        }
    }

    /// The top of a confirmed leftover: re-parented to launchd, carrying a dead session.
    fn leftover_top(pid: u32, cpu: f64) -> ProcessInfo {
        ProcessInfo {
            env: EnvRead::Readable(Markers {
                claudecode: Some("1".to_owned()),
                claude_code_session_id: Some(DEAD_ID.to_owned()),
                ..Markers::default()
            }),
            ..proc(pid, 1, cpu)
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

    fn history() -> History {
        History::new(AppTuning::default().order)
    }

    fn fold_tuning() -> FoldTuning {
        AppTuning::default().fold
    }

    fn classified(sample: &Sample, sort: SortKey) -> Classification {
        classify(sample, OWN, Path::new(HOME), sort)
    }

    /// Applies every sample in turn and orders the last one by the window.
    fn ordered(samples: &[Sample], sort: SortKey) -> Classification {
        let history = samples.iter().fold(history(), History::apply);
        let last = samples
            .last()
            .map_or_else(|| sample(Vec::new()), Clone::clone);
        order_by_window(classified(&last, sort), &history, sort)
    }

    fn group_pids(classification: &Classification) -> Vec<u32> {
        classification
            .groups
            .iter()
            .map(|group| group.root.process.identity.pid)
            .collect()
    }

    fn child_pids(classification: &Classification, group: usize) -> Vec<u32> {
        classification.groups[group]
            .root
            .children
            .iter()
            .map(|node| node.process.identity.pid)
            .collect()
    }

    fn cpu_readings(history: &History, pid: u32) -> Vec<Option<f64>> {
        history
            .readings(id(pid))
            .iter()
            .map(|reading| reading.cpu_percent)
            .collect()
    }

    /// A row as a test reads it: the variant, the PID, and the values that matter.
    #[derive(Debug, Clone, PartialEq)]
    enum Seen {
        Group(u32, FoldState),
        Process(u32, Vec<bool>),
        More(usize, Option<f64>, u64),
        Lifted(u32),
    }

    fn seen(rows: &[Row<'_>]) -> Vec<Seen> {
        rows.iter()
            .map(|row| match row {
                Row::Group { group, fold } => Seen::Group(group.root.process.identity.pid, *fold),
                Row::Process {
                    node, depth, last, ..
                } => {
                    assert_eq!(*depth, last.len(), "depth and last flags of {row:?}");
                    Seen::Process(node.process.identity.pid, last.clone())
                }
                Row::More { count, totals, .. } => {
                    Seen::More(*count, totals.cpu_percent, totals.rss_bytes)
                }
                Row::Lifted { node, .. } => Seen::Lifted(node.process.identity.pid),
            })
            .collect()
    }

    fn rows_of(classification: &Classification, folds: &Folds) -> Vec<Seen> {
        seen(&rows(classification, folds, fold_tuning()))
    }

    /// Session 91190 with five child subtrees A–E: A 91201 (180.0, one child 91211),
    /// B 91202 (40.0), C 91203 (12.0), D 91204 (2.0, 98M, with child 71002 at 0.0, 22M),
    /// E 91205 (0.5, 30M).
    fn five_subtrees(paused_in_d: bool) -> Sample {
        let d_child = with_rss(proc(71002, 91204, 0.0), 22 * MIB);
        sample(vec![
            claude(91190),
            proc(91201, 91190, 170.0),
            proc(91211, 91201, 10.0),
            proc(91202, 91190, 40.0),
            proc(91203, 91190, 12.0),
            with_rss(proc(91204, 91190, 2.0), 98 * MIB),
            if paused_in_d {
                stopped(d_child)
            } else {
                d_child
            },
            with_rss(proc(91205, 91190, 0.5), 30 * MIB),
        ])
    }

    fn group_91190() -> RowId {
        RowId::Group(id(91190))
    }

    // History

    #[test]
    fn history_keeps_only_the_last_three_cpu_samples_per_process() {
        let history = [10.0, 20.0, 150.0, 5.0]
            .into_iter()
            .map(|cpu| sample(vec![proc(91204, 500, cpu)]))
            .fold(history(), |history, sample| history.apply(&sample));
        assert_eq!(
            cpu_readings(&history, 91204),
            vec![Some(20.0), Some(150.0), Some(5.0)]
        );
        let mean = history.cpu_mean(id(91204)).unwrap_or(f64::NAN);
        assert!((mean - 58.333_333).abs() < 1e-5, "mean {mean}");
    }

    #[test]
    fn history_drops_a_process_absent_from_the_latest_sample() {
        let history = history()
            .apply(&sample(vec![
                proc(91204, 500, 10.0),
                proc(91230, 500, 20.0),
            ]))
            .apply(&sample(vec![proc(91204, 500, 11.0)]));
        assert_eq!(cpu_readings(&history, 91230), Vec::<Option<f64>>::new());
        assert_eq!(history.cpu_mean(id(91230)), None);
        assert_eq!(cpu_readings(&history, 91204), vec![Some(10.0), Some(11.0)]);

        let back = history.apply(&sample(vec![proc(91230, 500, 30.0)]));
        assert_eq!(cpu_readings(&back, 91230), vec![Some(30.0)]);
    }

    #[test]
    fn a_reused_pid_with_a_new_start_time_starts_a_fresh_history() {
        let old = history()
            .apply(&sample(vec![proc(91204, 500, 140.0)]))
            .apply(&sample(vec![proc(91204, 500, 141.0)]))
            .apply(&sample(vec![proc(91204, 500, 141.2)]));
        let reused = ProcessIdentity {
            pid: 91204,
            start_time: 1_763_650_100,
        };
        let new = old.apply(&sample(vec![ProcessInfo {
            ppid: Some(500),
            cpu_percent: Some(3.0),
            ..process_with(reused)
        }]));
        let readings: Vec<Option<f64>> = new
            .readings(reused)
            .iter()
            .map(|reading| reading.cpu_percent)
            .collect();
        assert_eq!(readings, vec![Some(3.0)]);
        assert_eq!(cpu_readings(&new, 91204), Vec::<Option<f64>>::new());
    }

    #[test]
    fn a_process_with_fewer_samples_than_the_window_uses_the_mean_it_has() {
        let one = history().apply(&sample(vec![proc(91204, 500, 12.0)]));
        assert_eq!(one.cpu_mean(id(91204)), Some(12.0));
        let two = one.apply(&sample(vec![proc(91204, 500, 18.0)]));
        assert_eq!(two.cpu_mean(id(91204)), Some(15.0));
    }

    #[test]
    fn an_unknown_cpu_sample_is_left_out_of_the_mean() {
        let unknown = ProcessInfo {
            cpu_percent: None,
            ..proc(91204, 500, 0.0)
        };
        let history = history()
            .apply(&sample(vec![unknown]))
            .apply(&sample(vec![proc(91204, 500, 10.0)]))
            .apply(&sample(vec![proc(91204, 500, 20.0)]));
        assert_eq!(history.cpu_mean(id(91204)), Some(15.0));
    }

    #[test]
    fn a_process_with_no_known_cpu_orders_as_zero() {
        let unknown = |pid| ProcessInfo {
            cpu_percent: None,
            ..proc(pid, 91190, 0.0)
        };
        let samples = [
            sample(vec![claude(91190), unknown(91201), proc(91202, 91190, 0.0)]),
            sample(vec![claude(91190), unknown(91201), proc(91202, 91190, 0.0)]),
        ];
        let history = samples.iter().fold(history(), History::apply);
        assert_eq!(history.cpu_mean(id(91201)), Some(0.0));
        // The one-sample order puts an unknown CPU last; the window orders it as 0.0,
        // which ties with 91202 and so comes first by PID.
        let order = ordered(&samples, SortKey::Cpu);
        assert_eq!(child_pids(&order, 0), vec![91201, 91202]);
    }

    #[test]
    fn a_window_of_two_keeps_two_samples() {
        let tuning = OrderTuning { window: 2 };
        let history = [1.0, 2.0, 3.0]
            .into_iter()
            .map(|cpu| sample(vec![proc(91204, 500, cpu)]))
            .fold(History::new(tuning), |history, sample| {
                history.apply(&sample)
            });
        assert_eq!(cpu_readings(&history, 91204), vec![Some(2.0), Some(3.0)]);
    }

    #[test]
    fn a_window_of_zero_still_keeps_the_latest_sample() {
        let history = History::new(OrderTuning { window: 0 })
            .apply(&sample(vec![proc(91204, 500, 1.0)]))
            .apply(&sample(vec![proc(91204, 500, 4.0)]));
        assert_eq!(cpu_readings(&history, 91204), vec![Some(4.0)]);
    }

    #[test]
    fn the_history_also_keeps_the_resident_memory() {
        let history = [10, 20, 60]
            .into_iter()
            .map(|rss| sample(vec![with_rss(proc(91204, 500, 0.0), rss)]))
            .fold(history(), |history, sample| history.apply(&sample));
        let rss: Vec<u64> = history
            .readings(id(91204))
            .iter()
            .map(|reading| reading.rss_bytes)
            .collect();
        assert_eq!(rss, vec![10, 20, 60]);
        assert_eq!(history.rss_mean(id(91204)), Some(30));
        assert_eq!(history.rss_mean(id(91230)), None);
    }

    // Order

    #[test]
    fn cpu_order_uses_the_three_sample_mean_not_the_latest() {
        let at = |a: f64, b: f64, other: f64| {
            sample(vec![
                claude(91190),
                proc(91204, 91190, a),
                proc(91230, 91190, b),
                claude(92000),
                proc(92001, 92000, other),
            ])
        };
        let samples = [
            at(10.0, 90.0, 0.0),
            at(20.0, 90.0, 0.0),
            at(150.0, 30.0, 200.0),
        ];
        // The latest sample alone puts 92000 (200.0) above 91190 (180.0), and 91204
        // above 91230.
        let latest = classified(&samples[2], SortKey::Cpu);
        assert_eq!(group_pids(&latest), vec![92000, 91190]);

        let order = ordered(&samples, SortKey::Cpu);
        assert_eq!(group_pids(&order), vec![91190, 92000]);
        assert_eq!(child_pids(&order, 0), vec![91230, 91204]);
        let shown: Vec<Option<f64>> = order.groups[0]
            .root
            .children
            .iter()
            .map(|node| node.process.cpu_percent)
            .collect();
        assert_eq!(shown, vec![Some(30.0), Some(150.0)]);
    }

    #[test]
    fn memory_order_uses_the_mean_of_the_last_three_samples() {
        let at = |a: u64, b: u64, other: u64| {
            sample(vec![
                claude(91190),
                with_rss(proc(91204, 91190, 0.0), a * MIB),
                with_rss(proc(91230, 91190, 0.0), b * MIB),
                claude(92000),
                with_rss(proc(92001, 92000, 0.0), other * MIB),
            ])
        };
        let samples = [at(100, 50, 10), at(100, 50, 10), at(10, 60, 90)];
        let latest = classified(&samples[2], SortKey::Memory);
        assert_eq!(group_pids(&latest), vec![92000, 91190]);
        assert_eq!(child_pids(&latest, 1), vec![91230, 91204]);

        let order = ordered(&samples, SortKey::Memory);
        // 91190: 70M + 53.3M; 92000: 36.7M.
        assert_eq!(group_pids(&order), vec![91190, 92000]);
        assert_eq!(child_pids(&order, 0), vec![91204, 91230]);
        let shown: Vec<u64> = order.groups[0]
            .root
            .children
            .iter()
            .map(|node| node.process.rss_bytes)
            .collect();
        assert_eq!(shown, vec![10 * MIB, 60 * MIB]);
    }

    #[test]
    fn memory_order_sums_each_member_s_own_mean() {
        // 91190 holds 91204 at 90M throughout. 92000 holds 92001 at 30M throughout and
        // 92002, which appeared in the last sample at 80M. Summing each member's mean
        // gives 110M for 92000; the mean of its tree totals would give 56.7M.
        let early = || {
            vec![
                claude(91190),
                with_rss(proc(91204, 91190, 0.0), 90 * MIB),
                claude(92000),
                with_rss(proc(92001, 92000, 0.0), 30 * MIB),
            ]
        };
        let mut last = early();
        last.push(with_rss(proc(92002, 92000, 0.0), 80 * MIB));
        let samples = [sample(early()), sample(early()), sample(last)];
        let order = ordered(&samples, SortKey::Memory);
        assert_eq!(group_pids(&order), vec![92000, 91190]);
    }

    #[test]
    fn equal_totals_keep_the_one_sample_tie_break() {
        let at = || {
            sample(vec![
                claude(92000),
                proc(92001, 92000, 5.0),
                claude(91190),
                proc(91240, 91190, 2.5),
                proc(91235, 91190, 2.5),
            ])
        };
        let order = ordered(&[at(), at(), at()], SortKey::Cpu);
        assert_eq!(group_pids(&order), vec![91190, 92000]);
        assert_eq!(child_pids(&order, 0), vec![91235, 91240]);
    }

    #[test]
    fn a_process_missing_from_the_history_orders_by_its_latest_sample() {
        let latest = sample(vec![
            claude(91190),
            proc(91204, 91190, 5.0),
            proc(91230, 91190, 9.0),
        ]);
        let order = order_by_window(classified(&latest, SortKey::Cpu), &history(), SortKey::Cpu);
        assert_eq!(child_pids(&order, 0), vec![91230, 91204]);
    }

    // Folding

    #[test]
    fn a_group_with_five_subtrees_shows_three_and_folds_the_rest() {
        let order = ordered(&[five_subtrees(false)], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![false]),
                Seen::Process(91211, vec![false, true]),
                Seen::Process(91202, vec![false]),
                Seen::Process(91203, vec![false]),
                Seen::More(3, Some(2.5), 150 * MIB),
            ]
        );
    }

    #[test]
    fn a_group_with_three_subtrees_has_no_fold_row() {
        let three = sample(vec![
            claude(91190),
            proc(91201, 91190, 3.0),
            proc(91202, 91190, 2.0),
            proc(91203, 91190, 1.0),
        ]);
        let order = ordered(&[three], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![false]),
                Seen::Process(91202, vec![false]),
                Seen::Process(91203, vec![true]),
            ]
        );
    }

    #[test]
    fn a_group_with_four_subtrees_folds_the_fourth() {
        let four = sample(vec![
            claude(91190),
            proc(91201, 91190, 4.0),
            proc(91202, 91190, 3.0),
            proc(91203, 91190, 2.0),
            with_rss(proc(91204, 91190, 1.0), 7 * MIB),
            with_rss(proc(91214, 91204, 0.5), MIB),
        ]);
        let order = ordered(&[four], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![false]),
                Seen::Process(91202, vec![false]),
                Seen::Process(91203, vec![false]),
                Seen::More(2, Some(1.5), 8 * MIB),
            ]
        );
    }

    #[test]
    fn an_empty_group_emits_only_its_group_row() {
        let order = ordered(&[sample(vec![claude(91190)])], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![Seen::Group(91190, FoldState::Folded)]
        );
    }

    #[test]
    fn no_groups_give_no_rows() {
        let order = ordered(&[sample(Vec::new())], SortKey::Cpu);
        assert_eq!(rows_of(&order, &Folds::default()), Vec::new());
    }

    #[test]
    fn expanding_shows_every_subtree() {
        let order = ordered(&[five_subtrees(true)], SortKey::Cpu);
        let expanded = vec![
            Seen::Group(91190, FoldState::Expanded),
            Seen::Process(91201, vec![false]),
            Seen::Process(91211, vec![false, true]),
            Seen::Process(91202, vec![false]),
            Seen::Process(91203, vec![false]),
            Seen::Process(91204, vec![false]),
            Seen::Process(71002, vec![false, true]),
            Seen::Process(91205, vec![true]),
        ];
        let by_group = Folds::default().expand(group_91190());
        assert_eq!(rows_of(&order, &by_group), expanded);
        let by_fold_row = Folds::default().expand(RowId::More(id(91190)));
        assert_eq!(rows_of(&order, &by_fold_row), expanded);
        let after_collapse = Folds::default()
            .collapse(group_91190())
            .expand(group_91190());
        assert_eq!(rows_of(&order, &after_collapse), expanded);
    }

    #[test]
    fn collapsing_shows_only_the_group_row() {
        let order = ordered(&[five_subtrees(false)], SortKey::Cpu);
        let folds = Folds::default().collapse(group_91190());
        assert_eq!(folds.state(id(91190)), FoldState::Collapsed);
        assert_eq!(
            rows_of(&order, &folds),
            vec![Seen::Group(91190, FoldState::Collapsed)]
        );
    }

    #[test]
    fn collapsing_a_fold_row_collapses_its_group() {
        let folds = Folds::default().collapse(RowId::More(id(91190)));
        assert_eq!(folds.state(id(91190)), FoldState::Collapsed);
    }

    #[test]
    fn expanding_or_collapsing_a_process_row_changes_no_fold_state() {
        let folds = Folds::default().expand(group_91190());
        assert_eq!(folds.clone().expand(RowId::Process(id(91201))), folds);
        assert_eq!(folds.clone().collapse(RowId::Process(id(91201))), folds);
    }

    #[test]
    fn a_paused_process_inside_a_fold_is_lifted_and_not_counted() {
        let order = ordered(&[five_subtrees(true)], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![false]),
                Seen::Process(91211, vec![false, true]),
                Seen::Process(91202, vec![false]),
                Seen::Process(91203, vec![false]),
                Seen::More(2, Some(2.5), 128 * MIB),
                Seen::Lifted(71002),
            ]
        );
    }

    #[test]
    fn a_paused_process_inside_a_collapsed_group_is_lifted() {
        let order = ordered(&[five_subtrees(true)], SortKey::Cpu);
        let folds = Folds::default().collapse(group_91190());
        assert_eq!(
            rows_of(&order, &folds),
            vec![
                Seen::Group(91190, FoldState::Collapsed),
                Seen::Lifted(71002),
            ]
        );
    }

    #[test]
    fn a_shown_paused_process_is_not_lifted() {
        let shown_paused = sample(vec![claude(91190), stopped(proc(91201, 91190, 3.0))]);
        let order = ordered(&[shown_paused], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![true]),
            ]
        );
    }

    #[test]
    fn every_folded_process_paused_emits_no_fold_row() {
        let all_paused = sample(vec![
            claude(91190),
            proc(91201, 91190, 4.0),
            proc(91202, 91190, 3.0),
            proc(91203, 91190, 2.0),
            stopped(proc(91204, 91190, 1.0)),
            stopped(proc(91214, 91204, 0.0)),
        ]);
        let order = ordered(&[all_paused], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![false]),
                Seen::Process(91202, vec![false]),
                Seen::Process(91203, vec![true]),
                Seen::Lifted(91204),
                Seen::Lifted(91214),
            ]
        );
    }

    #[test]
    fn one_subtree_per_group_folds_the_second() {
        let two = sample(vec![
            claude(91190),
            proc(91201, 91190, 4.0),
            with_rss(proc(91202, 91190, 3.0), MIB),
        ]);
        let order = ordered(&[two], SortKey::Cpu);
        let tuning = FoldTuning {
            subtrees_per_group: 1,
        };
        assert_eq!(
            seen(&rows(&order, &Folds::default(), tuning)),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![false]),
                Seen::More(1, Some(3.0), MIB),
            ]
        );
    }

    #[test]
    fn a_leftover_with_four_children_under_its_top_does_not_fold() {
        let leftover = sample(vec![
            leftover_top(52143, 0.0),
            proc(52151, 52143, 4.0),
            proc(52152, 52143, 3.0),
            proc(52153, 52143, 2.0),
            proc(52154, 52143, 1.0),
        ]);
        let order = ordered(&[leftover], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(52143, FoldState::Folded),
                Seen::Process(52143, vec![true]),
                Seen::Process(52151, vec![true, false]),
                Seen::Process(52152, vec![true, false]),
                Seen::Process(52153, vec![true, false]),
                Seen::Process(52154, vec![true, true]),
            ]
        );
    }

    #[test]
    fn a_paused_process_with_a_running_child_lifts_only_itself() {
        let paused_parent = sample(vec![
            claude(91190),
            proc(91201, 91190, 4.0),
            proc(91202, 91190, 3.0),
            proc(91203, 91190, 2.0),
            stopped(with_rss(proc(91204, 91190, 0.0), MIB)),
            with_rss(proc(91214, 91204, 1.0), 2 * MIB),
        ]);
        let order = ordered(&[paused_parent], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(91190, FoldState::Folded),
                Seen::Process(91201, vec![false]),
                Seen::Process(91202, vec![false]),
                Seen::Process(91203, vec![false]),
                Seen::More(1, Some(1.0), 2 * MIB),
                Seen::Lifted(91204),
            ]
        );
    }

    #[test]
    fn a_fold_row_of_unknown_cpu_has_no_cpu_total() {
        let unknown = |pid| ProcessInfo {
            cpu_percent: None,
            ..with_rss(proc(pid, 91190, 0.0), MIB)
        };
        let first_refresh = sample(vec![
            claude(91190),
            unknown(91201),
            unknown(91202),
            unknown(91203),
            unknown(91204),
        ]);
        let order = ordered(&[first_refresh], SortKey::Cpu);
        let all = rows_of(&order, &Folds::default());
        assert_eq!(all.last(), Some(&Seen::More(1, None, MIB)));
    }

    #[test]
    fn a_process_listed_twice_in_one_sample_is_read_once() {
        let history = history().apply(&sample(vec![
            proc(91204, 500, 10.0),
            proc(91204, 500, 99.0),
        ]));
        assert_eq!(cpu_readings(&history, 91204), vec![Some(10.0)]);
    }

    #[test]
    fn a_leftover_group_shows_its_top_process_as_its_one_subtree() {
        let leftover = sample(vec![leftover_top(52143, 0.0), proc(52150, 52143, 96.3)]);
        let order = ordered(&[leftover], SortKey::Cpu);
        assert_eq!(
            rows_of(&order, &Folds::default()),
            vec![
                Seen::Group(52143, FoldState::Folded),
                Seen::Process(52143, vec![true]),
                Seen::Process(52150, vec![true, true]),
            ]
        );
        let ids: Vec<RowId> = rows(&order, &Folds::default(), fold_tuning())
            .iter()
            .map(Row::id)
            .collect();
        assert_eq!(
            ids,
            vec![
                RowId::Group(id(52143)),
                RowId::Process(id(52143)),
                RowId::Process(id(52150)),
            ]
        );
    }

    #[test]
    fn every_row_carries_its_identity() {
        let order = ordered(&[five_subtrees(true)], SortKey::Cpu);
        let ids: Vec<RowId> = rows(&order, &Folds::default(), fold_tuning())
            .iter()
            .map(Row::id)
            .collect();
        assert_eq!(
            ids,
            vec![
                RowId::Group(id(91190)),
                RowId::Process(id(91201)),
                RowId::Process(id(91211)),
                RowId::Process(id(91202)),
                RowId::Process(id(91203)),
                RowId::More(id(91190)),
                RowId::Process(id(71002)),
            ]
        );
    }

    #[test]
    fn fold_state_of_a_vanished_group_is_dropped() {
        let folds = Folds::default().expand(group_91190());
        let without = classified(&sample(vec![claude(92000)]), SortKey::Cpu);
        let kept = folds.retain_present(&without);
        assert_eq!(kept.state(id(91190)), FoldState::Folded);
        assert_eq!(kept, Folds::default());
    }

    #[test]
    fn fold_state_survives_a_refresh() {
        let folds = Folds::default().expand(group_91190());
        let refreshed = ordered(&[five_subtrees(false), five_subtrees(false)], SortKey::Cpu);
        let kept = folds.retain_present(&refreshed);
        assert_eq!(kept.state(id(91190)), FoldState::Expanded);
        assert_eq!(rows_of(&refreshed, &kept).len(), 8);
    }
}
