# Logging

How the tool logs and why. The binary owns it: only the binary installs a subscriber,
in its composition root, so this page belongs to `designing-clis`. What a log line may
carry is `designing-errors`; the log file's format, which is contract, is in
`integrating-system-apis`' `references/file-formats.md`; reading the logs while a change
runs is `running-the-app`.

## The shape

- The binary and `loadtop-platform` log through the `tracing` macros; `loadtop-core` has no
  `tracing` dependency and logs nothing. `tracing` gives one logging API across every
  crate that logs.
- Only the binary installs a subscriber (`loadtop_platform::init_logging`, which
  `compose` calls).
- Files go to `~/Library/Logs/<bundle id>/` on macOS and `$XDG_STATE_HOME/loadtop/logs/`
  (default `~/.local/state/loadtop/logs/`) on Linux (`loadtop_platform::log_dir`), as
  `loadtop.YYYY-MM-DD.log`, one per day, and the newest 14 are kept. Retention counts
  every file in that directory whose name starts with the writer's prefix, so a second
  writer gets its own directory rather than sharing this one.
- The writer is synchronous: the volume is low, and an early `process::exit` would
  drop a background writer's last lines.
- A debug build of a plain subcommand also copies each line to stderr (`compose(true)`);
  a release build writes the file only.
- While `loadtop tui` owns the terminal it logs to the file only: a line on stdout or
  stderr would corrupt the frame (`building-tuis`).
- Logging is best effort. When the directory cannot be used, the binary prints a
  `warning:` line on stderr and still runs the action.
- No log line carries user data, and none carries a model prompt, an answer, a
  credential, or a provider's error body (`designing-errors`).
- `just logs` prints the newest file's last lines and exits; `just logs-follow` follows
  it and never ends, so it is a human's recipe.
