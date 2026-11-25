# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
