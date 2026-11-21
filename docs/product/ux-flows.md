# loadtop — Screens and Flows

- **Status:** Decided 2026-11-20
- **Source:** `docs/product/requirements.md` (signed off 2026-11-20). Section numbers (§) refer to it.
- **Platform:** a full-screen terminal UI (ratatui), keyboard only, on macOS, plus plain
  text from `--once`. App-wide policy (colors and marks, messages, timing, accessibility)
  is in `ux-guidelines.md` and is not restated here.

## 0. Layout decisions (2026-11-20)

- Sessions and leftover groups form one list, ordered by their tree total. The heaviest
  culprit is always the first row, whatever it is (rejected: leftovers in a first or a
  last block of their own).
- On launch, each session shows its top 3 child subtrees; the rest fold into one
  `+N more` row. `→` expands everything (rejected: fully expanded; session rows only).
- Details (full command line, cwd, session IDs, start time) open in an overlay on
  `Enter`, so the list keeps the full width (rejected: a pane that is always visible).
- Keys are mnemonic letters, and none needs `fn` on a Mac keyboard (rejected: htop's F
  keys).
- Also decided: the session-root
  variants confirm with `Y`; resuming what this run paused needs no dialog; `z` on a
  partly paused subtree pauses the rest; paused rows are never folded away; nested
  sessions are left out of subtree actions; resuming something loadtop did not pause
  confirms.
- Also decided at design time (2026-11-20): `/` filters the list without changing what
  an action hits; group rows carry a 10-sample CPU trend in cells they left empty.

## 1. Screen inventory

| # | Screen | Serves |
|---|---|---|
| S1 | Main view: header totals, the session/leftover list, a result row and a key bar | §3.1–3.3, §3.5 marks, §3.8 trend |
| S1f | Main view while filtering: the input line, then the filtered list | §3.7 |
| S2 | Confirm stop, with three variants: subtree, probable leftover, session root | §3.4 |
| S3 | Confirm pause, with warnings for ports and session children | §3.5 |
| S3r | Confirm resume of processes loadtop did not pause | §3.5 |
| S4 | Quit prompt, shown when something this run paused is still stopped | §3.5 |
| S5 | Detail overlay (`Enter`) | §3.1, §3.2 |
| S6 | Key help (`?`) | all |
| S7 | Empty and too-small states | §3.1 edge cases |
| T1 | `--once` plain text (`--json` is a schema, §3.6, and has no screen) | §3.6 |

## 2. Keys

| Key | Action | Where |
|---|---|---|
| `↑` `↓`, `j` `k` | Move the selection | S1 |
| `PgUp` `PgDn`, `Ctrl-U` `Ctrl-D` | Move by a page | S1 |
| `Home` `End`, `g` `G` | First / last row | S1 |
| `→` `l` / `←` `h` | Expand / collapse the selected row; `→` on `+N more` shows all | S1 |
| `x` | Stop the selected subtree (session row: all its children; the root is excluded) | S1 → S2 |
| `X` | Stop the session including its root (session rows only) | S1 → S2 (root variant) |
| `z` | Pause the selected subtree; resume it if every process in it is paused | S1 → S3; resume directly, or via S3r when something outside loadtop paused it |
| `Z` | Pause or resume the session including its root | S1 → S3 (root variant) |
| `s` | Toggle the sort key between CPU and memory | S1 |
| `/` | Filter the list; `Enter` keeps it, `Esc` restores the previous one | S1 → S1f |
| `Esc` | Clear an active filter | S1 |
| `Enter` | Open the detail overlay | S1 → S5 |
| `?` | Key help | S1 → S6 |
| `q`, `Ctrl-C` | Quit (via S4 when anything this run paused is still stopped) | S1 |
| `y` / `n`, `Esc` | Confirm / cancel; the default is No, so `Enter` alone cancels | S2, S3, S3r |

Resuming what this run paused needs no confirmation: SIGCONT undoes that pause and loses
nothing. A process stopped by something else (Ctrl-Z in a shell, a debugger) was stopped
on purpose, so resuming it confirms (S3r).

## 3. Wireframes

