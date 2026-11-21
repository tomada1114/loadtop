# loadtop's visual grammar

What every frame of `loadtop`'s screen looks like, fixed so that a `TestBackend` test can
write its expected buffer by hand. The screens, keys, and wireframes are
`docs/product/ux-flows.md`; the colors, glyphs, and copy are
`docs/design/ux-guidelines.md`. This page is the layout rules between them, and it
changes in the same pull request as `crates/loadtop/src/tui/view.rs` when one moves.

## Why there is no palette, lock, or contrast pass

loadtop is drawn in the user's terminal theme: the 16 named ANSI colors and text
modifiers, no RGB value of its own. A palette or a theme of the tool's own is a design
change (`AGENTS.md` › "Before changing the architecture"). With no palette there is no
token file and no contrast to measure: what the theme renders is the user's choice,
and every colored element repeats a word or a number, so a monochrome or `NO_COLOR`
terminal loses nothing.

## The frame

| Element | Rule |
|---|---|
| Density | One process per row; no blank rows inside the list; one blank column of left margin |
| Numbers | Right-aligned in fixed-width columns; CPU% with one decimal (`141.2`); memory with 3 significant figures and a unit (`612M`, `1.3G`); `…` while unknown, `-` when unreadable |
| Text | Left-aligned; truncated with `…` at the end (names) or the start (paths) |
| Column header | Dim; not bold, so group rows stay the strongest text |
| Header lines | Plain; values bold, labels normal (`CPU Claude **3.1 cores**`) |
| Group rows | Bold kind word; group totals in the CPU% and MEM columns, aligned with process values |
| Trend | Group rows only, starting at the PORTS column, no style. A sample `v` above 0 draws block `ceil(8 · v / top)` of `▁▂▃▄▅▆▇█`, 0 draws a space; `top` and the length are requirements §3.8's. When the group row shows a state word (`paused`, `self`) in STATE, the trend shrinks to the 6 PORTS cells (the last 6 samples) |
| Tree | Box drawing `├─ └─ │`, indent 3 per level; the fold arrows `▸ ▾` before group rows only |
| Folded rows | `+N more` for a fold, `+N hidden` for rows a filter hides, with their totals in the CPU% and MEM columns; a lifted paused row starts with `…` in the tree's place |
| Selection | Reversed video across the full row width |
| Result row and key bar | The result row in normal text; the key bar dims the descriptions and not the keys (`**x** stop`) |
| Filter input | Replaces the result row: `/ ` then the text and a reversed one-cell cursor; the key bar shows the input's keys |
| Overlays | Centered; rounded borders (ratatui `BorderType::Rounded`); width min(64, screen − 4); title in the top border; the action keys right-aligned on the last line, with the default (cancel) last |
| Warnings in dialogs | The line starts with `!` in yellow; the text stays normal |
| Color thresholds | As in ux-guidelines; never on labels other than `leftover`, `leftover?`, `paused`, and the `!` marks |

## Columns

The widths at 80 cells, what a wider terminal does with the rest, and the order in which
a group row truncates are ux-flows S1's notes; the view takes them from there and
`TestBackend` tests at 80 columns pin them. Below 80×24 the frame is the one too-small
line (ux-flows S7), never a clipped list.
