//! The command-line contract, against the built `loadtop` binary with `HOME` pointed at a
//! temporary directory (and the XDG variables unset), so nothing here touches the real
//! data or log directories: data on stdout,
//! diagnostics on stderr; exit 0 on success, 1 on a runtime error, 2 on a usage error.

use std::fs;
use std::path::Path;
use std::process::{Command, Output, Stdio};

fn command(home: &Path, args: &[&str]) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_loadtop"));
    command
        .args(args)
        .current_dir(home)
        .env("HOME", home)
        .env_remove("XDG_DATA_HOME")
        .env_remove("XDG_STATE_HOME");
    command
}

fn output(mut command: Command) -> Output {
    match command.output() {
        Ok(output) => output,
        Err(error) => panic!("loadtop did not start: {error}"),
    }
}

fn run(home: &Path, args: &[&str]) -> Output {
    output(command(home, args))
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// Debug builds also echo log lines to stderr, so the diagnostic is the last line.
fn last_stderr_line(output: &Output) -> Option<String> {
    stderr(output).lines().last().map(str::to_owned)
}

/// A runtime error: exit 1, nothing on stdout, and `error: <wording>` last on stderr.
fn assert_runtime_error(output: &Output, wording: &str) {
    assert_eq!(output.status.code(), Some(1), "{}", stderr(output));
    assert_eq!(stdout(output), "", "a failure prints no data");
    assert_eq!(last_stderr_line(output), Some(format!("error: {wording}")));
}

#[test]
fn bare_loadtop_without_a_terminal_refuses_and_touches_nothing() {
    let home = tempfile::tempdir().unwrap();
    // Neither end is a terminal, as in a script or CI: the screen must refuse before it
    // enters raw mode, reads a key, or opens the counter file or the log.
    let mut command = command(home.path(), &[]);
    command.stdin(Stdio::null());
    assert_runtime_error(
        &output(command),
        "the screen needs an interactive terminal on standard input and standard output; use --once or --json in a script",
    );
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}

#[test]
fn help_lists_no_subcommand() {
    let home = tempfile::tempdir().unwrap();
    let output = run(home.path(), &["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert!(
        !stdout(&output).contains("Commands:"),
        "{}",
        stdout(&output)
    );
    assert_eq!(stderr(&output), "");
}

#[test]
fn help_opens_with_the_tools_about_line() {
    let home = tempfile::tempdir().unwrap();
    let output = run(home.path(), &["--help"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output).lines().next(),
        Some("Read and change the counter")
    );
}

#[test]
fn version_prints_the_workspace_version_on_stdout() {
    let home = tempfile::tempdir().unwrap();
    let output = run(home.path(), &["--version"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(
        stdout(&output),
        format!("loadtop {}\n", env!("CARGO_PKG_VERSION"))
    );
    assert_eq!(stderr(&output), "");
}

#[test]
fn an_unknown_subcommand_is_a_usage_error_with_exit_code_2() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        &["counter", "show"][..],
        &["tui"],
        &["explode"],
        &["llm", "ask", "Explain Rust"],
    ] {
        let output = run(home.path(), args);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{args:?}: {}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), "", "{args:?}");
        assert!(
            stderr(&output).contains("Usage:"),
            "{args:?}: {}",
            stderr(&output)
        );
        assert_eq!(
            std::fs::read_dir(home.path()).unwrap().count(),
            0,
            "{args:?} wrote under HOME"
        );
    }
}

#[test]
fn an_unknown_flag_is_a_usage_error_with_exit_code_2() {
    let home = tempfile::tempdir().unwrap();
    let output = run(home.path(), &["--loud"]);
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(stdout(&output), "");
    assert!(stderr(&output).starts_with("error:"), "{}", stderr(&output));
}

#[test]
fn a_usage_error_touches_nothing() {
    let home = tempfile::tempdir().unwrap();
    assert_eq!(run(home.path(), &["explode"]).status.code(), Some(2));
    assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0);
}
