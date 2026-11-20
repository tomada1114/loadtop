# loadtop — Design direction

- **Status:** Decided 2026-11-20
- **Related:** `docs/design/ux-guidelines.md` (color and emphasis table, glyphs), `docs/product/ux-flows.md` (layouts)

## Why this stage is short

loadtop is a terminal UI. Its colors are the user's terminal theme: the 16 named ANSI
colors plus modifiers, and no RGB palette of its own (ux-guidelines › Color and
emphasis; a palette or theme of its own is a design change in the template's
`AGENTS.md`). The template ships no `designing-ui` skill and no token files. So there is
no palette to choose from a design system, no lock in a token format, and no contrast
pass: what the user's theme renders is the user's choice. `applying-design-systems`
targets apps and sites with their own palettes, so it was not run.

What remains is the visual grammar of the frame, fixed below so that view tests
(`TestBackend`) can be written against it.

## Visual grammar

| Element | Rule |
|---|---|
| Density | One process per row; no blank rows inside the list; one blank column of left margin |
| Numbers | Right-aligned in fixed-width columns; CPU% with one decimal (`141.2`); memory with 3 significant figures and a unit (`612M`, `1.3G`); `…` while unknown, `-` when unreadable |
| Text | Left-aligned; truncated with `…` at the end (names) or the start (paths) |
| Column header | Dim; not bold, so group rows stay the strongest text |
| Header lines | Plain; values bold, labels normal (`CPU Claude **3.1 cores**`) |
| Group rows | Bold kind word; group totals in the CPU% and MEM columns, aligned with process values |
| Tree | Box drawing `├─ └─ │`, indent 3 per level; the fold arrows `▸ ▾` before group rows only |
| Selection | Reversed video across the full row width |
| Result row and key bar | The result row in normal text; the key bar dims the descriptions and not the keys (`**x** stop`) |
| Overlays | Centered; rounded borders (ratatui `BorderType::Rounded`); width min(64, screen − 4); title in the top border; the action keys right-aligned on the last line, with the default (cancel) last |
| Warnings in dialogs | The line starts with `!` in yellow; the text stays normal |
| Color thresholds | As in ux-guidelines; never on labels other than `leftover`, `leftover?`, `paused`, and the `!` marks |

## README presentation (for the stage 8 issue)

- The README opens with the signed-off text (requirements §1) and then one animated
  capture of F1 (find the culprit, stop it) at 100×30 in a default dark theme. It is
  recorded with a scripted terminal recorder from a fake process table, so it shows no
  real paths or session names.
- Below it, a static capture of a paused process and the quit prompt. Then the
  comparison table.
