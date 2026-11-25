# UX guidelines

| | |
|---|---|
| Product | loadtop |
| Platforms | macOS terminal emulators (Terminal.app, iTerm2, Ghostty and the like), full-screen TUI; plus plain text and JSON on stdout |
| App type | Developer tools: a process monitor with actions |
| Related | Requirements: `docs/product/requirements.md` · UX flows: `docs/product/ux-flows.md` (screens, keys, flows: not restated here) |
| Status | Decided 2026-11-20 |

## Principles

1. **The heaviest thing is the first row.** Order, folding and color all serve finding
   the culprit in one look.
2. **Nothing is signaled without a confirmation that names what is hit.** The one
   exception is resume, which undoes a pause and loses nothing.
3. **A paused process is never out of sight.** No fold, filter or scroll position hides
   one, and quitting asks about the ones this run paused.
4. **The terminal is never taken hostage.** Every state has a way out by `Esc`, `q` or
   `Ctrl-C`, and the terminal is restored on every exit path (the template's
   `building-tuis`).
5. **Every number says its unit.** CPU% is a share of one core; the header's CPU is in
   cores; memory is RSS with a unit suffix.

## Navigation

- One screen (S1) with modal overlays (S2–S6). At most one overlay is open at a time.
  `Esc` closes it and returns the selection to where it was.
- `Esc` steps back one level and never quits: it closes an overlay, ends the filter
  input (restoring the previous filter), or clears an active filter, in that order.
- The filter input (S1f) is a text mode, not an overlay: the list stays live behind it
  and keeps refreshing, and only `Ctrl-C` among the list's keys still acts.
- While an overlay is open, the list behind it is frozen: sampling continues, but order
  and values do not change under it. A confirmation shows the values from when it
  opened; the start-time check at signal time (requirements §3.4) covers the gap.
- No mouse in v1: loadtop does not capture the mouse, so the terminal's own text
  selection keeps working (copying a PID).

## Platform conventions

- Raw mode on the alternate screen; the terminal is restored on quit, on error and on
  panic (`building-tuis` › "The terminal's lifecycle"). It refuses to start the TUI when
  stdin or stdout is not a terminal, and points to `--once`/`--json` instead.
- `Ctrl-C` quits, the same as `q`, including the quit prompt's rules (ux-flows S4). In
  an overlay, `q` closes or cancels it like `Esc`, and `Ctrl-C` cancels it and starts the
  quit. Below 80×24 only `q` and `Ctrl-C` act.
- Minimum 80×24. Any resize redraws at once; below the minimum, the too-small line is
  shown (ux-flows S7).
- Text uses the terminal's font. Glyphs are limited to box drawing (`├─ └─ │`), the
  fold arrows `▸ ▾`, `…`, `·`, `—`, the key glyphs `⏎ ←→`, and the eight block elements
  `▁▂▃▄▅▆▇█` of the trend (requirements §3.8). Box drawing, block elements, `…`, `·`,
  `—` and the arrows are East Asian Ambiguous width (observed with Python's
  `unicodedata.east_asian_width`, 2026-11-20): a terminal set to draw ambiguous
  characters wide breaks the columns for all of them alike, and that setting is not
  supported. There are no emoji, since their width differs between terminals and
  breaks the columns.

## States

| State | Trigger | Shows | Primary action | Copy pattern |
|---|---|---|---|---|
| First frame | Launch; CPU% needs a second read | Tree and memory at once; CPU% `…` for about 300 ms | — | — |
| No session | No live Claude Code session | The empty message (ux-flows S7); leftovers if any | `q` | "No Claude Code session is running." |
| Leftovers only | No session, with leftovers | The list, leftovers only | `x` | — |
| Field unreadable | Env or cwd not readable (platform binary, exited) | `-` in that cell, dimmed | — | — |
| Process gone | Exits between refreshes | Row removed on the next refresh; selection moves (below) | — | — |
| Paused process exited | Its session ended while it was paused | Result row note | — | "91204 exited while paused" |
| Not targetable | `x`/`z` on loadtop or an ancestor | Result row reason; no dialog | — | "can't stop loadtop's own session tree" |
| Too small | Below 80×24 | One centered line | Resize | "Terminal too small: 80×24 needed (now 72×20)" |
| Process table unreadable | The sampling call fails | TUI: restore the terminal, print one stderr line, exit nonzero. `--once`/`--json`: the same | — | "error: can't read the process list: <reason>" |

## Feedback and loading

| Rule | Value |
|---|---|
| Refresh | Every 2 s by default (`--interval 1–10`). No spinner; the values changing is the feedback |
| Selection across refreshes | Follows the selected process by PID and start time. If it is gone, the selection moves to its parent row, else to the row now at the same index |
| First sample | Two reads 300 ms apart, so CPU% is real within about 0.3 s of launch |
| Order | By the mean CPU% (or memory) of the last 3 samples; the CPU% column shows the latest. The header says `sort CPU (6s avg)` |
| Signal actions | The result row shows `stopping N…` within 100 ms of `y`, then the result once SIGKILL escalation finishes, at most 3.5 s later |
| Quitting during a stop | `q` or `Ctrl-C` while a SIGKILL escalation is pending shows `finishing stop… (Ctrl-C again to kill now)` and quits once it has run, at most 3 s later; a second `Ctrl-C` sends SIGKILL to the survivors at once. Either way the quit then follows S4's rules. The confirmation's "SIGKILL after 3 s" is never dropped |
| `Ctrl-C` twice | A second `Ctrl-C` within 1 s always means "now": in S4 it leaves the processes paused and quits; during a stop's grace it kills the survivors at once |
| Result line | Stays until the next key press; it is never auto-dismissed, so a result is not missed during the 2 s redraws |
| Confirmations | Every stop and pause confirms, defaulting to No (`Enter` alone cancels). The session-root variants confirm with `Y`, not `y` |
| Resume | No confirmation for what this run paused; a confirmation (ux-flows S3r) when any target was stopped by another tool. The result goes to the result row |
| Error placement | Per action → the result row. Fatal → exit with a stderr line, after restoring the terminal |

