# loadtop

[![CI](https://github.com/tomada1114/loadtop/actions/workflows/ci.yml/badge.svg)](https://github.com/tomada1114/loadtop/actions/workflows/ci.yml)
[![OpenSSF Scorecard](https://api.scorecard.dev/projects/github.com/tomada1114/loadtop/badge)](https://scorecard.dev/viewer/?uri=github.com/tomada1114/loadtop)
[![License: MIT](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)

A personal Rust command-line tool: one binary, `loadtop`, whose clap
subcommands do the work and whose `tui` subcommand opens a full-screen ratatui view over
the same core, built and run on macOS and Linux. It ships as a working counter —
persisted state, an injected clock, subcommands and a terminal view over one core — with
coverage floors, architecture boundaries that fail a build, and supply-chain-hardened
CI, all from the first commit.

It runs on macOS (Apple Silicon) and Linux. Windows, a release
pipeline or release artifacts, crates.io publishing, and localization are
non-goals.

## Quickstart

Prerequisites: macOS on Apple Silicon with the Xcode Command Line Tools
(`xcode-select --install`), or Linux with a C toolchain for the linker (`build-essential`
on Debian and Ubuntu); [rustup](https://rustup.rs/), [mise](https://mise.jdx.dev/), and
[Just](https://just.systems/) (`brew install mise just` on a Mac). A Claude Code cloud
session needs none of this by hand: its environment's setup script and the committed
`SessionStart` hook provision the VM
([Cloud sessions](docs/getting-started.md#cloud-sessions)).

```bash
git clone https://github.com/tomada1114/loadtop.git
cd loadtop
mise trust     # approve mise.toml once (mise asks before using an untrusted config)
just install   # pinned tools via mise and lefthook's git hook
just check     # everything the machine can run without a human; takes over no terminal
cargo run --locked -p loadtop -- counter show   # the tool itself
```

rustup installs the Rust toolchain `rust-toolchain.toml` names the first time `cargo`
runs (`RUSTUP_AUTO_INSTALL`, on by default:
<https://rust-lang.github.io/rustup/environment-variables.html>, checked 2026-09-30).
`just install` needs no `sudo` and opens no installer; a missing Command Line Tools
install is reported with the command to run. `cargo run --locked -p loadtop -- tui` opens
the full-screen view in the terminal you run it from; `q` quits.

## How it is built

- `crates/loadtop-core` — the rules, the state, and the ports (traits) everything outside
  the process is reached through; no OS API, no terminal, no direct I/O, and a coverage
  floor.
- `crates/loadtop-platform` — the adapters that implement core's ports on macOS and Linux.
- `crates/loadtop-test-support` — one fake and one contract function per port, for tests
  only.
- `crates/loadtop` — the `loadtop` binary: its subcommands, the `tui` screen, and the wiring.
- `xtask/` — repository automation in Rust, run as `cargo xtask <task>`.

The layer rules and what is contract are in [AGENTS.md](AGENTS.md)'s Architecture
section. How each part is built, and why, is written in the skill that owns it under
`.claude/skills/` (AGENTS.md's Skills table lists them); there is no separate
architecture document or roadmap.

## Development

See [CONTRIBUTING.md](CONTRIBUTING.md) for the full workflow.

```bash
just install      # once per clone
just check        # the full local gate; takes over no terminal
just test-fast increment   # one core test or a group of them, while iterating
just logs         # the newest app log's last lines
just install-cli  # install the loadtop binary into ~/.cargo/bin (a human's step)
```

`just --list` shows every recipe. The tool's data lives in
`~/Library/Application Support/io.github.tomada1114.loadtop/` and its logs in
`~/Library/Logs/io.github.tomada1114.loadtop/` on macOS; on Linux, in `$XDG_DATA_HOME/loadtop/`
(default `~/.local/share/loadtop/`) and `$XDG_STATE_HOME/loadtop/logs/` (default
`~/.local/state/loadtop/logs/`).

## Documentation

- [Getting Started](docs/getting-started.md)
- [AGENTS.md](AGENTS.md) — the architecture, the gates, and the skills an agent loads
- [Contributing](CONTRIBUTING.md), [Security Policy](SECURITY.md),
  [Code of Conduct](CODE_OF_CONDUCT.md), [Changelog](CHANGELOG.md)

## License

[MIT](LICENSE)
