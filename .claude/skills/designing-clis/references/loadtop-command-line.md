# loadtop's command line

The invocations, flags, and `--json` document `loadtop` promises. They are contract
(`AGENTS.md` › "What is contract and what is private"), pinned in
`crates/loadtop/tests/cli.rs`; a change here is a `CHANGELOG.md` entry, and a removed or
renamed form is a breaking one.

## Invocations

One command, no subcommand. The person's path is the screen; a script's is one sample.

| Invocation | Does | Exit |
|---|---|---|
| `loadtop` | opens the full-screen view (`building-tuis`); refuses with `error: …` when stdin or stdout is not a terminal | 0 on quit; 1 when refused or the process table cannot be read |
| `loadtop --once` | prints one sample as indented plain text (ux-flows T1) and exits | 0, also when nothing is found; 1 when the process table cannot be read |
| `loadtop --json` | prints the same sample as one JSON document (below); implies `--once` | as `--once` |
| `--interval <seconds>` | the screen's refresh interval, an integer from 1 to 10, default 2 | 2 when out of range (clap's `value_parser` range) or given with `--once`/`--json` |

- **Why no subcommand.** The core interaction is "run `loadtop`, see the culprit"
  (`AGENTS.md` › Product), and the README says exactly that. A `tui` subcommand would
  make the common path the longer one; keeping both would make two spellings of one
  entry point, each contract. `--once` and `--json` are modes of the same sample, not
  separate verbs, so they are flags.
- **Nothing signals from the command line.** No flag stops, pauses, or resumes: those
  need the on-screen confirmation (requirements §2, Later, for why). A future
  `loadtop stop <pid>` would be a subcommand beside the bare form, with `--yes` and the
  same guards, and a design change.
- **No short flags.** Three long flags are easy to remember, and one spelling keeps the
  docs, `--help`, and the permission patterns in `.claude/settings.json` agreeing. What
  keeps a script from hanging on the screen is the terminal check, not the flags.
- **`--interval` with `--once` or `--json` is a usage error** (exit 2, clap's
  `conflicts_with`): one sample has no refresh interval, and silently ignoring a flag
  hides a script's mistake.

## The `--json` document

- **Keys are snake_case** (`schema_version`, `cpu_percent`, `rss_bytes`): the output type
  derives `Serialize` with `#[serde(rename_all = "snake_case")]`, which overrides the
  camelCase default `designing-core-logic` gives a view. snake_case is what the
  requirements spell and what a `jq` filter reads without quoting, and the tools a
  loadtop user pipes beside it (`cargo metadata`, `ps`-style JSON) use it.
- **The shape, as a literal.** `cli.rs` pins this document's shape (values invented):

  ```json
  {
    "schema_version": 1,
    "sampled_at_ms": 1763650000000,
    "cpu_window_ms": 300,
    "machine": { "cpu_cores": 8, "cpu_percent": 520.0,
                 "memory_used_bytes": 15246955315, "memory_total_bytes": 17179869184 },
    "claude": { "cpu_percent": 310.0, "rss_bytes": 3113851289 },
    "sessions": [
      { "kind": "desktop", "project": "~/code/acme-web", "name": "Fix checkout flow",
        "status": "busy", "session_ids": ["3f9c2a10-…"], "cpu_percent": 182.0,
        "rss_bytes": 1717986918,
        "root": { "pid": 91190, "ppid": 1, "start_time": 1763647923,
                  "name": "claude", "command": "claude", "kind": null,
                  "cpu_percent": 0.4, "rss_bytes": 104857600, "ports": [],
                  "state": "running", "session": null, "children": [] } }
    ],
    "leftovers": [
      { "tier": "confirmed", "closed_session_id": "8b41d0e7-…",
        "project": "~/code/acme-api", "cpu_percent": 96.3, "rss_bytes": 775946240,
        "root": { "…": "a process, as above" } }
    ]
  }
  ```

  A process's `kind` is `"tool"`, `"mcp"`, or `null` when its env is unreadable;
  `state` is `"running"` or `"stopped"`; `cpu_percent` is `null` when it cannot be
  computed; `project`, `name`, `status`, and `closed_session_id` are `null` when
  unknown; `tier` is `"confirmed"` or `"probable"`. A nested session is a process in its
  parent's `children` whose `session` holds `kind`, `name`, `status`, and
  `session_ids`; a session whose parent session ended is a top-level entry.
  `start_time` is Unix seconds. Unreadable is `null`, never `"-"` or `0`.
- **`schema_version` is the first key and starts at 1.** A field removed, renamed, or
  retyped bumps it, with a `CHANGELOG.md` line; a field added does not. The JSON is its
  own type in core (`JsonReport`-shaped), built from the sample, never the screen's
  view, so the screen's layout can change without touching the contract.
