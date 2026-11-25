# loadtop's process sources

Where each fact loadtop shows comes from on macOS, what each adapter promises, and the
traps met finding out. Decisions about those facts (what is a session, a leftover, a
target) are core's and are not here (`designing-core-logic`). Measurements below are
from the author's MacBook Air (Mac14,2, 8 cores, macOS 27.0 26A428, about 780
processes).

## Contents

- [The ports](#the-ports)
- [The process table: sysinfo](#the-process-table-sysinfo)
- [Listening ports: lsof](#listening-ports-lsof)
- [Claude Code's session record](#claude-codes-session-record)
- [Project folder names](#project-folder-names)
- [Signals](#signals)
- [Permissions and privacy](#permissions-and-privacy)
- [Linux](#linux)

## The ports

Each is a synchronous `Send + Sync` trait in core over core's own types, with an adapter
in `loadtop-platform`, a fake and a `<port>_contract` in `loadtop-test-support`:

| Port | Answers | macOS adapter |
|---|---|---|
| Process table | every same-user process: pid, ppid, start time, name, argv, executable path, cwd, CPU%, RSS, run state, controlling terminal, and the marker variables of its environment | sysinfo |
| Socket table | (pid, TCP port) for every same-user listening socket | `lsof` |
| Session records | the parsed `~/.claude/sessions/<pid>.json` files | `std::fs` |
| Project folders | the folder names under `~/.claude/projects/` | `std::fs::read_dir` |
| Signals | send STOP, CONT, TERM, or KILL to a (pid, start time) | sysinfo |
| Clock | `UnixMillis` | the template's `SystemClock` |

The machine totals in the header (all CPUs, used and total memory) come with the
process table's sample. Everything the adapters return is a value: no sysinfo type
crosses a port.

## The process table: sysinfo

- `sysinfo` 0.40 with `default-features = false, features = ["system"]`, in
  `loadtop-platform` only, because it pulls `objc2-*` crates that the core boundary
  forbids in core (`AGENTS.md` › Architecture). Its safe API reads the process list,
  CPU, RSS, env, cwd, start time and run state with no `unsafe` of ours, so
  `unsafe_code = "forbid"` holds. Signed off by the owner 2026-11-20, with
  `rust-version` raised to 1.95, which 0.40 declares.
- `System::new()` returns a `Result` in 0.40 (read in the 0.40.4 source on this Mac,
  2026-11-20): a failure is the port's "process table unreadable" kind, the one fatal
  error (requirements §3.6).
- **Two `System` values, never shared.** The sampler thread owns the one it refreshes
  each sample; the signal adapter owns a second behind a `Mutex`, used only to refresh
  the PIDs it is about to signal. Sharing one would put the loop thread and the sampler
  on the same lock for the 20 ms a scan takes. The throwaway `System` values below (run
  state, one process's cwd) live for one call and are never shared either.
- The sampler's `System` lives for the run; CPU% is the change since the
  previous refresh, so the first frame takes two refreshes 300 ms apart (sysinfo's
  macOS `MINIMUM_CPU_UPDATE_INTERVAL` is 200 ms, same source).
- **Env and cwd once per process.** `ProcessRefreshKind` asks for env with
  `UpdateKind::OnlyIfNotSet`: a process's environment does not change after `exec`, so
  re-reading it every 2 s for 780 processes is wasted work. cwd can change, and matters
  only for leftover judgement, so it is re-read only for processes whose ppid is 1 and
  for session roots, in a second refresh right after the full one: on macOS every
  refresh, even of a few PIDs, restarts the interval the next CPU% is measured over, so
  `cwd` for one process (the detail overlay) uses a throwaway `System` of its own.
  Measured with that split on 0.40.4: about 20 ms of CPU and 9–10 MB RSS per full scan
  (observed with a prototype, 2026-11-20).
- **Only the marker variables leave the adapter.** It keeps `CLAUDECODE`,
  `CLAUDE_CODE_SESSION_ID`, `CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_PID`, and
  `CLAUDE_CODE_ENTRYPOINT`, and drops every other variable before building the value,
  so no other value can reach a view, a log, or `--json` (requirements §4 Privacy). The
  documented ones are listed at <https://code.claude.com/docs/en/env-vars> (checked
  2026-11-20).
- **Unreadable env is "unknown", not "none".** Apple platform binaries (`/bin/sh`,
  `/bin/zsh`, `/bin/sleep`, `/usr/bin/tail`) return no environment to another process
  (observed on macOS 27.0, 2026-11-20). The adapter reports "unreadable" for an empty
  result, and core treats it as such (requirements §3.3's probable tier).
- **Start time has 1 s resolution** (`Process::start_time`, seconds; equal to `ps
  lstart` in the prototype). PID plus start time is the identity everywhere; a PID
  reused within the same second would be mistaken for the old process; that is a
  known limit, accepted.
- **Stopped state.** `ProcessStatus::Stop` reports a SIGSTOPped process, but on macOS
  0.40.4 only from a `System` that first sees the process in that state: a long-lived
  one keeps the state each process had when it was first refreshed, so it never sees a
  STOP or a CONT (observed with a probe, 2026-11-24). So each sample reads run state
  from a throwaway `System` that refreshes only the same-user PIDs with nothing else
  asked for; a PID missing there, with another start time, or a zombie is left out.
  That read costs about 8–12 ms wall time for 470 same-user processes on top of the
  25–34 ms full refresh (same probe). It is the sleeping mark's only source.
- **No controlling terminal.** sysinfo 0.40 has no accessor for a process's
  controlling terminal (its `Process` API, read in the 0.40.4 source 2026-11-20), so the
  adapter reports the terminal as unknown, which core treats as no evidence either way.
- **A zombie is gone.** `ProcessStatus::Zombie` is reported as exited: never listed,
  never a target, never counted as a survivor. `kill(2)` on a zombie returns 0 on macOS
  (observed 2026-11-20), so without this a stopped process would log `sent` and block
  `z`'s "every process is paused" test.
- **Own uid and ancestry.** The adapter keeps only processes whose uid is loadtop's own
  (`Process::user_id` of its own PID, since the standard library has no `getuid`). A
  process whose parent is not in the table (root's, another user's) is the top of its
  tree: it attaches by markers only, and loadtop's ancestor chain for the guard stops
  there.
- **cwd freshness.** cwd is re-read every sample for ppid-1 processes and session
  roots, and once for the rest; the detail overlay (S5) re-reads the selected
  process's cwd when it opens, so the cwd a person reads is current.

## Listening ports: lsof

`/usr/sbin/lsof -a -nP -iTCP -sTCP:LISTEN -u <uid> -Fpn`, run through
`std::process::Command` with each argument through `.arg()`, and its `-F` output
parsed: `p<pid>` lines start a process, `n<addr>:<port>` lines name its sockets
(observed on this Mac, 2026-11-20). `-F` is lsof's documented machine-readable form
(`man 8 lsof` on macOS 27.0, "OUTPUT FOR OTHER PROGRAMS", read 2026-11-20); its human
table is never parsed. It also prints `f<fd>` lines and may print other field letters:
every line whose first character is not `p` or `n` is skipped, and the port is the
digits after the last `:` of an `n` line, so an IPv6 address's colons do not confuse it.
`<uid>` is the one the process-table adapter reads, never parsed from user input.

- **Why a command.** A crate for this either runs bindgen at build time against a
  hard-coded SDK path (`netstat2` 0.11.2's `build.rs`, read 2026-11-20: it fails
  `cargo install` on a Mac with only Xcode, and its build crates `bindgen`
  (BSD-3-Clause) and `libloading` (ISC) failed `cargo deny`'s licence list in a trial
  build the same day), or needs `unsafe` to read a socket union (`libproc`'s socket
  info API, read 2026-11-20).
  `lsof` ships with macOS and needs neither. `netstat2` matched lsof's (pid, port) set
  16 of 16 at 6 ms against 30 ms wall (observed 2026-11-20); the 24 ms is not worth a
  build that can fail.
- **Cost and cadence.** About 20 ms of CPU per call, so it runs every third sample
  (6 s at the default interval) and once more when a confirmation opens, so a
  confirmation's port warning is current (requirements §3.2). That adds about 0.3% of
  one core.
- **Failure is not fatal.** Exit 1 with empty stdout and empty stderr means no
  listening socket (observed with a port nothing listens on, 2026-11-20); lsof also
  exits 1 for a bad argument, but then writes to stderr, and the arguments here are
  fixed. Any other non-zero exit, a missing binary, unparsable output, or no answer
  within 2 s (the child is then killed and reaped) becomes the port's "unavailable"
  kind; core then shows ports as `-`, the pause confirmation says ports could not be
  read, and the rest of the sample stands.
- IPv4 and IPv6 sockets on the same port are one port in the view.

## Claude Code's session record

Claude Code writes `~/.claude/sessions/<pid>.json` for each live session and removes it
on exit (observed for desktop, terminal, and `claude -p` sessions, 2026-11-20). It is
not a documented contract, so loadtop depends on it in reverse: any Claude Code release
may rename, move, or drop it.

- The adapter reads only files named `<digits>.json` in that directory, never the
  `*.key` files beside them, and parses each field as optional: `pid`, `procStart`,
  `sessionId`, `cwd`, `entrypoint`, `status`, `name`. An unparsable file is skipped, not
  an error.
- `procStart` is a ctime-style string in UTC (`Fri Nov 20 22:34:19 2026`, observed
  against `ps -o lstart` 2026-11-20); the adapter parses it to Unix seconds, and an
  unparsable one makes the record invalid. Core uses a record only when its `pid` is a
  live `claude` process and that value equals the process's start time; otherwise the session falls back to
  argv, executable path, tty, and cwd (requirements §3.1). So a change to the format
  costs the session name and busy/idle status, never a session.
- The parser is tested against recorded samples of each kind, with invented IDs and
  paths, under `crates/loadtop-platform/tests/`. A change Claude Code makes shows up as
  sessions losing their name; check a fresh record against those samples before
  changing the parser.

## Project folder names

`read_dir` on `~/.claude/projects/`, returning names only. No file inside is opened:
the folders hold transcripts. A missing directory is an empty list.

## Signals

`sysinfo::Process::kill_with(Signal::{Stop, Continue, Term, Kill})`, all four supported
on macOS (`supported_signals` in the 0.40.4 source, read 2026-11-20), so sending needs no
new crate and no `unsafe` of ours.

- **Identity is re-checked at the last moment.** For each target the adapter refreshes
  that one PID (`ProcessesToUpdate::Some`), compares its start time with the one core
  holds, and only then calls `kill_with`; a mismatch or a missing process is `Gone`
  without sending. The window between the check and the call is microseconds, against
  the minutes a confirmation can stay open.
- **`kill_with` returns `Some(false)` with no reason** (it wraps `kill(2)` and drops
  `errno`; same source). On a failure the adapter refreshes the PID again: gone, or a
  new start time, is `Gone`; still there is `PermissionDenied`, the only other reason
  `kill(2)` gives for a valid signal to a same-user process. `None` (a signal the
  platform does not support) is the port's "unsupported" kind and is unreachable on
  macOS.
- Ordering (SIGCONT after SIGTERM for a paused target, the 3 s grace, re-resolving
  survivors) is core's plan; the adapter sends exactly what it is handed
  (`building-tuis` › "Refresh, sampling, and the stop grace").
- Contract tests send signals only to children the test spawned itself (a `sleep` it
  started), never to an existing process.

## Permissions and privacy

Everything above runs as the user with no `sudo`, no TCC prompt, and no Keychain:
reading another same-user process's env, cwd and sockets, and signaling it, needs no
grant (observed on macOS 27.0, 2026-11-20). Other users' and root's processes are
filtered out in the adapter by uid, before any value is built. Older macOS versions
are untested; the env rule for platform binaries is known only on 27.0.

## Linux

The platform crate keeps building and passing its tests on Linux, because CI's
`Rust Core` job runs there and the core it serves builds there; loadtop promises nothing
on Linux (`AGENTS.md` › Product, Non-goals). sysinfo reads `/proc` there; the socket
adapter runs `lsof` at `/usr/bin/lsof` and reports "unavailable" when it is missing;
the session record and project paths are the same under `$HOME`. Nothing Linux-only is
added beyond that.
