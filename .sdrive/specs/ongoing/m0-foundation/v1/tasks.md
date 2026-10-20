# Tasks - m0-foundation v1

> **Feature:** `m0-foundation`
> **Version:** v1
> **Branch:** `feature/m0-foundation`
> **Spec:** `.sdrive/specs/ongoing/m0-foundation/v1/spec.md`
> **Plan:** `.sdrive/specs/ongoing/m0-foundation/v1/plan.md`

## Test Boundary

This task list contains **no test-writing or test-running tasks**. Test authoring and test execution belong to `/sdrive:test`. The CI workflow created in P3 will run tests once `/sdrive:test` adds them.

Verification tasks are limited to formatter, linter, documentation build, packaging, governance validation, and manual content checks.

## P1 - YAML parser core

Depends on: none

- [x] **T1.1 - Define the internal YAML value tree**
  - Description: Add `YamlValue` with `Null`, `Bool`, `Integer`, `Float`, `String`, `Sequence`, and `Mapping` variants. Use `IndexMap<String, YamlValue>` for mappings so insertion order is preserved. Keep the type internal.
  - Requirements: FR-001, FR-002
  - Acceptance criteria: AC-001, AC-002

- [x] **T1.2 - Implement block structure lexing and parsing**
  - Description: Parse block mappings and sequences, indentation, comments, blank lines, and multi-line plain scalars. Track byte offsets for every token. Reject tab indentation and inconsistent indentation with a positioned error.
  - Requirements: FR-002, FR-005
  - Acceptance criteria: AC-001, AC-005

- [x] **T1.3 - Implement flow collections, quoted scalars, and block scalars**
  - Description: Parse single-line flow sequences and mappings, single- and double-quoted scalars with escapes, and literal and folded block scalars with `-` and `+` chomping. Reject multi-line flow collections.
  - Requirements: FR-002
  - Acceptance criteria: AC-001

- [x] **T1.4 - Implement YAML 1.2 core scalar resolution**
  - Description: Resolve `null`, `~`, `true`, `false`, decimal, hexadecimal, octal, and binary integers, and finite floats. Keep `yes`, `no`, `on`, and `off` as strings. Reject `.inf`, `.nan`, and malformed numeric literals with a positioned error.
  - Requirements: FR-003, FR-004
  - Acceptance criteria: AC-002, AC-003, AC-007

- [x] **T1.5 - Reject unsupported constructs**
  - Description: Detect and reject tags, directives, anchors, aliases, merge keys, multi-document streams, complex keys, duplicate keys, and unterminated quotes or flow collections. Every rejection carries a stable code and a position. Messages name the construct but never echo a scalar value.
  - Requirements: FR-005
  - Acceptance criteria: AC-004, AC-005, AC-006

- [x] **T1.6 - Enforce the depth limit and trailing-content rules**
  - Description: Track nesting depth and return a positioned error above 128. Allow trailing comments and whitespace; reject any other trailing content. Keep recursion bounded.
  - Requirements: FR-007, NFR-001
  - Acceptance criteria: AC-009

## P2 - Serde and error surface

Depends on: P1

- [x] **T2.1 - Implement `from_str` and the serde deserializer**
  - Description: Implement `from_str<T: serde::de::DeserializeOwned>` by walking `YamlValue` through a `serde::Deserializer`. Map type mismatches to positioned errors. Empty and comment-only input resolves to null.
  - Requirements: FR-001
  - Acceptance criteria: AC-001, AC-008

- [x] **T2.2 - Extend `yaml::Error` with offset and redaction**
  - Description: Add `offset: usize`; implement `Display` as `code: message (line:column, offset N)` and keep `std::error::Error`. Ensure `Debug` and `Display` do not echo the full source or any scalar value.
  - Requirements: FR-006, NFR-002, NFR-007
  - Acceptance criteria: AC-011, AC-013

- [x] **T2.3 - Provide `diag` interop without a feature dependency**
  - Description: Document and verify that `diag::Span::new(error.offset, error.offset + 1)` renders the correct caret. Do not add `diag` to the `yaml` feature.
  - Requirements: FR-006, FR-009
  - Acceptance criteria: AC-012, AC-036

- [x] **T2.4 - Preserve purity, feature independence, and lints**
  - Description: Keep `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`. Verify `--no-default-features` and each feature independently. Confirm no network, filesystem, or runtime dependency is added.
  - Requirements: FR-008, FR-009, FR-010, FR-011, NFR-003, NFR-006
  - Acceptance criteria: AC-035, AC-036, AC-037

## P3 - CI workflow

Depends on: none

- [x] **T3.1 - Create the workflow shell**
  - Description: Add `.github/workflows/ci.yml` with `push` and `pull_request` triggers, `contents: read` permissions, a concurrency group per workflow and ref, and `cancel-in-progress: true`.
  - Requirements: FR-012, FR-014
  - Acceptance criteria: AC-014, AC-021

- [x] **T3.2 - Add format, clippy, docs, and package jobs**
  - Description: Add jobs for `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo doc --workspace --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings`, and `cargo package --workspace`. Install the required components through `dtolnay/rust-toolchain`.
  - Requirements: FR-012
  - Acceptance criteria: AC-015, AC-016, AC-019, AC-020