## Motion

None. No animation, blinking, or fades. The only change over time is the redraw on
each refresh.

## Color and emphasis

Only the terminal's 16 named ANSI colors and text modifiers are used, so colors follow
the user's own terminal theme. There is no palette of the tool's own (no RGB values),
and no theme (requirements Non-goals). Color always repeats something the text already
says.

| Element | Text that carries the meaning | Style |
|---|---|---|
| Selected row | — (the cursor) | Reversed |
| Group rows (session, leftover) | The kind word | Bold |
| Applies to | Process rows and group totals alike |  |
| CPU% ≥ 50 (of one core) | The number | Yellow |
| CPU% ≥ 100 | The number | Red |
| MEM ≥ 500M | The number | Yellow |
| MEM ≥ 1G | The number | Red |
| Confirmed leftover | `leftover` | Red, bold |
| Probable leftover | `leftover?` | Yellow, bold |
| Paused | `paused` / `paused 4m` | Cyan |
| Not targetable | `self` | Dim |
| Unreadable field | `-` | Dim |
| Dialog warnings | the leading `!` | Yellow |
| Trend | — (supplementary; the row's numbers carry the value) | None |

- The table applies to the TUI only. `--once` and `--json` print plain text always, on
  a terminal as well as in a pipe or a file, since their output is data a person may
  paste or a script may read (`designing-clis` › "stdout is data").
- In the TUI, no color when `NO_COLOR` is set (any value, per no-color.org) or when
  `TERM=dumb`.
- Thresholds are fixed in v1; there are no settings (requirements Non-goals).

## Language and copy

English only, in the binary's `wording.rs` and the view's label constants (the
template's rule). Sentence case, no trailing period on result-row messages, and numbers with
units.

| Use | Don't use | For |
|---|---|---|
| stop | kill (except naming SIGKILL itself) | ending a process tree |
| pause / resume | suspend, freeze, sleep | SIGSTOP / SIGCONT in UI copy (the README may say "put it to sleep") |
| leftover | orphan, zombie | a process a closed session left behind ("zombie" means something else) |
| started by a closed session | garbage, junk | the confirmed-leftover label |
| session | agent, conversation | a running Claude Code instance |
| `stopped 3 · already gone 1 · failed 1 (permission)` | prose sentences | action results |

## Accessibility targets

| Target | This product |
|---|---|
| Keyboard | Every action has a key, listed in `?` help from the same key table the code uses (`building-tuis`); no mouse-only or hidden chords; no key needs `fn` |
| Focus | One selection cursor, shown by reversed video, never by color alone |
| Color use | Never the only carrier: every colored element repeats a word or number (table above). Usable in a monochrome terminal and with `NO_COLOR` |
| Text scaling | The terminal's font size; layout reflows to any size ≥ 80×24 (ux-flows S1 narrowing order) |
| Motion | None to reduce |
| Screen readers | A full-screen TUI redraws constantly and reads poorly in VoiceOver. The accessible path is `--once` (plain, indented text) and `--json`; the README says so |
| Contrast | Not measured: the colors are the user's terminal theme, not ours. A contrast pass applies only if a palette of our own is ever added, which is a design change |

## Decision log

| Date | Decided | Rejected | Why |
|---|---|---|---|
| 2026-11-20 | The terminal's 16 named colors, sparingly, always repeating text | Bold and reverse only | Heavy rows should stand out at a glance; named colors follow the user's theme, so it is not a theme of our own. Recorded for stage 7: the template's `building-tuis` says "the terminal's own foreground and background", so this is a deliberate, small widening |
| 2026-11-20 | Re-sort every refresh, with the selection following the PID; freeze while an overlay is open | Freeze order for 3 s after a key; a pause-the-view key | Matches htop; the top row stays the heaviest; a frozen dialog prevents acting on a moved row |
| 2026-11-20 | After review: resuming something loadtop did not pause confirms; the target is re-resolved at confirm; a 300 ms first sample; order by a 3-sample mean; a fixed result row and key bar; paused rows never folded | Resume always without a dialog; a 2 s blank first frame; instantaneous order | A job stopped on purpose must not be resumed by accident; the top row should not flap; `building-tuis` wants every action named on screen |
| 2026-11-20 | No mouse in v1 | Wheel scroll and click to select | Keeps the terminal's text selection for copying PIDs |
| 2026-11-20 | Box drawing for the tree, words for states, no emoji | ASCII only; emoji marks | Every macOS terminal draws box characters; emoji widths break columns |
| 2026-11-20 | A `/` filter that never changes what an action hits, and a 10-sample CPU trend on group rows | No filter; a trend per process; a trend in its own column | Many sessions need narrowing; an action still hits the whole subtree and the confirmation counts hidden members, so the filter never changes what is signaled. Group rows had empty cells for the trend; a trend per process or a column of its own costs width the 80-column layout does not have |

## Open items

- Open: whether the CPU and MEM color thresholds feel right on the author's Air.
  Settled by a week of dogfooding before the first release.
