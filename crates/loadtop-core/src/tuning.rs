//! loadtop's tunable numbers, in one place (`designing-core-logic` › "One `Tuning`").
//!
//! Named `AppTuning` because `Tuning` is still the template counter's.

use std::time::Duration;

/// Every number someone might want to tweak, one nested struct per feature. The binary
/// builds it and passes it in; [`Default`] holds the shipped values.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AppTuning {
    /// How often and how the ports are read.
    pub sampling: SamplingTuning,
    /// How rows keep their order.
    pub order: OrderTuning,
    /// How many subtrees a group shows before it folds.
    pub fold: FoldTuning,
    /// The per-row load trend.
    pub trend: TrendTuning,
    /// Stopping a tree.
    pub stop: StopTuning,
    /// When a load reading is marked as heavy.
    pub heat: HeatTuning,
}

impl AppTuning {
    /// The shortest refresh interval a user may set, in seconds.
    pub const MIN_INTERVAL_SECS: u64 = 1;
    /// The longest refresh interval a user may set, in seconds.
    pub const MAX_INTERVAL_SECS: u64 = 10;

    /// The shipped values with the refresh interval set to `secs` seconds (`--interval`).
    ///
    /// # Errors
    /// [`AppTuningError::IntervalOutOfRange`] when `secs` is outside
    /// [`Self::MIN_INTERVAL_SECS`]..=[`Self::MAX_INTERVAL_SECS`]: below one second loadtop
    /// itself would exceed its load budget, and above ten the view is too stale to act on.
    pub fn with_interval_secs(secs: u64) -> Result<Self, AppTuningError> {
        if !(Self::MIN_INTERVAL_SECS..=Self::MAX_INTERVAL_SECS).contains(&secs) {
            return Err(AppTuningError::IntervalOutOfRange);
        }
        let mut tuning = Self::default();
        tuning.sampling.interval = Duration::from_secs(secs);
        Ok(tuning)
    }
}

impl Default for AppTuning {
    fn default() -> Self {
        Self {
            sampling: SamplingTuning {
                interval: Duration::from_secs(2),
                first_sample_gap: Duration::from_millis(300),
                port_every: 3,
            },
            order: OrderTuning { window: 3 },
            fold: FoldTuning {
                subtrees_per_group: 3,
            },
            trend: TrendTuning { length: 10 },
            stop: StopTuning {
                grace: Duration::from_secs(3),
            },
            heat: HeatTuning {
                cpu_warm_percent: 50,
                cpu_hot_percent: 100,
                memory_warm_bytes: 524_288_000,
                memory_hot_bytes: 1_073_741_824,
            },
        }
    }
}

/// How often and how the ports are read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SamplingTuning {
    /// Time between samples: 2 s by default, so loadtop stays within 2% of one core and
    /// the view is still fresh enough to act on. Private, so only
    /// [`AppTuning::with_interval_secs`] sets it, within 1–10 s.
    interval: Duration,
    /// The gap between the first two refreshes: 300 ms, since CPU% is the change between
    /// two readings and the OS needs at least 200 ms between them, so the first frame
    /// shows CPU without waiting a whole interval.
    pub first_sample_gap: Duration,
    /// The socket table is read every this many samples: 3, since a read costs about
    /// 20 ms of CPU and ports change rarely.
    pub port_every: u32,
}

impl SamplingTuning {
    /// Time between samples, within 1–10 s.
    #[must_use]
    pub const fn interval(&self) -> Duration {
        self.interval
    }
}

/// How rows keep their order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderTuning {
    /// Rows are ordered by load over this many samples: 3, so a row does not jump on
    /// one spike while a person is about to select it.
    pub window: u32,
}

/// How many subtrees a group shows before it folds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FoldTuning {
    /// The heaviest 3 subtrees of a group are shown and the rest fold into one row, so
    /// one busy session cannot push every other off the screen.
    pub subtrees_per_group: usize,
}

/// The per-row load trend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrendTuning {
    /// The trend holds the last 10 samples: 20 s at the default interval, enough to
    /// tell a spike from a steady load, short enough to fit beside a row.
    pub length: usize,
}

/// Stopping a tree.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StopTuning {
    /// SIGKILL follows SIGTERM after 3 s: long enough for a dev server to shut down
    /// cleanly, short enough that the person is still watching.
    pub grace: Duration,
}

/// When a load reading is marked as heavy (ux-guidelines).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeatTuning {
    /// CPU at or above 50% of one core is warm: half a core held is noticeable on a
    /// laptop with little headroom.
    pub cpu_warm_percent: u32,
    /// CPU at or above 100% of one core is hot: a whole core is taken.
    pub cpu_hot_percent: u32,
    /// Memory at or above 500 MiB (524 288 000 bytes) is warm, in binary units as the
    /// view shows memory.
    pub memory_warm_bytes: u64,
    /// Memory at or above 1 GiB (1 073 741 824 bytes) is hot.
    pub memory_hot_bytes: u64,
}

/// An [`AppTuning`] that could not be built.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum AppTuningError {
    /// The refresh interval is outside 1–10 seconds.
    #[error("the refresh interval is outside 1 to 10 seconds")]
    IntervalOutOfRange,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn app_tuning_ships_a_two_second_interval() {
        assert_eq!(
            AppTuning::default().sampling.interval(),
            Duration::from_secs(2)
        );
    }

    #[test]
    fn app_tuning_ships_the_documented_values() {
        let tuning = AppTuning::default();
        assert_eq!(tuning.sampling.first_sample_gap, Duration::from_millis(300));
        assert_eq!(tuning.sampling.port_every, 3);
        assert_eq!(tuning.order.window, 3);
        assert_eq!(tuning.fold.subtrees_per_group, 3);
        assert_eq!(tuning.trend.length, 10);
        assert_eq!(tuning.stop.grace, Duration::from_secs(3));
        assert_eq!(tuning.heat.cpu_warm_percent, 50);
        assert_eq!(tuning.heat.cpu_hot_percent, 100);
        assert_eq!(tuning.heat.memory_warm_bytes, 524_288_000);
        assert_eq!(tuning.heat.memory_hot_bytes, 1_073_741_824);
    }

    #[test]
    fn intervals_of_one_and_ten_seconds_are_accepted() {
        for secs in [1, 10] {
            let tuning = AppTuning::with_interval_secs(secs);
            assert_eq!(
                tuning.map(|tuning| tuning.sampling.interval()),
                Ok(Duration::from_secs(secs)),
                "interval {secs}"
            );
        }
    }

    #[test]
    fn setting_the_interval_keeps_the_other_values() {
        let tuning = AppTuning::with_interval_secs(5).unwrap();
        assert_eq!(
            AppTuning {
                sampling: SamplingTuning {
                    interval: Duration::from_secs(2),
                    ..tuning.sampling
                },
                ..tuning
            },
            AppTuning::default()
        );
    }

    #[test]
    fn an_interval_of_zero_seconds_is_refused() {
        assert_eq!(
            AppTuning::with_interval_secs(0),
            Err(AppTuningError::IntervalOutOfRange)
        );
    }

    #[test]
    fn an_interval_of_eleven_seconds_is_refused() {
        assert_eq!(
            AppTuning::with_interval_secs(11),
            Err(AppTuningError::IntervalOutOfRange)
        );
    }
}
