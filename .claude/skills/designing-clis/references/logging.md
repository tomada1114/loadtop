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
- While the screen owns the terminal it logs to the file only: a line on stdout or
  stderr would corrupt the frame (`building-tuis`).
- Logging is best effort. When the directory cannot be used, the binary prints a
  `warning:` line on stderr and still runs the action.
- No log line carries user data, and none carries a model prompt, an answer, a
  credential, or a provider's error body (`designing-errors`).

## What loadtop logs

- **Every signal it sends, one line each, at `info`:** the PID, the process start time,
  the process name (the executable's name only), the signal, and the outcome (`sent`,
  `gone`, `permission`). A user asking "what did loadtop kill?" after the fact has only
  this file to read, since the tool keeps no other state.
- **Never a command line, a working directory, or an environment value.** A command
  line can hold a secret passed as an argument, and a cwd names a private project; the
  screen shows them (as `ps` does), but a log file outlives the session and gets pasted
  into bug reports. The session ID is not logged either.
- **A failed sample at `error`**, by its error code only, just before the run ends
  (it is fatal, ux-guidelines › States); **ports becoming unavailable or available
  again at `warn`**, once per change, not once per failed read; and the start and end
  of a run at `info` with the version and the interval. Nothing per refresh: at 2 s that
  would be 43,200 lines a day of nothing.
- `just logs` prints the newest file's last lines and exits; `just logs-follow` follows
  it and never ends, so it is a human's recipe.
