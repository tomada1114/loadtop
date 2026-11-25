//! `loadtop`: the command-line tool, and the composition root that wires the real adapters
//! from `loadtop-platform` into `loadtop-core`.
//!
//! The contract (`AGENTS.md` › Architecture): data on stdout, diagnostics on stderr; exit 0
//! on success, 1 when the action failed (no terminal for the screen, no home directory,
//! the process table unreadable, stdout unwritable, the terminal failing), and 2 on a
//! usage error (clap's own code).

// A `match` on a core enum names each variant, so a new one is a compile error in
// `wording` until it has its words.
#![deny(clippy::wildcard_enum_match_arm)]

mod once;
mod tui;
mod wording;

use std::io::{self, IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use loadtop_core::{
    AppTuning, CounterService, JsonReport, ProcessTable, ProcessTableError, Sample, SamplePorts,
    SortKey, Tuning, classify,
};
use loadtop_platform::{
    JsonFileCounterStore, LSOF_PATH, LsofSocketTable, ProjectFoldersDir, SessionRecordsDir,
    SysinfoProcessTable, SystemClock, claude_projects_dir, claude_sessions_dir, counter_file,
    home_dir, init_logging, log_dir,
};

/// Top for Claude Code sessions.
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {
    /// Print one sample as indented plain text and exit.
    #[arg(long)]
    once: bool,
    /// Print one sample as one JSON document and exit (implies --once).
    #[arg(long)]
    json: bool,
    /// Seconds between refreshes of the screen, from 1 to 10.
    #[arg(
        long,
        value_name = "SECONDS",
        default_value_t = 2,
        value_parser = clap::value_parser!(u64)
            .range(AppTuning::MIN_INTERVAL_SECS..=AppTuning::MAX_INTERVAL_SECS),
        conflicts_with_all = ["once", "json"],
    )]
    interval: u64,
}

/// What one sample is printed as.
#[derive(Debug, Clone, Copy)]
enum Format {
    Text,
    Json,
}

fn main() -> ExitCode {
    let cli = Cli::parse();
    let tuning = match AppTuning::with_interval_secs(cli.interval) {
        Ok(tuning) => tuning,
        Err(error) => {
            eprintln!("error: {}", wording::app_tuning_error(error));
            return ExitCode::from(2);
        }
    };
    match (cli.json, cli.once) {
        (true, _) => one_sample(Format::Json, &tuning),
        (false, true) => one_sample(Format::Text, &tuning),
        (false, false) => screen(),
    }
}

/// `HOME`, with logging started; `None` once the reason has been printed. `echo_logs`
/// copies log lines to stderr in a debug build, which only a run that does not own the
/// screen may do.
fn start(echo_logs: bool) -> Option<PathBuf> {
    let Some(home) = home_dir() else {
        eprintln!("error: {}", wording::HOME_MISSING);
        return None;
    };
    // Logging is best effort: a read-only log directory must not stop the action.
    if init_logging(
        &log_dir(&home),
        env!("CARGO_PKG_NAME"),
        echo_logs && cfg!(debug_assertions),
    )
    .is_err()
    {
        eprintln!("warning: {}", wording::LOGGING_UNAVAILABLE);
    }
    Some(home)
}

/// `--once` and `--json`: one sample, printed, and nothing signalled. This path builds
/// no signal sender.
fn one_sample(format: Format, tuning: &AppTuning) -> ExitCode {
    let Some(home) = start(true) else {
        return ExitCode::FAILURE;
    };
    let sample = match first_sample(&home, tuning) {
        Ok(sample) => sample,
        Err(error) => {
            eprintln!("error: {}", wording::process_table_error(error));
            return ExitCode::FAILURE;
        }
    };
    let own = sample.identity_of(std::process::id());
    let classification = classify(&sample, own, &home, SortKey::Cpu);
    // `println!` would panic (an abort in release) when stdout is closed.
    let mut stdout = io::stdout().lock();
    let written = match format {
        Format::Text => stdout.write_all(once::render(&classification, &home).as_bytes()),
        Format::Json => {
            let report = JsonReport::new(
                &classification,
                sample.taken_at,
                tuning.sampling.first_sample_gap,
                &home,
            );
            serde_json::to_writer(&mut stdout, &report)
                .map_err(io::Error::from)
                .and_then(|()| stdout.write_all(b"\n"))
        }
    };
    if written.and_then(|()| stdout.flush()).is_err() {
        eprintln!("error: {}", wording::STDOUT_UNAVAILABLE);
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

/// The first sample over the real adapters under `home`: two process-table reads the
/// first-sample gap apart, so CPU% is measured over that window.
fn first_sample(home: &Path, tuning: &AppTuning) -> Result<Sample, ProcessTableError> {
    let processes = SysinfoProcessTable::new()?;
    let sockets = LsofSocketTable::new(PathBuf::from(LSOF_PATH), processes.user_id());
    let records = SessionRecordsDir::new(claude_sessions_dir(home));
    let folders = ProjectFoldersDir::new(claude_projects_dir(home));
    processes.sample()?;
    std::thread::sleep(tuning.sampling.first_sample_gap);
    Sample::read(SamplePorts {
        processes: &processes,
        sockets: &sockets,
        records: &records,
        folders: &folders,
        clock: &SystemClock,
    })
}

/// Bare `loadtop`: the full-screen view.
fn screen() -> ExitCode {
    // Checked before anything else is touched: without a terminal on both ends there is
    // no screen to draw and no key to read, and waiting for one would hang a script.
    if !(io::stdin().is_terminal() && io::stdout().is_terminal()) {
        eprintln!("error: {}", wording::TERMINAL_MISSING);
        return ExitCode::FAILURE;
    }
    // Nothing may write to the terminal while the screen owns it, so logs go to the file only.
    let Some(home) = start(false) else {
        return ExitCode::FAILURE;
    };
    let service = CounterService::new(
        Arc::new(JsonFileCounterStore::new(counter_file(&home))),
        Arc::new(SystemClock),
        Tuning::default(),
    );
    match tui::run(&service) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(kind = ?error.kind(), "the terminal failed");
            eprintln!("error: {}", wording::TERMINAL_FAILED);
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use clap::CommandFactory;

    use super::Cli;

    #[test]
    fn the_command_line_declaration_is_consistent() {
        Cli::command().debug_assert();
    }
}
