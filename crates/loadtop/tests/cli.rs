//! The command-line contract, against the built `loadtop` binary with `HOME` pointed at a
//! temporary directory (and the XDG variables unset), so nothing here touches the real
//! data or log directories: data on stdout,
//! diagnostics on stderr; exit 0 on success, 1 on a runtime error, 2 on a usage error.

use std::fs;
use std::io;
use std::path::Path;
use std::process::{Command, Output, Stdio};

#[cfg(target_os = "macos")]
const LOG_DIR: &str = "Library/Logs/io.github.tomada1114.loadtop";
#[cfg(not(target_os = "macos"))]
const LOG_DIR: &str = ".local/state/loadtop/logs";

const HOME_MISSING: &str = "HOME is not set, so Claude Code's session records cannot be found";

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
        Some("Top for Claude Code sessions")
    );
}

#[test]
fn help_lists_the_three_flags() {
    let home = tempfile::tempdir().unwrap();
    let help = stdout(&run(home.path(), &["--help"]));
    for flag in ["--once", "--json", "--interval <SECONDS>"] {
        assert!(help.contains(flag), "{flag}: {help}");
    }
}

#[test]
fn once_prints_the_header_and_exits_zero() {
    let home = tempfile::tempdir().unwrap();
    let output = run(home.path(), &["--once"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let text = stdout(&output);
    assert!(text.starts_with("CPU Claude "), "{text}");
    assert!(text.ends_with('\n'), "{text}");
    assert!(!stderr(&output).contains("error:"), "{}", stderr(&output));
}

#[test]
fn json_prints_one_document_with_schema_version_one() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        &["--json"][..],
        &["--json", "--once"],
        &["--once", "--json"],
    ] {
        let output = run(home.path(), args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}",
            stderr(&output)
        );
        let text = stdout(&output);
        assert!(
            text.starts_with(r#"{"schema_version":1,"#),
            "{args:?}: {text}"
        );
        assert!(text.ends_with("}\n"), "{args:?}: {text}");
        assert_eq!(text.lines().count(), 1, "{args:?}: one line");
        let document: serde_json::Value = serde_json::from_str(&text).unwrap();
        let keys: Vec<&str> = document
            .as_object()
            .unwrap()
            .keys()
            .map(String::as_str)
            .collect();
        let mut expected = [
            "schema_version",
            "sampled_at_ms",
            "cpu_window_ms",
            "machine",
            "claude",
            "sessions",
            "leftovers",
        ];
        expected.sort_unstable();
        let mut keys = keys;
        keys.sort_unstable();
        assert_eq!(keys, expected, "{args:?}");
        assert_eq!(document["schema_version"], 1, "{args:?}");
        assert_eq!(document["cpu_window_ms"], 300, "{args:?}");
        assert!(document["sessions"].is_array(), "{args:?}");
        assert!(document["leftovers"].is_array(), "{args:?}");
    }
}

#[test]
fn interval_outside_one_to_ten_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    for value in ["0", "11", "two"] {
        let output = run(home.path(), &["--interval", value]);
        assert_eq!(
            output.status.code(),
            Some(2),
            "{value}: {}",
            stderr(&output)
        );
        assert_eq!(stdout(&output), "", "{value}");
        assert!(
            stderr(&output).contains("--interval"),
            "{value}: {}",
            stderr(&output)
        );
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0, "{value}");
    }
}

#[test]
fn interval_with_once_or_json_is_a_usage_error() {
    let home = tempfile::tempdir().unwrap();
    for args in [
        &["--once", "--interval", "3"][..],
        &["--json", "--interval", "3"],
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
            stderr(&output).contains("cannot be used with"),
            "{args:?}: {}",
            stderr(&output)
        );
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0, "{args:?}");
    }
}

#[test]
fn interval_alone_still_needs_a_terminal() {
    let home = tempfile::tempdir().unwrap();
    for value in ["1", "3", "10"] {
        let mut command = command(home.path(), &["--interval", value]);
        command.stdin(Stdio::null());
        assert_runtime_error(
            &output(command),
            "the screen needs an interactive terminal on standard input and standard output; use --once or --json in a script",
        );
        assert_eq!(fs::read_dir(home.path()).unwrap().count(), 0, "{value}");
    }
}

#[test]
fn a_missing_home_fails_with_exit_code_1() {
    for args in [&["--once"][..], &["--json"]] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_loadtop"));
        command.args(args).env_remove("HOME");
        assert_runtime_error(&output(command), HOME_MISSING);
    }
}

#[test]
fn an_empty_home_fails_with_exit_code_1() {
    for args in [&["--once"][..], &["--json"]] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_loadtop"));
        command.args(args).env("HOME", "");
        assert_runtime_error(&output(command), HOME_MISSING);
    }
}

#[test]
fn a_closed_stdout_pipe_is_a_runtime_error_not_a_panic() {
    let home = tempfile::tempdir().unwrap();
    // The read end is dropped before the child starts, so its first write fails with
    // EPIPE deterministically, as in `loadtop --once | true` once `true` exited.
    let (reader, writer) = io::pipe().unwrap();
    drop(reader);
    let mut command = command(home.path(), &["--once"]);
    command.stdout(Stdio::from(writer));
    assert_runtime_error(
        &output(command),
        "the result could not be written to standard output",
    );
}

#[test]
fn each_run_logs_to_the_file_just_logs_reads() {
    let home = tempfile::tempdir().unwrap();
    let output = run(home.path(), &["--once"]);
    assert!(output.status.success(), "{}", stderr(&output));
    let names: Vec<String> = fs::read_dir(home.path().join(LOG_DIR))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    // The justfile's `logs` recipe reads `<log_dir>/loadtop.*.log`.
    assert_eq!(names.len(), 1, "{names:?}");
    assert!(names[0].starts_with("loadtop."), "{names:?}");
    assert_eq!(
        Path::new(&names[0]).extension().and_then(|e| e.to_str()),
        Some("log")
    );
}

#[test]
fn an_unwritable_log_directory_does_not_stop_the_action() {
    let home = tempfile::tempdir().unwrap();
    let log_parent = home.path().join(LOG_DIR);
    let log_parent = log_parent.parent().unwrap();
    fs::create_dir_all(log_parent.parent().unwrap()).unwrap();
    fs::write(log_parent, "a file where a directory should be").unwrap();
    let output = run(home.path(), &["--once"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).starts_with("CPU Claude "),
        "{}",
        stdout(&output)
    );
    assert!(
        stderr(&output).contains("warning: logging is unavailable for this run"),
        "{}",
        stderr(&output)
    );
}

#[cfg(not(target_os = "macos"))]
#[test]
fn absolute_xdg_state_home_moves_the_log_directory() {
    let home = tempfile::tempdir().unwrap();
    let xdg = tempfile::tempdir().unwrap();
    let mut command = Command::new(env!("CARGO_BIN_EXE_loadtop"));
    command
        .args(["--once"])
        .env("HOME", home.path())
        .env_remove("XDG_DATA_HOME")
        .env("XDG_STATE_HOME", xdg.path().join("state"));
    assert!(output(command).status.success());
    assert!(xdg.path().join("state/loadtop/logs").is_dir());
    assert!(!home.path().join(LOG_DIR).exists());
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
