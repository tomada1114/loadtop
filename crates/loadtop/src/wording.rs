//! Every sentence `loadtop` writes to stderr, and the error wording its screen shows.
//! Core reports a failure as a code and never as a sentence, so the wording for each code
//! lives here, in one place, and each `match` names every variant: a new core variant does
//! not compile until it has its words.

use loadtop_core::{AppTuningError, CounterError, ProcessTableError, StorageErrorKind};

/// Printed when `HOME` is unset or empty, so no file location can be derived.
pub const HOME_MISSING: &str = "HOME is not set, so Claude Code's session records cannot be found";

/// Printed when the result cannot be written to stdout (a closed pipe, an unwritable
/// descriptor).
pub const STDOUT_UNAVAILABLE: &str = "the result could not be written to standard output";

/// Printed by `--once` under the header when no session and no leftover was found.
pub const NO_SESSION: &str = "No Claude Code session is running.";

/// Printed when the log directory cannot be used; the action still runs.
pub const LOGGING_UNAVAILABLE: &str = "logging is unavailable for this run";

/// Printed when bare `loadtop` runs without a terminal on standard input and standard
/// output (in a pipe, a script, or a CI job); nothing was touched.
pub const TERMINAL_MISSING: &str = "the screen needs an interactive terminal on standard input and standard output; use --once or --json in a script";

/// Printed after the terminal was restored, when it could not be entered, read, drawn to,
/// or restored; a change made before that is already saved.
pub const TERMINAL_FAILED: &str = "the terminal could not be used";

/// The wording for a process table that could not be read, which ends the run.
pub fn process_table_error(error: ProcessTableError) -> String {
    let reason = match error {
        ProcessTableError::Unreadable => "the system refused or failed to list the processes",
        ProcessTableError::Gone => "a process went away while it was being read",
    };
    format!("can't read the process list: {reason}")
}

/// The wording for a refresh interval core refused; clap checks the same range first,
/// so a person only sees this if the two ever disagree.
pub fn app_tuning_error(error: AppTuningError) -> &'static str {
    match error {
        AppTuningError::IntervalOutOfRange => "the refresh interval must be from 1 to 10 seconds",
    }
}

/// The wording for a failed counter action, on the screen's error line.
pub fn counter_error(error: CounterError) -> &'static str {
    match error {
        CounterError::AtMaximum => "the counter is already at its maximum",
        CounterError::AtMinimum => "the counter is already at its minimum",
        CounterError::Storage { kind } => storage_error(kind),
    }
}

/// The wording for a storage failure behind a counter action.
fn storage_error(kind: StorageErrorKind) -> &'static str {
    match kind {
        StorageErrorKind::Unavailable => "the counter file could not be read or written",
        StorageErrorKind::Corrupt => "the counter file holds data this version cannot read",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn process_table_error_has_words_for_every_variant() {
        assert_eq!(
            process_table_error(ProcessTableError::Unreadable),
            "can't read the process list: the system refused or failed to list the processes"
        );
        assert_eq!(
            process_table_error(ProcessTableError::Gone),
            "can't read the process list: a process went away while it was being read"
        );
    }

    #[test]
    fn an_interval_out_of_range_names_the_range() {
        assert_eq!(
            app_tuning_error(AppTuningError::IntervalOutOfRange),
            "the refresh interval must be from 1 to 10 seconds"
        );
    }

    #[test]
    fn at_maximum_says_the_counter_cannot_go_higher() {
        assert_eq!(
            counter_error(CounterError::AtMaximum),
            "the counter is already at its maximum"
        );
    }

    #[test]
    fn at_minimum_says_the_counter_cannot_go_lower() {
        assert_eq!(
            counter_error(CounterError::AtMinimum),
            "the counter is already at its minimum"
        );
    }

    #[test]
    fn unavailable_storage_says_the_file_could_not_be_used() {
        assert_eq!(
            counter_error(CounterError::Storage {
                kind: StorageErrorKind::Unavailable
            }),
            "the counter file could not be read or written"
        );
    }

    #[test]
    fn corrupt_storage_says_the_file_cannot_be_read_by_this_version() {
        assert_eq!(
            counter_error(CounterError::Storage {
                kind: StorageErrorKind::Corrupt
            }),
            "the counter file holds data this version cannot read"
        );
    }
}
