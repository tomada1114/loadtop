# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- `loadtop --once` prints every running Claude Code session and leftover, with the
  processes each started, as indented plain text, and exits; nothing is signalled.
- `loadtop --json` prints the same sample as one JSON document on one line
  (`schema_version` 1, snake_case keys); `--json` implies `--once`.
- `--interval <seconds>` sets the screen's refresh interval, from 1 to 10 (default 2);
  given with `--once` or `--json` it is a usage error (exit 2).
- `--once` and `--json` exit 1 with `error: can't read the process list: …` when the
  process table cannot be read, and 1 when `HOME` is unset or stdout cannot be written.

### Changed

- **Breaking:** `loadtop` with no arguments opens the full-screen view; the `tui` and
  `counter` subcommands are removed. Without a terminal on both standard input and
  standard output, bare `loadtop` refuses with an error and exits 1.
- Building loadtop needs Rust 1.95 or later (was 1.90).

### Removed

- The optional `openrouter` Cargo feature and its `loadtop llm ask` subcommand: loadtop
  calls no model and needs no API key. Breaking for a build made with
  `--features openrouter`, which no longer builds; the default build never had the
  subcommand.
