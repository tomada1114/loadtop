//! `loadtop`: the command-line tool, and the composition root that wires the real adapters
//! from `loadtop-platform` into `loadtop-core`.
//!
//! The contract (`AGENTS.md` › Architecture): data on stdout, diagnostics on stderr; exit 0
//! on success, 1 when the action failed (no terminal for the screen, no home directory, the
//! terminal failing), and 2 on a usage error (clap's own code).

// A `match` on a core enum names each variant, so a new one is a compile error in
// `wording` until it has its words.
#![deny(clippy::wildcard_enum_match_arm)]

mod tui;
mod wording;

use std::io::{self, IsTerminal};
use std::process::ExitCode;
use std::sync::Arc;

use clap::Parser;
use loadtop_core::{CounterService, Tuning};
use loadtop_platform::{
    JsonFileCounterStore, SystemClock, counter_file, home_dir, init_logging, log_dir,
};

/// Read and change the counter.
#[derive(Debug, Parser)]
#[command(version)]
struct Cli {}

fn main() -> ExitCode {
    let Cli {} = Cli::parse();
    screen()
}

/// The service over the real store and clock under `HOME`, with logging started; `None`
/// once the reason has been printed. `echo_logs` copies log lines to stderr in a debug
/// build, which only a command that does not own the screen may do.
fn compose(echo_logs: bool) -> Option<CounterService> {
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
    Some(CounterService::new(
        Arc::new(JsonFileCounterStore::new(counter_file(&home))),
        Arc::new(SystemClock),
        Tuning::default(),
    ))
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
    let Some(service) = compose(false) else {
        return ExitCode::FAILURE;
    };
    match tui::run(&service) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            tracing::error!(kind = ?error.kind(), "the terminal failed");
            eprintln!("error: {}", wording::TERMINAL_FAILED);
            ExitCode::FAILURE
        }
    }
}
