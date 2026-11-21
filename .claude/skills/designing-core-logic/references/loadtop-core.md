# loadtop's core

What loadtop's core computes and decides, on top of the shape the skill body gives every
core. The screens and the rules they serve are `docs/product/requirements.md` and
`docs/product/ux-flows.md`; this page is how core is cut to serve them.

- **Six ports**, each at a boundary something outside the process answers: the process
  table, the socket table, the session records, the project folder names, the signal
  sender, and the clock. What each promises and which adapter serves it is
  `integrating-system-apis`' process-sources reference.
- **One sample is the unit of input.** The binary hands core a sample value (the
  process table, the sockets when read this time, the records, the folder names, and
  the instant it was taken) and, once at start, loadtop's own PID and start time.
  Everything shown is computed from them by pure functions:
  sessions and their trees, leftover tiers, guards (loadtop and its ancestors, the
  session root, nested sessions), totals, the 3-sample order, folding, the filter, and
  the trend. They are tested with hand-built samples, inside the coverage floor, on
  Linux: no fake OS is needed to test what a leftover is.
- **What a session is.** A process of the `claude` executable whose argv holds none of
  the helper flags and subcommands core keeps as a list (`--chrome-native-host` today,
  observed 2026-11-20), each entry with a test over a recorded argv. A helper seen later
  is a new entry and a new test, not a heuristic.
- **What an action can hit.** A target is the selected row's subtree minus loadtop, its
  descendants (its own `lsof`), its ancestors, and any live nested session's subtree;
  on the session row that contains loadtop, `x` therefore stops the session's other
  children and leaves loadtop's chain running. A row with nothing left to target says
  why in the result row.
- **Acting is a plan.** A stop, pause, or resume is a value core returns: the targets as
  (pid, start time), the signals to send now (SIGCONT after SIGTERM for a paused
  target), and, for a stop, a pending escalation with its deadline. When a later sample
  shows the deadline passed, core returns the SIGKILLs: survivors are the original set
  still alive and not zombies, plus their current descendants. Several escalations can
  be pending; each is its own value. The binary sends what it is handed through the
  signal port and returns the per-target results. Core never waits out the 3 s itself;
  how the loop carries this is `building-tuis` › "Refresh, sampling, and the stop
  grace".
- **Tuning** holds the shipped numbers, each with its reason: the interval (2 s, 1–10),
  the first-sample gap (300 ms), the order window (3 samples), the port cadence (every
  3rd sample), the stop grace (3 s), the subtrees shown per group (3), the trend length
  (10), and the heat thresholds (CPU 50/100 %, MEM 500M/1G, ux-guidelines). `--interval`
  is the one a user sets.