Drawn at the minimum size, 80×24. Values are examples. Overlays are drawn with square
corners here; they render with rounded borders (`building-tuis`' visual grammar).

### S1 Main view

```
loadtop  CPU Claude 3.1 cores · all 5.2 of 8   MEM Claude 2.9G · used 14.2/16G
sort CPU (6s avg) · every 2s · 2 paused                                    1/14
    PID  NAME                              KIND    CPU%    MEM  PORTS STATE
▾ desktop  acme-web · Fix checkout flow · busy    182.0   1.6G  ▂▃▅▇██▇█▇█
  91204  ├─ node vite                      tool   141.2   612M  5173
  91230  │  └─ esbuild --service           tool    38.9    96M
  81006  ├─ npm exec @playwright/mcp       mcp      1.4    88M
         └─ +6 more                                 0.5   310M
▾ leftover  acme-api · closed session 8b41d0e7     96.3   740M  ▄▄▄▄▄▄▄▄▄▄
  52143  └─ zsh -c npm run test:watch      -        0.0     4M
  52150     └─ node vitest --watch         tool    96.3   736M
▸ terminal  ~/code/notes · idle                    12.0   420M  ▁▁▁ ▁▁▁▁▁▁
▾ leftover? code/…/api                              3.1    52M        ▁▁▁
  60311  └─ sh -c ./watch.sh               -        3.1    52M
▸ headless  tmp/spike · busy                        0.4    90M
  71002  …  python -m http.server          tool     0.0    22M  8000  paused 4m

stopped 3 · already gone 1
 x stop  z pause/resume  s sort  ⏎ details  ←→ fold  ? help  q quit
```

- Line 1: Claude Code's total load beside the whole machine's (§3.2 header totals).
  CPU in the header is in cores; the CPU% column is a share of one core (§3.2).
- Line 2: the sort key (with the averaging window), the refresh interval, and the
  paused count. The count covers the listed trees only (sessions and leftovers),
  including processes paused by other tools (§3.5).
- The position indicator `1/14` (selected row / rows) sits at the right of line 2.
- Group rows:
  - Session: kind, project, the session name when the record has one, and `busy`/`idle`.
  - Confirmed leftover: `leftover`, the project, and the closed session's short ID.
  - Probable leftover: `leftover?`. Its wording and mark differ from confirmed (§3.3).
- KIND for processes: `tool`, `mcp`, or `-` when the env is unreadable (§3.1).
- STATE: empty when running; `paused` when stopped; `paused 4m` when this run paused it
  (§3.5).
- Rows that cannot be targeted (loadtop and its ancestors) show `self` in STATE, and `x`
  or `z` on them explains why in the result row.
- The last two rows are fixed. The result row shows the result of the last action
  (§3.4 Result), or the reason a key did nothing, until the next key press. The key bar
  is built from the same key table as `?` help (`building-tuis`).
- Paused rows inside a collapsed group or inside `+N more` are lifted out of the fold
  and shown, prefixed `…` (Principle 3 of ux-guidelines).
- Column widths at 80 columns: PID 6, KIND 5, CPU% 6, MEM 6, PORTS 6, STATE 10; NAME
  takes the rest (about 35). Wider terminals give every extra column to NAME. Ports that
  do not fit show the first one plus `+N`. All columns, KIND included, stay at the 80
  minimum.
- Group rows truncate in this order: the session name (end, `…`), then the project
  (from the left, `…/acme-api`). The kind word, `busy`/`idle` and the totals never
  go.
- Heat colors (ux-guidelines) apply to process rows and to group totals alike.
- The trend (requirements §3.8) sits on group rows in the PORTS and STATE cells, which
  group rows leave empty: 10 block glyphs, newest on the right, one scale for every
  shown group. A group seen for fewer than 10 samples shows only those, right-aligned
  (the probable leftover above appeared 6 s ago). It has no color.

### S1f Filtering

```
loadtop  CPU Claude 3.1 cores · all 5.2 of 8   MEM Claude 2.9G · used 14.2/16G
sort CPU (6s avg) · every 2s · 1 paused · filter "vite" · 1 of 5         1/4
    PID  NAME                              KIND    CPU%    MEM  PORTS STATE
▾ desktop  acme-web · Fix checkout flow · busy    182.0   1.6G  ▂▃▅▇██▇█▇█
  91204  ├─ node vite                      tool   141.2   612M  5173
         └─ +8 hidden                                40.8   1.0G
  71002  …  python -m http.server          tool     0.0    22M  8000  paused 4m

/ vite█
 ⏎ keep  esc undo  ^U clear
```

- While typing, the result row is the input and the key bar shows the input's keys;
  list keys do not act (requirements §3.7). Once kept with `Enter`, the key bar returns
  and line 2 keeps `filter "vite" · 1 of 5` (groups shown of all). Filter text longer
  than 16 characters is cut with `…` on line 2; the input line shows it whole.
- Totals stay whole-tree: the group row says 182.0 although only one child is shown.
- The paused `python -m http.server` does not match and is still shown, lifted with `…`
  (ux-guidelines Principle 3). It counts as shown, not hidden.
- A filter that matches nothing shows `No group matches "vite".` in the list area, with
  any paused process still listed below it.

### S2 Confirm stop

Subtree (the default variant):

```
┌ Stop 3 processes? ───────────────────────────────────────────┐
│ desktop · acme-web · Fix checkout flow  (session stays)       │
│                                                               │
│  91204  node vite              141.2%   612M  :5173           │
│  91230  esbuild --service       38.9%    96M                  │
│  91260  node postcss             0.3%    40M                  │
│                                                               │
│ Total 180.4% · 748M. SIGTERM now; SIGKILL after 3 s.          │
│                                        [y] Stop   [n] Cancel  │
└───────────────────────────────────────────────────────────────┘
```

- Up to 5 processes are listed, heaviest first; beyond that, `and N more` (§3.4).
- With a filter active and members of the target hidden by it, a line says so:
  `Includes 4 processes hidden by the filter.` The count and the list cover them.
- When the target contains a live nested session, a line names it: `Skipped: 1 nested
  session (headless · tmp/spike). Select it to act on it.`
- The count is the one at opening. At `y` the target is resolved again; the result row
  reports what was actually signaled.
- Probable-leftover variant: one extra line: `Owner inferred from the working directory
  — not confirmed by Claude Code's marker.`
- Session-root variant (`X`): the title becomes `End session and stop N processes?`, plus
  the line `The Claude Code session itself (terminal · ~/code/notes) will end.` The confirm
  key becomes `Y` (shift), so a habitual `y` cannot end a session.

### S3 Confirm pause

```
┌ Pause 2 processes? ──────────────────────────────────────────┐
│ desktop · acme-web · Fix checkout flow                        │
│                                                               │
│  91204  node vite        141.2%   612M  :5173                 │
│  91230  esbuild          38.9%     96M                        │
│                                                               │
│ ! :5173 will accept connections but not answer until resumed. │
│ ! The session waits on them; if it ends, they are likely      │
│   killed.                                                     │
│ Resume later with z.                   [y] Pause  [n] Cancel  │
└───────────────────────────────────────────────────────────────┘
```

- The dialog opens at once with the ports from the last read; its port lines update
  when the fresh read it asked for lands (the one exception to the overlay freeze). When
  ports cannot be read, the line says `! Ports could not be read; a listening server
  will hang its clients.`
- The port line appears only when a target listens. The session line appears only when
  the targets belong to a live session (§3.5 Pause confirmation).
- Session-root variant (`Z`): `Pause the whole session, including Claude Code itself?`,
  confirmed with `Y`.

### S3r Confirm resume of processes loadtop did not pause

```
┌ Resume 1 process paused outside loadtop? ────────────────────┐
│  48211  vim notes.md     paused (stopped by another tool)     │
│                                                               │
│ It may have been stopped on purpose (Ctrl-Z, a debugger).     │
│                                       [y] Resume  [n] Cancel  │
└───────────────────────────────────────────────────────────────┘
```

- Shown only when the subtree contains such a process. Processes this run paused are
  resumed with it, without being listed.

### S4 Quit prompt

```
┌ 2 processes you paused are still paused ─────────────────────┐
│  71002  python -m http.server   paused 4m   :8000             │
│  91204  node vite               paused 1m   :5173             │
│                                                               │
│ [r] Resume and quit   [l] Leave paused and quit   [Esc] Back  │
└───────────────────────────────────────────────────────────────┘
```

- Lists only what this run paused and what is still stopped (§3.5). It never lists
  processes paused by other tools.
- `Ctrl-C` here means Back, not quit; a second `Ctrl-C` within 1 s quits and leaves
  them paused. The terminal must never be stuck.

### S5 Detail overlay

```
┌ 91204 node ──────────────────────────────────────────────────┐
│ Command  node /Users/…/acme-web/node_modules/.bin/vite --port │
│          5173 --host 127.0.0.1                                │
│ cwd      ~/code/acme-web                                      │
│ Session  desktop · Fix checkout flow · busy                   │
│          id 3f9c2a10…, also 1f0c9a77… (before /clear)         │
│ Parent   91190 zsh -c npm run dev                             │
│ Started  2026-11-20 09:12:03 (42m ago)                        │
│ Load     CPU 141.2% · MEM 612M · ports 5173                   │
│                                        [x] [z]   [Esc] Close  │
└───────────────────────────────────────────────────────────────┘
```

- The full command line wraps; nothing is truncated here. No environment variable is
  shown except the session ID (§4 Privacy).
- `x` and `z` act on this process's subtree, exactly as on the list.

### S6 Key help

The §2 key table, in an overlay, plus three lines: `CPU% is a share of one core; header
CPU is in cores.`, `MEM is resident memory (RSS); Activity Monitor's Memory counts
differently.`, and `Order uses a 6 s average; the trend shows the last 20 s.` (both
durations follow `--interval`). `Esc` or `?` closes it.

### S7 Empty and too-small states

```
loadtop  CPU Claude 0 cores · all 1.1 of 8   MEM Claude 0 · used 9.8/16G


            No Claude Code session is running.
            Leftovers from closed sessions appear here too.



 ? help  q quit
```

- The result row stays empty, and the key bar shows only the keys that apply.
- With no sessions but leftovers present, the list shows the leftovers alone; this
  message is not shown.
- Below 80×24: one centered line, `Terminal too small: 80×24 needed (now 72×20)`. The
  sampling continues, and the view returns when the terminal grows.
- First frame: the tree and memory appear at once; CPU% shows `…` for about 300 ms,
  until the second read of the first sample (requirements §3.6).

### T1 `--once`

```
$ loadtop --once
CPU Claude 3.1 cores · all 5.2 of 8   MEM Claude 2.9G · used 14.2/16G
desktop  acme-web · Fix checkout flow · busy        182.0%  1.6G
  91204  node vite                    tool  141.2%  612M  :5173
  91230    esbuild --service          tool   38.9%   96M
  81006  npm exec @playwright/mcp     mcp     1.4%   88M
  81118    node cli.js --headless     mcp     0.3%  120M
  90011  zsh -c git status            -       0.0%    4M
leftover acme-api · closed session 8b41d0e7    96.3%  740M
  52143  zsh -c npm run test:watch    -       0.0%    4M
  52150    node vitest --watch        tool   96.3%  736M
```

(The example stops here; real output continues to the last group.)

- Same order as S1, but the full tree with no folding: `--once` is the accessible path
  (ux-guidelines), so it hides nothing. Indentation instead of box-drawing characters,
  so it pastes cleanly.
- No color when stdout is not a terminal.

## 4. Flows

### F1 Find and stop the culprit (the core task)

1. Run `loadtop`. S1 opens; the heaviest group is the first row.
2. Move to the group, or to a child subtree inside it.
3. Press `x`. S2 opens with the targets and their total.
4. Press `y`. The result row shows `stopping 3…`, then the result after at most 3.5 s.

```
[loadtop] -> [S1 heaviest first] -> [select] -> [x] -> [S2] -y-> [SIGTERM] -> [re-scan]
                                                         |                       |
                                                        n/Esc         survivors? -+-> [SIGKILL after 3 s]
                                                         v                       |
                                                       [S1]                      v
                                                                     [result: stopped N · gone M · failed K]
```

Failure branches:
- **Target exited, or its PID was reused, before `y`:** the start-time check skips it;
  the result counts it as `already gone` (§3.4 Identity check).
- **Permission denied:** counted as `failed 1 (permission)`; the rest still stop.
- **Target is loadtop or one of its ancestors:** `x` does not open S2; the result row
  says `can't stop loadtop's own session tree`.

### F2 Pause, then resume

1. Select a subtree, and press `z`. S3 opens with the warnings that apply.
2. Press `y`. The rows show `paused 0m`, and the header count goes up.
3. Later, select it and press `z` again. It resumes at once, with no dialog (S3r first
   if something outside loadtop paused part of it), and the result row says `resumed 2`.

```
[select] -> [z] -> all paused? -yes-> [any paused outside loadtop? -> S3r] -> [SIGCONT] -> [result: resumed N]
                       |
                       no -> [S3] -y-> [SIGSTOP] -> [rows: paused 0m]
                              |
                             n/Esc -> [S1]
```

- **Mixed subtree (some paused):** `z` pauses the rest, via S3. Resuming needs the whole
  subtree to be paused, so `z` never does two things.
- **The session ended while its child was paused:** that row is gone on the next
  refresh. The result row says `91204 exited while paused`, which is the case S3 warned
  about.

### F3 Quit with processes still paused

```
[q] -> anything this run paused still stopped? -no-> [exit 0]
               |
              yes -> [S4] -r-> [SIGCONT each, with start-time check] -> [exit 0]
                        |-l-> [exit 0, left paused]
                        `-Esc-> [S1]
```

### F4 End a whole session (rare)

`X` on a session row → S2 root variant → `Y` → SIGTERM to the root and its tree → the
same 3 s escalation → the group disappears. `X` on a non-session row says, in the result
row, `X works on session rows; use x for a subtree`.

### F5 Scripted read

`loadtop --once` prints T1 and exits 0. `loadtop --json` prints the §3.6 JSON and exits
0. Either exits nonzero only when the process table cannot be read, with one line on
stderr.