- [x] **T3.3 - Add the OS matrix, MSRV job, and caching**
  - Description: Add stable-Rust jobs on ubuntu, windows, and macos, plus a Rust 1.85 job on ubuntu. Use `Swatinem/rust-cache` and `actions/checkout`. Configure each job to invoke the workspace test command; do not add or modify test files.
  - Requirements: FR-012, FR-013, NFR-004, NFR-010
  - Acceptance criteria: AC-014, AC-017, AC-018, AC-021

## P4 - Contributor and security surfaces

Depends on: none

- [x] **T4.1 - Write `CONTRIBUTING.md`**
  - Description: Document prerequisites, build and test commands, branch naming (`feature/<name>`), commit convention (`specdrive(<command>):`), the SpecDrive proposal and apply workflow, and a pull request checklist. State that no CLA or DCO is required and contributions are MIT OR Apache-2.0.
  - Requirements: FR-015
  - Acceptance criteria: AC-022

- [x] **T4.2 - Write `SECURITY.md`**
  - Description: Document GitHub private vulnerability reporting, supported versions (0.1.x), acknowledgement and triage targets, the no-public-disclosure request, and what to include in a report. Do not include a personal email unless the user supplies one.
  - Requirements: FR-015
  - Acceptance criteria: AC-023

- [x] **T4.3 - Add issue and pull request templates**
  - Description: Add YAML issue forms for bug reports and feature requests, an issue template config, and a pull request template. The PR template asks for a summary, linked BP or spec, test evidence, and a checklist.
  - Requirements: FR-015
  - Acceptance criteria: AC-024, AC-025

- [x] **T4.4 - Add license files**
  - Description: Add `LICENSE-MIT` and `LICENSE-APACHE` with the standard texts. They must match the workspace `MIT OR Apache-2.0` declaration.
  - Requirements: FR-016
  - Acceptance criteria: AC-026

## P5 - Package metadata and release

Depends on: P1, P2

- [x] **T5.1 - Complete crate and workspace metadata**
  - Description: Add `repository` at the workspace level if inherited, and ensure the crate resolves `repository`, `readme`, `keywords`, `categories`, `license`, and `rust-version`. Add `[package.metadata.docs.rs] all-features = true`.
  - Requirements: FR-018
  - Acceptance criteria: AC-032, AC-034

- [x] **T5.2 - Add the dado changelog**
  - Description: Create `catalogue/crates/dado/CHANGELOG.md` in Keep a Changelog format with a `0.1.0` entry covering the first YAML parser, diagnostics offset, and baseline release.
  - Requirements: FR-017
  - Acceptance criteria: AC-030

- [x] **T5.3 - Write `RELEASING.md`**
  - Description: Document pre-flight checks, version and changelog, `cargo publish -p dado --dry-run`, tag `dado-v0.1.0`, the publish command, post-publish verification, and yank or rollback. Document `CARGO_REGISTRY_TOKEN` as an environment or CI secret; never include a real token.
  - Requirements: FR-017, FR-019, FR-020, FR-021
  - Acceptance criteria: AC-027, AC-028, AC-030, AC-031

- [x] **T5.4 - Confirm package contents and publish dry-run**
  - Description: Inspect `cargo package -p dado --list` and confirm `Cargo.toml`, `README.md`, `src/**`, and the license files are included. Run `cargo publish -p dado --dry-run` and record the result in the release checklist.
  - Requirements: FR-018, FR-019
  - Acceptance criteria: AC-033, AC-028

## P6 - Verification and handoff

Depends on: P1, P2, P3, P4, P5

- [x] **T6.1 - Run the formatter check**
  - Description: Run `cargo fmt --all --check` and fix any formatting differences.
  - Requirements: NFR-005
  - Acceptance criteria: AC-015

- [x] **T6.2 - Run clippy with warnings denied**
  - Description: Run `cargo clippy --workspace --all-features -- -D warnings` and fix all findings.
  - Requirements: NFR-005
  - Acceptance criteria: AC-016

- [x] **T6.3 - Build the documentation with warnings denied**
  - Description: Run `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps` and fix all findings.
  - Requirements: NFR-005
  - Acceptance criteria: AC-019

- [x] **T6.4 - Verify packaging and governance**
  - Description: Run `cargo package --workspace`, `cargo publish -p dado --dry-run`, `node .sdrive/scripts/validate-governance.js`, and `node .sdrive/scripts/check-encoding.js`. Confirm the working tree is on `feature/m0-foundation`.
  - Requirements: FR-018, FR-019
  - Acceptance criteria: AC-020, AC-028

- [x] **T6.5 - Perform manual content checks**
  - Description: Confirm the CI triggers and job names, issue and PR templates, license files, release runbook, and changelog entry are present and internally consistent. Do not run the test suite; that belongs to `/sdrive:test`.
  - Requirements: FR-012, FR-015, FR-017
  - Acceptance criteria: AC-014, AC-022, AC-023, AC-024, AC-025, AC-026, AC-027

## Gate 2

Plan and tasks draft complete. Review and approve to start implementation.
