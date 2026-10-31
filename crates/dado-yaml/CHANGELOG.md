# Changelog

All notable changes to `dado-yaml` are documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-10-20

The first baseline release of `dado-yaml`.

### Added

- A strict YAML 1.2 core parser behind the `yaml` feature, using `serde` for
  deserialization into application types.
- Block mappings and sequences, single-line flow collections, plain and quoted
  scalars, literal and folded block scalars with chomping, comments, and
  multi-line plain scalars.
- YAML 1.2 core scalar resolution: `null`, booleans, decimal, hexadecimal,
  octal, and binary integers, and finite floats. `yes`, `no`, `on`, and `off`
  remain strings; `.inf` and `.nan` are rejected.
- Positioned errors with stable codes, 1-based line and column, and a 0-based
  byte `offset` suitable for `diag::Span::new(error.offset, error.offset + 1)`.
- A nesting depth limit of 128 for untrusted documents.
- The `diag`, `graph`, `emit`, and `glob` modules, available independently of
  `yaml`.
- GitHub Actions CI for formatting, clippy, documentation, packaging, a stable
  Rust test matrix on Ubuntu, Windows, and macOS, and Rust 1.85 MSRV.
- Contributor, security, issue, and pull request templates, plus the
  `dado-yaml-v0.1.0` release runbook.

### Security

- Rejected unsupported constructs instead of guessing: tags, directives,
  anchors, aliases, merge keys, multi-document streams, complex keys, duplicate
  keys, tab indentation, and unterminated quotes or flow collections.
- Error messages name the failing construct without echoing scalar values.

[0.1.0]: https://github.com/kenule500/Rust-packages/releases/tag/dado-yaml-v0.1.0
