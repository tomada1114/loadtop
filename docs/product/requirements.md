# loadtop — Requirements

- **Status:** Signed off 2026-11-20
- **Platform and stack:** macOS (tested on Apple Silicon with macOS 27; Intel and older macOS untested); a single Rust
  binary cut from `rust-template`, with clap flags and a ratatui
  full-screen view over one core crate. Local only: no network, no daemon, no LLM, no API key.

## 1. Overview

- **What it is:** a terminal tool for people who run many Claude Code sessions on one Mac.
  It lists every Claude Code session with the processes it started (child, grandchild,
  and so on). It puts the heaviest first, shows which project each belongs to, and marks
  the leftovers a closed session left behind. You can then stop a whole tree, or pause it
  and resume it later, without hunting PIDs.
- **Target users:** developers on a laptop with limited headroom (the author uses a
  MacBook Air) who keep several Claude Code sessions open. Those sessions start dev
  servers, test watchers, MCP servers and builds. The machine slows down and nobody can
  tell which session is to blame, or what is left over from a session already closed.
- **Core interaction:** open `loadtop`, see the heaviest session or leftover at the top,
  select it, and stop it (whole tree) or pause it. If this is not quick and safe, nothing
  else matters.
- **How it differs (the README's opening, signed off as option A):**

  > **loadtop** — top for Claude Code sessions. See which session, and which project, is
  > slowing your Mac down, then stop or pause that whole process tree in one action.
  > htop shows processes one by one; loadtop groups them by the Claude Code session that
  > started them, finds what closed sessions left behind, and acts on a whole tree at once.

  Below it, a comparison table states only checked facts. htop 3.5.3 has a tree and
  sends any signal, SIGSTOP included, but one PID at a time; a tree takes two steps, `c`
  to tag and then `k`. It has no ports column (`l` opens lsof's file list for one
  process) and no grouping by agent session. btop
  v1.4.8 has a tree and per-PID signals, with no env, cwd or ports. cctop (150 stars,
  Claude Code only) stops a session's parent process only, sorts by CPU or memory,
  stops leftovers with ports, and has no pause. Sources checked 2026-11-20: the htop
  source (`Action.c`, `darwin/Platform.c`, `Process.c`), the btop source, and cctop's
  README and source.

## 2. Scope

### MVP
- Session discovery: Claude Code CLI (terminal), desktop, and headless runs (§3.1)
- The session → child → grandchild tree, with each process's CPU, memory and listening TCP ports (§3.2)
- Heaviest first by default (§3.2)
- Leftover groups, with or without ports, in two tiers (confirmed and probable), listed among the sessions by weight (§3.3)
- Stop a whole tree: SIGTERM, then SIGKILL for survivors, with confirmation (§3.4)
- Pause and resume a tree (SIGSTOP and SIGCONT), with a sleeping mark and a quit prompt (§3.5)
- `--once` and `--json` (§3.6)
- A `/` filter over the list (§3.7)
- A per-group CPU trend, kept in memory while loadtop is open (§3.8)
- Safety guards: the session process itself, loadtop, and loadtop's ancestors (§3.4, §4)

### Later
- Codex sessions: dropped from the first release because the author no longer uses Codex
  and cannot dogfood it. Codex's child markers (`CODEX_THREAD_ID`) exist in source but
  are undocumented. Pulled forward by a user request with a way to test it.
- Load history and looking back ("what froze a minute ago"): the second release. It
  needs a background recorder.
- Non-interactive actions (`loadtop stop|pause|resume <pid>`): an action without the on-screen confirmation is riskier and is a step toward automatic stopping. Pulled forward by a concrete scripting need; it would require `--yes` and the same guards.

### Non-goals
- Tokens, context, quota or cost: abtop and agtop already cover them, and adding them
  would dilute "load only".
- Stopping anything automatically: no daemon, no hooks, no LaunchAgent (cc-reaper's
  territory). A tool that kills on its own is too risky.
- Waking a paused server when a connection arrives: that needs a proxy in front of the port.
- Guaranteeing Windows or Linux: the author cannot test them. The code may build on
  Linux, since the template's core builds there.
- Cleaning up simulators, containers, or automation browsers.
- Themes and a settings screen. Behavior is set with command-line flags only.
- Claude desktop's chat-side MCP servers (the Claude app's own, not Claude Code's).
- Other users' processes and root processes: never shown, never signaled. No `sudo` path.
- Showing environment variable values other than the session ID (§4 Privacy).
- Listing or acting on processes Claude Code did not start: that is htop's job. Only their total load appears in the header (§3.2).

## 3. Features

### 3.1 Session discovery

#### Overview
- **Purpose:** find every running Claude Code session and attach its processes to it.
- **Access:** every refresh, in the TUI and in `--once`/`--json`.

#### Specifications

| Item | Specification |
|------|---------------|
| What counts as a session | A process of the `claude` executable, from either the CLI install or the desktop app's bundled `claude`, that is not a helper invocation |
| Helper invocations excluded | `--chrome-native-host` (observed 2026-11-20). The helper flags and subcommands are a list kept as data in core (`designing-core-logic` › "loadtop's core"), each entry tested with a recorded argv sample; a helper seen later is added to it |
| Session record (undocumented, preferred when valid) | Claude Code writes `~/.claude/sessions/<pid>.json` for each live session and removes it on exit. Observed 2026-11-20 for the desktop, terminal and `claude -p` sessions. Fields used: `pid`, `procStart`, `sessionId`, `cwd`, `entrypoint`, `status`, `name`. A record is used only when its `pid` is alive and its `procStart` matches the process's start time. loadtop reads only `<pid>.json` files there, never the `*.key` files beside them |
| Session kinds shown | `terminal` (entrypoint `cli`), `desktop` (`claude-desktop`), `headless` (`sdk-*`, for example `claude -p`). Without a valid record, the kind comes from the root's argv and executable path (desktop's bundled `claude` lives in the app's support directory; `-p`/`--print` means headless), and from whether it has a controlling terminal |
| Session status and name | From the record when valid: `busy` or `idle`, and the session's name, both shown on the session row. Without a record, both are omitted. The record is not a documented contract, so every field must have a fallback or be optional |
| Child markers (documented) | `CLAUDECODE=1`, `CLAUDE_CODE_SESSION_ID`, and `CLAUDE_CODE_CHILD_SESSION` (set for tool children, not for stdio MCP servers). Source: code.claude.com/docs/en/env-vars, checked 2026-11-20 |
| Child markers (observed, not relied on alone) | `CLAUDE_PID` (Bash tool children only), `AI_AGENT` (in the changelog only), `CLAUDE_CODE_ENTRYPOINT` (undocumented) |
| Attaching a live process | Ancestry first: a process under a session root belongs to it. Markers decide only what ancestry cannot: leftovers (§3.3) |
| Child kind label | `tool` when `CLAUDE_CODE_CHILD_SESSION` is set, `mcp` when the session marker is present without it, `-` when env is unreadable |
| Nested sessions | A `claude` started inside another session (for example `claude -p` from a Bash tool) appears as a session nested inside its parent's tree |
| Project | The record's `cwd`; otherwise the root's cwd; otherwise the cwd of its marked descendants. Shown home-relative (`~/ghq/…/rust-template`). Observed: the desktop's `claude` root has the project as its cwd, and the `disclaimer` wrapper above it does not. Two roots returned no cwd once while exiting |
| Session ID changes | The documented ID changes on `/clear`. Grouping therefore uses the root process; the IDs are a set per root, collected from its live descendants |

#### Edge Cases
- **No sessions:** the view says no Claude Code session is running, unless leftover groups exist; then they are listed alone.
- **Unreadable env:** Apple platform binaries (`/bin/sh`, `/bin/zsh`, `/bin/sleep`, `/usr/bin/tail`, `/usr/bin/caffeinate`) return no environment. Observed 2026-11-20 on macOS 27.0; suspected cause: they are platform binaries. They still attach by ancestry. As leftovers they can only be probable (§3.3).
- **Process exits between reads:** dropped silently on the next refresh; never an error.

### 3.2 The tree and the load columns

#### Specifications

| Item | Specification |
|------|---------------|
| Columns per process | PID, name and command line (truncated to fit), CPU %, memory (RSS), listening TCP ports, state (running, sleeping-by-signal) |
| CPU % | Over the last refresh interval, as a share of one core (htop's convention: 100% is one full core) |
| Memory | Resident set size, in human units (`412M`, `1.3G`) |
| Ports | TCP sockets in LISTEN state, IPv4 and IPv6, port numbers only. UDP is not shown. A new listener appears within 6 s; ports are read again when a confirmation opens, so its warnings are current. Read through macOS's `lsof` (`integrating-system-apis`); when ports cannot be read they show as `-` and the pause confirmation says so |
| Group totals | Each session row and each subtree shows the sum of its tree's CPU and memory |
| Default order | Heaviest first by the sort key: sessions and leftover groups in one list by tree total; inside a tree, siblings by subtree total |
| Sort key | CPU by default; one key toggles between CPU and memory. The active key is shown in the header |
| Header totals | Claude Code's total (CPU in cores, memory) beside the whole machine's (CPU in cores of the total, memory used of total), so you can tell at once whether the culprit is outside Claude Code |
| Refresh interval | 2 s by default; `--interval <seconds>` accepts 1–10 |
| Collapse | On launch each group shows its top 3 child subtrees and folds the rest into `+N more`; any group can be expanded or collapsed. A paused process is never folded away (ux-flows) |
| Order stability | Order uses the mean CPU % of the last 3 samples (6 s at the default interval), so the top row does not flap; the CPU% column shows the latest sample. The help overlay says so |

#### Edge Cases
- **Hundreds of children (a parallel test run):** the tree stays sorted and scrollable, and totals stay correct. Rendering cost is bounded by the visible rows.
- **A process re-parented while being viewed:** shown where it is now. A child of a live session that moved to ppid 1 keeps its session group, labeled `detached`.

### 3.3 Leftovers

#### Overview
- **Purpose:** show the processes a closed session left behind, so they can be stopped.

#### Specifications

Leftovers are judged per orphan subtree, not per process: the topmost process with ppid
1 and everything under it. The Bash tool runs commands through `/bin/zsh -c`, so the top
of a leftover tree is often unreadable while a `node` below it carries the marker.

| Tier | Rule | Shown as |
|------|------|----------|
| Confirmed | Same user. Some member of the subtree carries `CLAUDE_CODE_SESSION_ID`, and its owning session is not alive. Not alive means no live session has that ID (its record, or its live descendants' IDs), and, when `CLAUDE_PID` is present, that PID is not a live `claude` session | `leftover`, labeled "started by a closed session", with the dead session's short ID and the project from its cwd |
| Probable | Same user. No member's env carries a marker, and the top's env is unreadable. Its cwd is inside a Claude Code project | `leftover?`, visibly different from confirmed |
| Never a leftover | A subtree that contains a live `claude` session (a nested session whose parent ended): it is listed as a session, with root protection. Also a subtree whose readable envs carry no marker | not listed |

- **What "inside a project" means:** the cwd itself, or one of its ancestor directories,
  encoded the way Claude Code names project folders (every non-alphanumeric character
  becomes `-`), equals a live session's cwd or a folder name under `~/.claude/projects/`.
  Only the folder names are read, never their contents. An exact match per ancestor, not
  a prefix, so `~/foo-bar` is not inside project `~/foo`. The home directory and `/` are
  never projects, even if a session once ran there.
- **Observed 2026-11-20:** on the author's Mac this rule flagged none of the same-user,
  env-unreadable processes at ppid 1, which are mostly launchd agents with cwd `/`.
- **Daemons:** a long-lived daemon started from a session (tmux, ssh-agent) inherits the
  marker too. That is why the label says "started by a closed session", not "garbage".

- With or without ports: both count.
- Leftover groups sit in the same list as sessions, ordered by tree total (§3.2). A leftover's own descendants are listed under it.

#### Edge Cases
- **None:** no leftover rows in the TUI, and an empty `leftovers` array in `--json`.
- **A probable leftover that is the user's own:** for example a server started by hand in a project directory. That is why probable leftovers get the stronger confirmation (§3.4).

### 3.4 Stop a tree

#### Specifications

| Item | Specification |
|------|---------------|
| Target | The selected row's subtree. On a session row, every child subtree of that session; the session root is excluded. A live nested `claude` session inside the target, with its subtree, is always left out, and the confirmation names it ("skipped: 1 nested session — select it and use the session action") |
| Target drift | The confirmation shows the target as it was when it opened; at `y` the target is resolved again (processes born into it since are included), and the result row reports the real counts |
| Session root | Excluded by default. A separate, explicit action stops it, and its confirmation names the session's project and kind and says the session will end |
| Never targetable | loadtop itself and every ancestor of loadtop, so loadtop running inside a Claude Code session cannot stop or freeze its own tree |
| Signals | SIGTERM to every process in the target, re-scan, then SIGKILL to every survivor after a 3 s grace. Processes born into the target during the grace are included |
| Confirmation | Default answer No. Shows the process count, the top 5 processes by load, and the total CPU and memory. Probable leftovers add a line saying ownership is inferred |
| Identity check | Each target is held as PID plus process start time. Before every signal (TERM, KILL, STOP, CONT, the quit prompt's resume included), a process whose start time no longer matches is skipped as already gone, because a confirmation can stay open long enough for a PID to be reused |
| Result | A one-line result: stopped N, already gone M, failed K, with the reason (permission, not found) |

#### Edge Cases
- **A paused process in the target:** SIGCONT follows SIGTERM, so it can act on the signal.
- **Permission denied (a process changed user):** reported in the result row, not fatal.

### 3.5 Pause and resume

#### Specifications

| Item | Specification |
|------|---------------|
| Pause | SIGSTOP to every process in the selected subtree; the same target rules and guards as §3.4. The session root needs the same separate action |
| Resume | SIGCONT to every stopped process in the selected subtree. No confirmation when every one was paused by this run; when any was stopped by something else (Ctrl-Z in a shell, a debugger), a confirmation names those first, default No |
| Sleeping mark | Read from the kernel's process state every refresh; no state file. Each stopped row gets a mark, and the header shows the count |
| Stopped by someone else | A process stopped by another tool (for example Ctrl-Z in a shell) gets the same mark and can be resumed. It is not counted in the quit prompt |
| Pause confirmation | Says what is known to go wrong: a paused process with a listening port accepts connections but never answers (observed: connect at once, the request waits until the client times out, and is served on resume). The session's tool call waits on it. If the session ends while it is paused, the paused child is likely killed (observed: a stopped member of a newly orphaned process group dies) |
| Quit prompt | When anything this run paused is still stopped: "Resume and quit / Quit and leave them paused / Back" |
| Elapsed time | Shown only for processes paused in this run (the kernel does not record when a stop happened) |

#### Edge Cases
- **loadtop killed (SIGKILL, terminal closed):** no prompt can run; the marks show the stopped processes on the next launch.
- **`--once` and `--json`:** read-only; they never pause, resume or stop.

### 3.6 `--once` and `--json`

| Item | Specification |
|------|---------------|
| First sample | The first frame of the TUI and `--once`/`--json` use two reads 300 ms apart, so CPU % appears within about 0.3 s; later refreshes use the interval |
| `--once` | Takes the first sample, prints the full tree as indented plain text (no folding: it is the accessible path), and exits |
| `--json` | Same sample, printed as JSON on stdout. Implies `--once` |
| `--interval` with them | A usage error (exit 2): one sample has no refresh interval, and its CPU window is always the 300 ms first sample |
| JSON contents | `schema_version`, sample time, the CPU measurement window, sessions (kind, project, root PID, session IDs, totals, process tree), leftovers (tier, process tree), and per process: PID, ppid, name, command line, CPU %, RSS bytes, ports, state |
| Exit codes | 0 on success, including when nothing is found; nonzero when the process table cannot be read. Exact codes follow `designing-clis` |
| Output stability | `schema_version` starts at 1; a field removed or retyped bumps it and is noted in `CHANGELOG.md` |
| Key style | snake_case throughout (`schema_version`, `cpu_percent`, `rss_bytes`) |

### 3.7 Filter

#### Overview
- **Purpose:** with many sessions open, narrow the list to one project or one command
  without losing the order or the safety rules.
- **Access:** `/` in the main view (ux-flows S1f).

#### Specifications

| Item | Specification |
|------|---------------|
| Input | `/` opens a one-line input in the result row. Typing filters the list live. `Enter` keeps the filter and returns to the list; `Esc` while typing restores the filter that was active before `/`. `Backspace` deletes, `Ctrl-U` clears the input. While typing, letters are text: `q`, `x`, `z`, `j`, `k` and the other list keys do not act. `Ctrl-C` still quits (through the quit prompt when something this run paused is still stopped) |
| Clearing | `Esc` in the list clears an active filter. An empty input is no filter |
| Match | Case-insensitive substring, against the group row's text (kind, project, session name, leftover short ID) and each process's name, full command line (not the truncated text on screen, so a match does not depend on the width), and PID |
| What is shown | A group whose row matches is shown whole (folded as usual). A group where only processes match is shown with those processes and the chain of ancestors from each up to the group row; the rest of the group folds into `+N hidden`. Groups with no match are hidden |
| Paused processes | Never hidden: a paused process that does not match is lifted out, prefixed `…`, as a fold lifts it (ux-guidelines Principle 3). Its group row is then shown as well, so the paused row always has its session or leftover above it |
| Totals | Group totals and header totals always cover the whole tree and the whole machine, never only the matching rows |
| `+N` rows | `x` and `z` on a `+N more` or `+N hidden` row do nothing and say, in the result row, `select a process or the group row` |
| Actions | Unchanged by the filter. `x`, `z` and their root variants act on the selected row's whole subtree, matching or not; the confirmation counts hidden members and says so ("includes 4 processes hidden by the filter") |
| Shown state | Line 2 shows `filter "acme" · 3 of 9` (groups shown of all) while a filter is active, after the paused count |
| Across refreshes | The filter persists until cleared; new matching processes appear. If the selected row is filtered out, the selection moves to the first shown row |
| `--once`/`--json` | Not affected and not offered: a script filters the JSON itself |

### 3.8 CPU trend

#### Overview
- **Purpose:** show whether a group is heavy now or was heavy a moment ago, without a
  recorder: loadtop remembers the last samples only while it is open.

#### Specifications

| Item | Specification |
|------|---------------|
| Where | Group rows only (sessions and leftover groups), in the cells that the PORTS and STATE columns occupy on process rows, so the 80-column layout is unchanged. A group row whose root is paused or not targetable shows `paused` or `self` in STATE as a process row would, and the trend then takes only the PORTS cells (its last 6 samples) |
| Length | The last 10 samples (20 s at the default interval), newest on the right; a group seen for fewer samples shows only those, right-aligned |
| Glyphs | The eight block elements `▁▂▃▄▅▆▇█` (U+2581–U+2588). A sample above 0% draws at least `▁` (the level is the sample's share of the scale, rounded up to eighths); a sample at 0% is a space |
| Scale | Shared by every shown group, so rows compare: the top is the largest group CPU% in any shown group's window, and never less than 100% (one core), so an idle list stays flat |
| Metric | The group's tree CPU% as the CPU% column computes it, whatever the sort key |
| Memory | Held only in memory, keyed by the group's root PID and start time, and dropped when the group disappears. Nothing is written to disk (the second release's history is separate) |
| Supplementary | The trend repeats nothing the row's numbers do not already allow, so it has no color, no label of its own beyond the column, and is absent from `--once` and `--json`, which hold one sample |

## 4. Cross-cutting rules

- **Light on the machine (a requirement):** at the default 2 s interval, at most 2% of
  one core on average and at most 30 MB RSS on the author's MacBook Air, with around 800
  processes running. Measured by the prototype (2026-11-20, Mac14,2, 8 cores, sysinfo
  0.40.4): about 20 ms of CPU per full scan (about 1% of one core at 2 s), and 9–10 MB
  RSS. Reading ports through `lsof` costs about 20 ms more per call. Checked two
  ways: an `#[ignore]`d measurement test that samples the real process table 30 times
  and checks CPU time per sample and RSS, which a human runs with `just test-local`
  before each release, and core tests in CI that pin the work per refresh (drawing is
  bounded by the visible rows), since wall-clock timing in CI is too noisy to gate on
  (`placing-tests`).
- **Permissions:** runs as the user. Needs no `sudo`, no macOS privacy (TCC) prompt, no
  Keychain, and no network. Fields it cannot read are shown as `-`, never as an error.
- **Privacy:** reads other processes' environments only to find the markers above. It
  never displays, logs or writes any other variable's value. Command lines are shown as
  `ps` shows them.
- **No state on disk:** v1 writes no state file. It writes logs only where the template's
  logging puts them.
- **Safety first:** every signal-sending action confirms first and defaults to No. Rows
  that cannot be targeted say why when selected.

## 5. Data

No persistent data. In-memory only: the process sample (per refresh), the last 3
samples per process for the order (§3.2), the last 10 group CPU% values per group for
the trend (§3.8), the set of processes this run paused (for the quit prompt and elapsed
time), the per-root session ID sets, and the filter text (§3.7). `--json` output is the
one external format (§3.6).

## 6. Open questions

- Command lines in `--json` can contain secrets that a user passed as arguments, exactly
  as `ps` shows them. Proposed: document it, and do not redact.
- What a session does while one of its children is paused: tool-call timeouts and MCP
  reconnects. Observing it needs a live session; it does not block the spec. Settled by
  a manual check during implementation.
- After `/clear`, an orphan that carries only the old session ID, with no `CLAUDE_PID`,
  can look confirmed-leftover while its session is alive. Settled by observing how often
  MCP children (no `CLAUDE_PID`) outlive a live session's `/clear`.
- Whether the platform-binary env rule holds on older macOS versions (only 27.0 was
  checked). Settled by a check on another macOS version, or by documenting 27.0 as the
  tested version.

## 7. Decision log

- 2026-11-18 Name `loadtop`, one-liner "surface the heavy culprit, then stop it or put
  it to sleep", and first-release scope (rejected: B, load history first, because it
  needs a resident recorder; C, leftover cleanup as the core, because cc-reaper already
  does it).
- 2026-11-19 Rust, one binary, on `rust-template` (rejected: Go).
- 2026-11-20 Claude Code only in the first release (rejected: also Codex, because the
  author no longer uses it).
- 2026-11-20 Leftovers in two tiers, confirmed and probable (rejected: confirmed only,
  which misses sh/zsh/sleep leftovers; cwd only, which cannot name the owning session).
- 2026-11-20 Sleeping mark from kernel state, plus a quit prompt for processes this run
  paused (rejected: always resume on quit, which cannot leave anything paused; mark
  only, which leaves forgotten sleepers).
- 2026-11-20 Sessions include interactive and headless runs, labeled separately
  (rejected: interactive only).
- 2026-11-18 License MIT (the author's direction in the brief).
- 2026-11-20 README opening leads with per-session grouping (A) and a fact-only comparison table (rejected: B, pause first, which has no evidence of demand and htop can send SIGSTOP; C, leftovers first, which overlaps cc-reaper).
- 2026-11-20 Processes outside Claude Code appear only as a header total (rejected: a top-5 "other" section, which dilutes the focus; nothing at all, which hides an outside culprit).
- 2026-11-20 Default sort CPU, with a toggle to memory (rejected: memory first; a combined score, which is hard to explain).
- 2026-11-20 No non-interactive stop/pause/resume commands in v1 (rejected: subcommands with `--yes`).
- 2026-11-20 Advisor review of the requirements: direction sound. Changes: leftovers are
  judged per orphan subtree; a live nested session is never a leftover; "inside a
  project" is an exact per-ancestor match, and home and `/` are never projects; PID plus
  start time is re-checked before every signal; the README says "one action" and "no
  ports column"; and the undocumented session record is used when valid, with fallbacks.
- 2026-11-20 Nested sessions are left out of a subtree action; resuming something loadtop did not pause confirms; the target is re-resolved at `y`; `--once` prints the full tree; the first sample takes 300 ms; order uses a 3-sample mean.
- 2026-11-20 `loadtop` with no arguments opens the full-screen view; `--once`, `--json`
  and `--interval` are flags, and there is no `tui` subcommand (rejected: keeping
  `loadtop tui`, which the README's "run `loadtop`" contradicts; both, which doubles the
  entry points). `--json` keys are snake_case (rejected: camelCase). Memory is RSS, and
  the help overlay and README say it differs from Activity Monitor's "Memory".
- 2026-11-20 The `/` filter (§3.7) and the in-memory CPU trend (§3.8) join the MVP
  (rejected: leaving both for later). Neither writes anything to disk.
- 2026-11-20 The session record is read as §3.1 says: it is preferred when valid and
  never required, so a Claude Code release that changes it degrades the session name and
  status to absent, not the tool.
- 2026-11-20 The session root, loadtop and loadtop's ancestors are excluded by default;
  stopping the root is a separate action; SIGTERM, then SIGKILL after 3 s (rejected:
  root included in the tree; root untouchable).
