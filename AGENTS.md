# Project Guide

Everything an agent needs is in this repository; a cloud session starts with nothing
else. This file holds what every agent needs before it knows its task: what the app is,
how to check a change, where code goes, and which decisions need a human. It keeps no
second copy of the rest: the conventions of one kind of change are a skill's
([Skills](#skills)), the rules for one kind of file load by path ([Rules](#rules)), and
a value a gate enforces is its config's, where running the gate is how you learn it.

## Overview

This is a Rust command-line tool built from a strict template: one binary, `loadtop`,
which opens a full-screen ratatui view when run bare and prints one sample with
`--once` or `--json`, both over the same core (`designing-clis`). It targets macOS and
also builds and tests on Linux, and is installed from its checkout with
`cargo install --path`; there is no release pipeline. How it is built and why lives in
the skills and in [Architecture](#architecture), never in a separate architecture or
roadmap document (`updating-docs`).

## Product

This section is the one part of this file about the application rather than the
harness: without it, an agent implementing an issue here has no in-repo answer to "is
this in scope?". The owner writes each bullet (the `starting-an-app` skill says how);
`just check-harness` fails while one still holds its `TODO` marker.

- **What it is, and who it is for** — `top` for Claude Code sessions on macOS. It lists
  every running Claude Code session (terminal, desktop, and headless) with the processes
  it started, heaviest first, by project. It marks what closed sessions left behind,
  and stops or pauses a whole process tree in one confirmed action. It is for
  developers on a laptop with little headroom who keep several sessions open; dev
  servers, watchers, MCP servers and builds pile up, and neither Activity Monitor nor
  htop says which session or project they belong to. It calls no LLM and needs no API
  key. The requirements are in `docs/product/requirements.md`, the screens and keys in
  `docs/product/ux-flows.md`, and the app-wide UX policy in `docs/design/ux-guidelines.md`.
- **The core interaction** — run `loadtop`, see the heaviest session or leftover in the
  first row, select it, and stop its tree (SIGTERM, then SIGKILL after 3 s) or pause it
  (SIGSTOP, later SIGCONT), after a confirmation that names what is hit. A person works
  in the full-screen view; `--once` and `--json` print one sample and exit, for scripts
  and screen readers, and never signal anything.
- **Non-goals** — tokens, context, quota or cost; stopping anything automatically (no
  daemon, hook, or LaunchAgent); waking a paused server when a connection arrives; load
  history on disk (a later release); Codex or other agents (a later release); stop,
  pause or resume commands without the on-screen confirmation; a guarantee for Windows
  or Linux; cleaning up simulators, containers, or automation browsers; themes, a
  palette of its own, or a settings screen; listing or acting on processes Claude Code
  did not start (only their total load is shown); other users' or root's processes, and
  any `sudo` path; showing environment variable values other than the session ID.
- **Why it is shaped this way** — the pain is "which session is making my Mac slow, and
  what did closed sessions leave behind", so the unit is the session and its tree, not
  the process. htop shows processes one at a time and signals one PID at a time; loadtop
  groups by session and acts on whole trees. It never signals on its own, and every
  stop or pause confirms, because a tool that kills on a guess is worse than none; the
  session process, loadtop, and loadtop's ancestors are protected by default. It must
  itself stay light (at most 2% of one core and 30 MB at the default 2 s refresh), since
  its users are short on headroom: so there is no resident recorder, and history is
  left for later. Ownership comes from what Claude Code already leaves on each process
  (documented environment markers, plus its session records when they validate), not
  from a daemon of loadtop's own.

## Quick Reference

```bash
just install  # Pinned tools (mise) and lefthook's git hooks
# Cloud: the setup script and SessionStart hook run provision.sh instead
bash .claude/cloud/provision.sh session  # By hand in a multi-repo session or project thread
just verify-hooks  # Fail when the pre-commit hook is missing
just fmt
just fix  # rustfmt's fixes only
just lint  # rustfmt check, clippy -D warnings
just test  # test-core + test-xtask
just test-core  # Core with its coverage floors, and the Linux-buildable crates
just test-xtask  # The xtask crates with their floors
just test-fast increment  # One core test or a group, no floor
just test-platform  # Adapter and CLI tests against the real OS
just test-scripts  # Skills' Python suites and shellcheck
just check-harness  # The harness's claims about itself
just logs  # Print the end of the newest app log
just deny  # cargo deny
just check  # Every check a machine runs without a human
just clean
just prune-temp  # Remove leftover temp dirs and idle scratchpads
# A human's recipes: only when asked, never in a cloud session
just test-local  # The #[ignore]d tests
just logs-follow  # Never ends
just install-cli  # Install the binary into ~/.cargo/bin
# Writes to GitHub or rewrites the repository: a human's step
just labels  # Create/update labels from .github/labels.yml
just ruleset  # Create/update .github/rulesets/*.json (admin-only)
```

## Validating a change

Run the narrowest check that can fail, then `just check` before you open a PR. None
opens a window or takes focus.

| What you changed | The narrowest check that can fail |
|---|---|
| `crates/loadtop-core/` | `just test-fast <filter>`, then `just test-core` (the floors) and `just lint` |
| `crates/loadtop-platform/` | `just test-platform`; an `#[ignore]`d test needs the human's `just test-local` |
| `crates/loadtop-test-support/`, `crates/loadtop/` | `just test-core`, then `just test-platform`; the terminal loop needs the human's `loadtop tui` run |
| `xtask/`, a harness check | `cargo nextest run -p xtask -p xtask-guard`, then `just test-xtask`, `just lint`, `just check-harness` |
| `lefthook.yml` | `just test-xtask`, then `just verify-hooks` |
| A recipe, skill, workflow, label, issue form, ruleset, `## Product`, core's forbidden-crate lists (`deny.toml`'s `wrappers`), `.claude/settings.json` | `just check-harness`; for a workflow also `mise exec -- actionlint` and `mise exec -- zizmor .`, for a skill's scripts `just test-scripts` |
| Markdown, `.github/labels.yml`, an issue form | `mise exec -- typos <file>` |
| `Cargo.toml`, `Cargo.lock`, `deny.toml` | `just deny`, `mise exec -- cargo shear`, `just lint`, `just test` |
| `mise.toml`, `rust-toolchain.toml` | `mise install`, then `just check` |

## Architecture

| Crate | Role |
|---|---|
| `loadtop-core` | Logic, state, and the ports (traits) to everything outside the process; coverage-gated |
| `loadtop-platform` | Adapters for core's ports against the real OS: translation only |
| `loadtop-test-support` | A fake and a `<port>_contract` function per port; dev-dependency only |
| `loadtop` | The binary and composition root: translates and decides nothing; `src/wording.rs` holds every string a user reads, `src/tui/` the terminal loop and view |
| `xtask`, `xtask-guard` | `cargo xtask <task>` automation and the staged guard; never shipped |

- New logic goes in `loadtop-core` with tests: a decision found in an adapter or the
  binary belongs in core, where the coverage floor sees it.
- Dependencies point one way: `loadtop-platform` → `loadtop-core`, and both ← `loadtop`. Core
  never names clap, ratatui, or crossterm.
- A port is a synchronous `Send + Sync` trait core declares, `loadtop-platform`
  implements, the binary constructs, and a test fakes. Core never meets async. Ports:
  process table, sockets, session records, project folders, signals, clock.
- The core boundary is enforced three times, so removing one layer leaves the others:
  core's `Cargo.toml` lists no OS or platform crate; `deny.toml`'s `[bans]`
  `wrappers` let only `loadtop` depend on `loadtop-platform`; and a harness check fails
  when core's normal and build dependency closure reaches `objc2*`,
  `core-foundation*`, `security-framework*`, or `loadtop-platform`. Those lists change
  together, and `just check-harness` fails when they differ.
- I/O, time, the environment, and processes reach core only through ports or arguments,
  and core never sleeps or starts a thread that outlives the call:
  `crates/loadtop-core/clippy.toml` lists the banned calls, and `just lint` enforces it.
- Core logs nothing and builds no user-facing sentence.
- `target/` and `coverage/` are build output: never edit or commit them.

### What is contract and what is private

The contract is what something outside a change can observe; everything else (an
adapter's internals, the view's layout, module layout, log wording) is private and
needs only the gates. This is the one list of it:

- **Core's public API**, every `pub` item `crates/loadtop-core/src/lib.rs` re-exports:
  update every caller in the same pull request. A new port is a design change.
- **The data and log locations**, keyed by the bundle identifier `io.github.tomada1114.loadtop`
  (`crates/loadtop-platform/src/paths.rs`, the justfile's `bundle_id`) and the XDG
  directory name `loadtop`: fixed once the tool has run anywhere but your checkout, since
  a new name leaves the user's data behind. A human's decision, and a design change.
- **On-disk file formats** (`integrating-system-apis`' `references/file-formats.md`):
  a new version still reads the old one, with a format version, a migration, and a test
  that reads a sample of the previous format.
- **The command line** (`designing-clis`), its subcommands, flags, stdout and stderr,
  and exit codes: keep the old form working, or say the change is breaking in
  `CHANGELOG.md`.

Only the first (the compiler) and the last (`crates/loadtop/tests/cli.rs`) are guarded:
a changed format or identifier passes every check and fails on the user's machine, so
review catches it. A user-visible change to any owes a `CHANGELOG.md` entry.

## Before changing the architecture

The design is recorded once per subject, as it stands now: in the skill that owns it,
in [Architecture](#architecture) for the layer rules and the contract, and in Product
for scope. A design change, any of the below, is hard to reverse, so its pull request
also rewrites that skill or section, which the owner reviews with the change: the
current design and its reasoning (what it protects and, where it matters, the
alternative it rules out and why), in the present tense, replacing the old sentences,
never as a dated entry:

- a new crate in the workspace, a new dependency (runtime or dev), or a new port in core;
- where and in what format the tool keeps state, or a settings file or environment
  variable it reads;
- a new target platform, or dropping one;
- distribution: a release workflow, prebuilt or signed binaries, a package-manager
  tap, crates.io publishing, an updater;
- `rust-version` in `Cargo.toml`;
- an OS privacy permission (on macOS, a TCC grant);
- `unsafe` code, which means lifting `unsafe_code = "forbid"` for `loadtop-platform`;
- the bundle identifier (`io.github.tomada1114.loadtop`) or the XDG
  directory name (`loadtop`);
- a second language for the tool's wording, or a TUI theme beyond the terminal's colors;
- replacing clap or ratatui, a clap major, or ratatui 1.0 and its later majors (a
  pre-1.0 ratatui or crossterm minor is `merging-dependency-prs`' migration instead).

Writing the reasoning down grants nothing: the sign-off "Security and human approval"
asks for is still needed.

## Skills

Skills live in `.claude/skills/`, the one tree: edit a skill there, in place.

| Skill | Load it for |
|---|---|
| `authoring-skills` | Writing or editing a skill |
| `smart-commit` | Committing; a refused pre-commit hook |
| `create-pr` | Opening or updating a pull request |
| `triaging-issues` | Filing or labelling an issue |
| `shipping-issues` | Taking open issues to merged pull requests |
| `merging-dependency-prs` | Landing Dependabot and Renovate PRs |
| `managing-dependencies` | Adding or changing a crate |
| `changing-gates` | A file that enforces; the enforcement layers |
| `updating-docs` | Which document a change must update |
| `writing-repo-scripts` | A `cargo xtask` task or a skill's script |
| `starting-an-app` | The app's first decisions; GitHub settings |
| `writing-rust` | Rust idioms, compiler messages, clippy |
| `tdd` | Red-green-refactor |
| `writing-tests` | The body of one Rust test |
| `placing-tests` | Where a test goes; its coverage floor |
| `designing-core-logic` | Core logic: ports, transitions, views |
| `designing-clis` | A subcommand, its output, logging |
| `building-tuis` | The `loadtop tui` screen |
| `designing-errors` | Error enums, wording, `ERR_*` codes |
| `integrating-system-apis` | OS calls in `loadtop-platform`; file formats |
| `running-the-app` | Seeing a change work |

### Rules

Each loads while you touch a file its `paths:` globs match, and restates nothing a gate
enforces.

| Rule | Loads when you touch |
|---|---|
| `.claude/rules/rust.md` | `crates/**/*.rs` |
| `.claude/rules/testing.md` | `crates/*/tests/**`, `crates/loadtop-test-support/**` |
| `.claude/rules/project.md` | manifests, lockfiles, tool pins, and gate configs |
| `.claude/rules/docs.md` | `docs/**/*.md`, `README.md`, `CONTRIBUTING.md`, `CHANGELOG.md` |
| `.claude/rules/scripts.md` | `xtask/**`, `.claude/skills/*/scripts/**`, `.claude/hooks/**`, `.claude/cloud/**` |

### Sub-agents

`.claude/agents/` defines the tiers `executor`, `architect`, `scout`, and `worker`:
each file carries its model, effort, and instructions (its description says what it
takes), is maintained here by hand, and shadows a same-named agent in
`~/.claude/agents/`. Delegate by `subagent_type`, never a bare `model`; prose names
tiers by role only.

## Security and human approval

Only what is mechanically decidable is blocked at commit time: the staged guard
(`cargo xtask check-staged`) refuses a secret-shaped path or credential-shaped content.
Whether a commit *should* contain what it contains stays in PR review.

Never read a secret-shaped file, even to check it: `.env`, `.env.*`, or `.envrc.*`
(the `.example`/`.sample`/`.template` samples excepted), anything under a `secrets/`
directory, `*.p12`, `*.pfx`, `*.p8`, `*.provisionprofile`, `*.mobileprovision`,
`*.keychain`/`*.keychain-db`, `*key*.pem`, `private-key.*`, an SSH private key
(`id_rsa`, `id_ed25519`, …), `.netrc`, `credentials.json`, and `secrets.json`: the
list `xtask/guard/src/paths.rs` refuses to commit (with `.claude/settings.local.json`,
which is fine to read). If a task seems to need one, ask the human for the non-secret
fact instead.

Get a human's sign-off before any of these. This section is the rule itself, not a
check: the committed `.claude/settings.json`, which personal files add to, stops a few
of them in Claude Code only (the permission-list gap in `changing-gates`'
`references/enforcement-layers.md`).

- Adding a release pipeline or anything it would need (a workflow that builds or
  uploads artifacts, signing or notarization, a repository secret, publishing to
  crates.io or a package-manager tap), or creating or pushing a release tag.
- Editing a personal permission file (`.claude/settings.local.json` or
  `~/.claude/settings.json`) unless the owner asks for it in that session: it decides
  what an agent may run, and no review ever sees it.
- Adding a new crate (the dependency policy is in `.claude/rules/project.md`).
- Weakening any gate: lowering a coverage floor, relaxing a lint, widening a workflow's
  `permissions:`, or, in the committed `.claude/settings.json` (a gate file, changed
  only in a reviewed PR), loosening a `deny` or `ask` or widening `allow`. If a gate
  looks wrong, say so and let a human decide. So
  is any of these, used to make a failing check pass: `#[allow(…)]` or
  `#[expect(…)]` on a lint; `unsafe`, or lifting `unsafe_code = "forbid"`; `#[ignore]`
  or `skip` on a failing test; excluding a file from coverage, or adding a path to an
  ignore list (`typos.toml`, `deny.toml`, `osv-scanner.toml`); deleting or loosening an
  assertion; `continue-on-error` in CI; `git commit --no-verify`.
- Working around a command a permission file, a hook, or a human denied: re-spelling
  it (`git -C . …`, `bash -c '…'`, bundled short flags such as `-anm`, an alias or
  script wrapper) is forbidden. The same holds for any instruction here that blocks
  something that looks necessary: fix what made the bypass look necessary, or ask;
  never look for another spelling.
- Any write to a remote: `git push`, a `gh api` write, or any other. `just labels` and
  `just ruleset` only create or update what `.github/labels.yml` and
  `.github/rulesets/*.json` declare, and each still needs sign-off before its first run
  against the live repository (`starting-an-app`' `references/github-settings.md`).

Standing exceptions: invoking one of these skills is the sign-off for the remote
writes that skill exists to make, for that invocation only.

- `smart-commit`, when asked to push: pushing the commits it made to the current branch.
- `create-pr`: pushing the current branch and creating or updating its pull request.
- `shipping-issues`: adding a missing `priority:` tier label to an open issue; removing
  `blocked: dependency` from issues its merge unblocked; for a design-held issue it
  takes on, posting its `## Design decision` comment and then clearing `blocked: design`
  and the ship contract's `design=open`; creating and pushing its own branches (in a
  cloud session, only the session's designated branch); creating the pull request; on
  that open PR alone, appending `Closes #<n>` when missing or retargeting its base to
  the default branch; merging it once CI passes for the current head; closing, with a
  back-reference comment naming the PR, the issue that merged PR was meant to close when
  GitHub left it open; filing and labelling follow-up issues; and removing the local
  branches and worktrees it created — never a force-push, a remote branch deletion, a
  reply to or resolution of a review thread, a comment requesting a review, or Auto-fix.

One request is a standing exception too: the human explicitly asking for an issue
("file an issue for this") is the sign-off for the `gh api -X POST …/issues` call that
creates each issue it asks for, with the labels `triaging-issues` gives it, and for
comments on those issues in the same request. An issue the agent would file from a
friction it noticed on its own, or from one the human raised without asking for an
issue, is drafted in the reply and waits for a yes.

None of them covers anything else: a force push or other history rewrite,
`--no-verify`, weakening a gate, a release pipeline or signing, a release tag, a new
dependency, `just labels`, or `just ruleset`. A skill that reaches one stops and asks.

### Never taking over the machine

Checks run on the owner's own machine, often beside an unattended agent. Nothing a
routine check runs — `just check` and every recipe in it, the pre-commit hook, a
PostToolUse hook, any step an agent runs to verify its work — may show a window, take
keyboard focus, move the pointer, raise a permission, Keychain, or Gatekeeper prompt, or
take over a terminal.

- Never run `loadtop tui` (`cargo run -p loadtop -- tui` included). No check needs a TTY:
  the TUI is tested through ratatui's `TestBackend` (`building-tuis`). When only the
  real terminal shows a change, ask the human to run it (`running-the-app`).
- For evidence, use the tests, `just test-platform`, a plain subcommand run against a
  scratch `HOME` (`running-the-app`), and `just logs`.
- `just test-local`, `just logs-follow`, and `just install-cli` are a human's to start:
  never run one unless asked.
- A test that needs a GUI session, a TCC grant, or the Keychain is
  `#[ignore = "local machine: <what it needs>"]` and runs only in `just test-local`.
- Never trigger an installer or a `sudo` prompt; report the command for the human
  (`running-the-app`).
- A cloud session has no human at the VM: instead of asking, leave it unrun and name it
  in the PR as needing a local run. Never run a human's recipe there, even when asked:
  report that it needs a local machine.

## Enforcement layers

The rules here are held by layers, from mechanical to procedural: lefthook's pre-commit
hook on the staged files (`rustfmt --check`, `typos`, the staged guard); `just check`
(the lints, core's bans and boundary, the coverage floors, the harness checks); CI,
with the same recipes and the security workflows; and this file for the rest.
`changing-gates`' `references/enforcement-layers.md` has each layer's row and the gaps
left open on purpose; a gate change updates it. `guide-size` caps this file at 20,000
bytes outside `## Product`.

### What no local gate sees

Every local layer can be skipped, so these reach `main` only if CI or GitHub stops
them: `git commit --no-verify` or another hook bypass; a clone where `just install`
never ran; an edit made through GitHub's web UI or API; a secret whose path and content
pattern the guard does not know; and any tool other than Claude Code, the
only one `.claude/settings.json` binds. A new repository's admin turns GitHub's side
on once (secret scanning, push protection, the rulesets): `starting-an-app`'
`references/github-settings.md`.

## Review Checklist

Before submitting a PR:

1. `just check` passes
2. New public items have `///` comments explaining *why*
3. Tests cover the happy path AND the error path; a change to an adapter with an
   `#[ignore]`d test carries `just test-local` output
4. No new crate without justification and a human's sign-off
5. User-facing changes have a `CHANGELOG.md` entry under `[Unreleased]`
6. Commits and the PR title follow Conventional Commits, in English

## Important Reminders

- Write all code, docs, commits, and PRs in English
- Do what has been asked; nothing more, nothing less. NEVER create files unless
  absolutely necessary: prefer editing an existing file, and never create a
  documentation file unless asked
- A comment carries only a non-obvious why, a trap, or an external constraint: default
  to none, and keep the rest to a line or two. A `///` on a public item is its contract
  and stays (`writing-rust`)
- A problem found outside the task is recorded, not fixed: file an issue as
  `triaging-issues` says, or, where filing is not yours to do (it is a remote write),
  list it in the pull request description. Never widen the pull request to fix it
