//! Each process's last few readings, and the order they give: a row does not jump on
//! one spike while a person is about to select it (requirements §3.2 Order stability).

use std::cmp::Ordering;
use std::collections::HashMap;

use crate::classify::{Classification, Node, SortKey};
use crate::process_table::{ProcessIdentity, ProcessInfo};
use crate::sample::Sample;
use crate::tuning::OrderTuning;

/// The fewest readings a history keeps, whatever the tuning says: the latest one.
const MIN_WINDOW: usize = 1;

/// What one sample said about one process.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Reading {
    /// Its CPU % in that sample; `None` when it could not be computed.
    pub cpu_percent: Option<f64>,
    /// Its resident memory in that sample, in bytes.
    pub rss_bytes: u64,
}

/// The newest readings of every process in the latest sample, keyed by identity, at
/// most the order window's length each. A process absent from a sample loses its
/// history, and a reused PID with a new start time is a new key, so it starts afresh.
#[derive(Debug, Clone, PartialEq)]
pub struct History {
    window: usize,
    readings: HashMap<ProcessIdentity, Vec<Reading>>,
}

impl History {
    /// An empty history keeping `tuning.window` readings per process (at least one).
    #[must_use]
    pub fn new(tuning: OrderTuning) -> Self {
        Self {
            window: usize::try_from(tuning.window)
                .unwrap_or(usize::MAX)
                .max(MIN_WINDOW),
            readings: HashMap::new(),
        }
    }

    /// The history after `sample`: each process in it gains its reading, the oldest
    /// falling out past the window, and every process not in it is dropped. A process
    /// listed twice is read once, the first time.
    #[must_use]
    pub fn apply(mut self, sample: &Sample) -> Self {
        let mut next = HashMap::with_capacity(sample.processes.processes.len());
        for process in &sample.processes.processes {
            if next.contains_key(&process.identity) {
                continue;
            }
            let mut readings = self.readings.remove(&process.identity).unwrap_or_default();
            readings.push(Reading {
                cpu_percent: process.cpu_percent,
                rss_bytes: process.rss_bytes,
            });
            let excess = readings.len().saturating_sub(self.window);
            readings.drain(..excess);
            next.insert(process.identity, readings);
        }
        Self {
            window: self.window,
            readings: next,
        }
    }

    /// The readings kept for `identity`, oldest first; empty when it has none.
    #[must_use]
    pub fn readings(&self, identity: ProcessIdentity) -> &[Reading] {
        self.readings.get(&identity).map_or(&[], Vec::as_slice)
    }

    /// The mean of the known CPU % readings of `identity`: 0.0 when none is known,
    /// `None` when it has no history.
    #[must_use]
    pub fn cpu_mean(&self, identity: ProcessIdentity) -> Option<f64> {
        let readings = self.readings.get(&identity)?;
        let (sum, count) = readings
            .iter()
            .filter_map(|reading| reading.cpu_percent)
            .fold((0.0, 0_u32), |(sum, count), cpu| {
                (sum + cpu, count.saturating_add(1))
            });
        Some(if count == 0 {
            0.0
        } else {
            sum / f64::from(count)
        })
    }

    /// The mean resident memory of `identity`'s readings, in whole bytes (rounded
    /// down); `None` when it has no history.
    #[must_use]
    pub fn rss_mean(&self, identity: ProcessIdentity) -> Option<u64> {
        let readings = self.readings.get(&identity)?;
        let count = u128::try_from(readings.len()).ok().filter(|&n| n > 0)?;
        let sum: u128 = readings
            .iter()
            .map(|reading| u128::from(reading.rss_bytes))
            .sum();
        Some(u64::try_from(sum / count).unwrap_or(u64::MAX))
    }
}

/// `classification` with its groups and every node's children reordered by the window:
/// heaviest first by the sum of each member's mean over `history` for `sort`, ties by
/// PID ascending (the one-sample tie-break). A process `history` does not hold weighs
/// its latest reading, and an unknown CPU % weighs 0.0. Only the order changes: every
/// value shown is still the latest sample's.
#[must_use]
pub fn order_by_window(
    classification: Classification,
    history: &History,
    sort: SortKey,
) -> Classification {
    let mut weighed: Vec<(Load, crate::classify::Group)> = classification
        .groups
        .into_iter()
        .map(|mut group| (reorder(&mut group.root, history, sort), group))
        .collect();
    weighed.sort_by(|(a_load, a), (b_load, b)| {
        heavier_first(*a_load, &a.root, *b_load, &b.root, sort)
    });
    Classification {
        groups: weighed.into_iter().map(|(_, group)| group).collect(),
        ..classification
    }
}

/// A subtree's windowed load.
#[derive(Debug, Clone, Copy)]
struct Load {
    cpu: f64,
    rss: u64,
}

impl Load {
    fn of(process: &ProcessInfo, history: &History) -> Self {
        Self {
            cpu: history
                .cpu_mean(process.identity)
                .unwrap_or_else(|| process.cpu_percent.unwrap_or_default()),
            rss: history
                .rss_mean(process.identity)
                .unwrap_or(process.rss_bytes),
        }
    }

    fn plus(self, other: Self) -> Self {
        Self {
            cpu: self.cpu + other.cpu,
            rss: self.rss.saturating_add(other.rss),
        }
    }
}

/// Sorts `node`'s children (and theirs) by their windowed load and returns its own.
/// Loads are summed in the children's incoming order, so the same input sums the same.
fn reorder(node: &mut Node, history: &History, sort: SortKey) -> Load {
    let mut weighed: Vec<(Load, Node)> = std::mem::take(&mut node.children)
        .into_iter()
        .map(|mut child| (reorder(&mut child, history, sort), child))
        .collect();
    let load = weighed
        .iter()
        .fold(Load::of(&node.process, history), |sum, (child, _)| {
            sum.plus(*child)
        });
    weighed.sort_by(|(a_load, a), (b_load, b)| heavier_first(*a_load, a, *b_load, b, sort));
    node.children = weighed.into_iter().map(|(_, child)| child).collect();
    load
}

fn heavier_first(a_load: Load, a: &Node, b_load: Load, b: &Node, sort: SortKey) -> Ordering {
    let heavier = match sort {
        SortKey::Cpu => b_load.cpu.total_cmp(&a_load.cpu),
        SortKey::Memory => b_load.rss.cmp(&a_load.rss),
    };
    heavier.then(a.process.identity.pid.cmp(&b.process.identity.pid))
}
