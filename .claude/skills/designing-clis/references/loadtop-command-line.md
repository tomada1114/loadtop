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
| `loadtop --once` | prints one sample as indented plain text (ux-flows T1) and exits | 0, also when nothing is found; 1 when the process table cannot be read, `HOME` is unset or empty, or stdout cannot be written |
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

## The `--once` text

ux-flows T1, laid out by `crates/loadtop/src/once.rs` and pinned by its unit tests:

- **The header** is `CPU Claude <cores> cores · all <cores> of <cpus>   MEM Claude <mem> ·
  used <used>/<total>`; cores have one decimal (`0` when zero), and `used/total` writes a
  shared unit once (`14.2/16G`), as the ux-flows examples do.
- **A group line** is the kind word and a space (`desktop  `, `leftover `, `leftover? `),
  then `project · name · status` for a session, `project · closed session <first 8 of
  the ID>` for a confirmed leftover, or `project` for a probable one, missing parts
  dropped; the project is home-relative. Its CPU% field sits at columns 44–49, as on
  process lines, pushed right to keep two spaces after a long label.
- **A process line** is `  <pid:>5>  <command:29><kind:4>  <cpu%:>6>  <mem:>4>`, then
  the ports and `paused` when present, two spaces apart, with trailing spaces trimmed.
  The command is two spaces per level, the name, and the arguments, cut at 27
  characters to 26 and `…`. A session's root is its group line, so its children start
  at level 0, and its own ports and `paused` follow its group line by the same rule; a
  leftover's top is a process line at level 0. A nested session's kind column holds its
  session word (`headless`), which pushes the rest of that line right.
- **Control characters** (C0, DEL, C1) in a name, an argument, a session name, a status,
  or a path are shown as `\n`, `\t`, `\r`, or `\xNN` before the line is measured and
  cut, so no value can break a line or drive the terminal. `--json` leaves escaping to
  JSON.
- **Units.** CPU% has one decimal and `%`; memory is whole MiB below 1 GiB (`612M`),
  else GiB with one decimal and `.0` dropped (`1.6G`, `16G`), `0` when zero; anything
  unknown is `-`. Ports are `:5173` style, `-` when the sockets could not be read.
- **Nothing found** prints the header, then `No Claude Code session is running.`

## The `--json` document

- **Keys are snake_case** (`schema_version`, `cpu_percent`, `rss_bytes`): the output type
  derives `Serialize` with `#[serde(rename_all = "snake_case")]`, which overrides the
  camelCase default `designing-core-logic` gives a view. snake_case is what the
  requirements spell and what a `jq` filter reads without quoting, and the tools a
  loadtop user pipes beside it (`cargo metadata`, `ps`-style JSON) use it.
- **One compact document on one line.** `--json` writes the document without
  whitespace between tokens, then one `\n`, so a script reads it as one line and the
  first bytes are always `{"schema_version":1,`. Keys appear in the order of the literal
  below.
- **The shape, as a literal.** `crates/loadtop-core/tests/serialization.rs` pins this
  document's shape against core's `JsonReport`, and `cli.rs` pins the built binary's
  top-level keys (values invented):

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

  A process's `kind` is `"tool"`, `"mcp"`, or `null` when its env is unreadable or
  carries no marker; `command` is the argument vector joined by spaces, `null` when it
  is unreadable; `ports` is the listening ports ascending, `[]` when the read found no
  listener, and `null` when the sockets could not be read (an empty array would claim
  there is none); `state` is `"running"` or `"stopped"`; `cpu_percent` is `null` when it
  cannot be computed; `project` is home-relative (`~/…`), and it, `name`, `status`, and
  `closed_session_id` are `null` when unknown; a session's `kind` is `"terminal"`,
  `"desktop"`, or `"headless"`; `tier` is `"confirmed"` or `"probable"`. Sessions and
  leftovers each keep the heaviest-first order by CPU. A nested session is a process in its
  parent's `children` whose `session` holds `kind`, `name`, `status`, and
  `session_ids`; a session whose parent session ended is a top-level entry.
  `start_time` is Unix seconds. Unreadable is `null`, never `"-"` or `0`.
- **`schema_version` is the first key and starts at 1.** A field removed, renamed, or
  retyped bumps it, with a `CHANGELOG.md` line; a field added does not. The JSON is its
  own type in core (`JsonReport`-shaped), built from the sample, never the screen's
  view, so the screen's layout can change without touching the contract.
