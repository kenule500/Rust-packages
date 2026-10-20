---
title: M0 Foundation - dado 0.1.0 and repository baseline
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# M0 Foundation - dado 0.1.0 and repository baseline

## Overview

### Problem Statement

The repository has one crate, `dado`, with four implemented modules (`diag`, `graph`, `emit`, `glob`) and a YAML parser that returns `D0001`. There is no CI, no test suite, no contributor or security documentation, no release runbook, and no license files. External developers cannot install a working YAML parser from this repository, and maintainers cannot verify or publish changes repeatably.

This blocks every later milestone: `wirekit` depends on `dado`, and all public crates depend on the CI, contributor, and release baseline defined here.

### Proposed Solution

Deliver the M0 foundation as one feature:

1. Finish the strict YAML 1.2 core parser behind the `yaml` feature.
2. Keep `dado` I/O-free, dependency-light, and lint-clean.
3. Add a GitHub Actions CI workflow covering format, clippy, tests, docs, packaging, and MSRV 1.85.
4. Add `CONTRIBUTING.md`, `SECURITY.md`, issue templates, and a pull request template.
5. Add `LICENSE-MIT` and `LICENSE-APACHE`.
6. Add `RELEASING.md`, a `dado` changelog, complete package metadata, and a verified `cargo publish -p dado --dry-run`.
7. Leave the actual `dado` 0.1.0 publish as a documented manual step from `main` after review and archive.

### Target Audience

- **Primary:** external Rust developers who need strict YAML parsing into typed structs with precise diagnostics.
- **Secondary:** maintainers and contributors to the GIA Packages workspace.

### Assumptions

- `dado` remains the crate name. `https://crates.io/api/v1/crates/dado` returned 404 on 2026-10-20. The name MUST be checked again immediately before publication (CON-111).
- The workspace remains rooted at the repository root with members `crates/*` (CON-101).
- Rust 1.85 remains the MSRV (CON-102).
- GitHub is the source host and CI platform.
- The supported YAML subset in `crates/dado/README.md` is authoritative.
- No CLA and no DCO are required for contributions.
- The 0.1.0 release is manual, not automated.
- Test authoring belongs to `/sdrive:test`, not this feature's implementation tasks.

### Confirmed Decisions

| Decision | Choice |
|---|---|
| Feature scope | BP-001, BP-002, BP-003, BP-004 in one `m0-foundation` feature |
| YAML scalars | Core scalars with finite numbers; reject `.inf` and `.nan` |
| Error model | `yaml::Error` stays independent of `diag`; add `offset: usize` |
| Publish timing | Release-ready now; publish from `main` after merge and review |
| CI matrix | Stable on Linux, Windows, macOS; MSRV 1.85 on Linux |
| Contribution policy | No CLA, no DCO |
| Release automation | Manual runbook |

### Sources

- `crates/dado/README.md`
- `crates/dado/src/yaml.rs`
- `Cargo.toml`
- `.sdrive/constitution.md`
- `.sdrive/product/backlog.md` (BP-001 to BP-004)
- YAML 1.2.2 specification: <https://yaml.org/spec/1.2.2/>
- YAML 1.2 core schema: <https://yaml.org/spec/1.2.2/#103-core-schema>
- Cargo publishing: <https://doc.rust-lang.org/cargo/reference/publishing.html>
- Keep a Changelog: <https://keepachangelog.com/en/1.1.0/>
- GitHub Actions: <https://docs.github.com/en/actions>
- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/checklist.html>

## User Stories

### US-001: Parse strict YAML into typed Rust values

As an external Rust developer, I want to parse YAML configuration into typed Rust structs with positioned errors, so that invalid configuration fails clearly instead of being silently guessed.

**Acceptance Criteria:**

- AC-001: Given a document with block mappings and sequences, plain and quoted scalars, single-line flow collections, block scalars (`|`, `>` with chomping), comments, and multi-line plain scalars, when `dado::yaml::from_str::<T>` is called, then it returns the expected typed value.
- AC-002: Given `null`, `~`, `true`, `false`, decimal, hexadecimal, octal, and binary integers, and finite floats, when parsed into matching Rust types, then the values are correct.
- AC-003: Given the strings `yes`, `no`, `on`, and `off`, when parsed into a string field, then they remain strings.
- AC-004: Given a duplicate mapping key, when parsed, then a positioned error is returned and no value is produced.
- AC-005: Given a tab used for indentation, when parsed, then a positioned error is returned.
- AC-006: Given a tag, directive, anchor, alias, merge key, multi-document stream, or complex key, when parsed, then a positioned error names the unsupported construct.
- AC-007: Given `.inf` or `.nan` in any casing, when parsed, then a positioned error is returned.
- AC-008: Given a YAML value that does not match `T`, when deserialization runs, then a positioned error is returned instead of a panic.
- AC-009: Given nesting deeper than the depth limit, when parsed, then a positioned error is returned instead of a stack overflow.
- AC-010: Given an empty or comment-only document, when parsed into `Option<T>` or a unit-like type, then it succeeds; when parsed into a type that rejects null, then it returns a positioned error.

### US-002: Render precise diagnostics

As a developer integrating `dado` with an editor or CI, I want stable error codes and byte offsets, so that I can render a precise caret and annotate the failing construct.

**Acceptance Criteria:**

- AC-011: Given a parse error, when `Display` is used, then the output includes the stable code, message, line, column, and byte offset.
- AC-012: Given a parse error's offset, when converted to a `diag::Span` and rendered with `diag::render`, then the caret underlines the failing construct.
- AC-013: Given a parse error, when `Debug` or `Display` is printed, then it does not echo the full source or any scalar value, so secrets in configuration are not leaked.

### US-003: Enforce the quality baseline in CI

As a maintainer, I want every push and pull request to run format, clippy, tests, docs, packaging, and MSRV checks, so that regressions are caught before review.

**Acceptance Criteria:**

- AC-014: Given a push or pull request, when GitHub Actions runs, then format, clippy, test, docs, package, and MSRV jobs execute.
- AC-015: Given a formatting violation, when the format job runs, then it fails.
- AC-016: Given a clippy warning, when the clippy job runs with `-D warnings`, then it fails.
- AC-017: Given the workspace, when `cargo test --workspace --all-features` runs, then all tests pass.
- AC-018: Given Rust 1.85, when the MSRV job runs, then the workspace builds and tests pass.
- AC-019: Given a documentation warning, when `cargo doc --workspace --all-features --no-deps` runs with `RUSTDOCFLAGS="-D warnings"`, then the job fails.
- AC-020: Given a packaging problem, when `cargo package --workspace` runs, then CI fails before any publish.
- AC-021: Given a newer commit on the same pull request, when a CI run is already in progress, then the older run is cancelled.

### US-004: Contribute and report vulnerabilities safely

As a contributor or security reporter, I want clear contribution and disclosure instructions, so that my work or report arrives through the correct path.

**Acceptance Criteria:**

- AC-022: Given `CONTRIBUTING.md`, when a contributor reads it, then it states prerequisites, build and test commands, branch naming, commit convention, the SpecDrive workflow, and a pull request checklist.
- AC-023: Given `SECURITY.md`, when a reporter reads it, then it states GitHub private advisories, supported versions, response targets, and a no-public-disclosure request.
- AC-024: Given a new issue, when the user chooses bug or feature, then the matching issue template appears.
- AC-025: Given a new pull request, when it opens, then the pull request template asks for a summary, linked backlog or spec, test evidence, and a checklist.
- AC-026: Given the repository root, when a user checks licensing, then `LICENSE-MIT` and `LICENSE-APACHE` exist and match `MIT OR Apache-2.0`.

### US-005: Release dado 0.1.0 repeatably

As a maintainer, I want a manual release runbook and a publish-ready package, so that the first publish is safe, verifiable, and repeatable.

**Acceptance Criteria:**

- AC-027: Given `RELEASING.md`, when followed, then it covers pre-flight checks, version and changelog, dry-run, tag naming, publish, verification, and yank or rollback.
- AC-028: Given a clean `main` after merge, when `cargo publish -p dado --dry-run` runs, then it succeeds.
- AC-029: Given the published crate, when a new project runs `cargo add dado@0.1.0`, then the crate resolves and the YAML example compiles.
- AC-030: Given the published release, when the repository is inspected, then tag `dado-v0.1.0` exists and the changelog entry is present.
- AC-031: Given `RELEASING.md`, when a token is required, then it documents `CARGO_REGISTRY_TOKEN` as an environment or CI secret and contains no committed token.

### US-006: Publish a complete crate page

As an external developer, I want complete crate metadata and docs.rs configuration, so that I can find, trust, and navigate the crate.

**Acceptance Criteria:**

- AC-032: Given the published package metadata, when inspected, then `repository`, `readme`, `keywords`, `categories`, `license`, and `rust-version` resolve correctly.
- AC-033: Given `cargo package -p dado --list`, when run, then the package includes `Cargo.toml`, `README.md`, `src/**`, and the license files.
- AC-034: Given the published crate, when docs.rs builds it, then it builds with all features enabled.

### US-007: Keep the crate safe and dependency-light

As a security-conscious developer, I want `dado` to remain I/O-free, feature-independent, and lint-clean, so that I can embed it in any tool without pulling in a runtime or network stack.

**Acceptance Criteria:**

- AC-035: Given the `dado` dependency tree, when inspected, then it contains no networking, filesystem, or async runtime dependency.
- AC-036: Given each feature, when built independently with `--no-default-features --features <feature>`, then it compiles, and `yaml` does not require `diag`.
- AC-037: Given the crate root, when compiled, then `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` are present.

## Requirements

### Functional Requirements

| ID | Requirement | Linked ACs |
|---|---|---|
| FR-001 | `dado::yaml::from_str<T: serde::de::DeserializeOwned>(src: &str) -> Result<T, yaml::Error>` MUST parse the supported YAML subset into `T`. | AC-001, AC-002, AC-008 |
| FR-002 | The parser MUST support block mappings and sequences, single-line flow collections, plain and quoted scalars, block scalars (`|`, `>` with chomping), comments, and multi-line plain scalars. | AC-001 |
| FR-003 | The parser MUST support `null`, `~`, `true`, `false`, decimal, hexadecimal, octal, and binary integers, and finite floats. It MUST reject `.inf` and `.nan`. | AC-002, AC-007 |
| FR-004 | `yes`, `no`, `on`, and `off` MUST parse as strings, not booleans. | AC-003 |
| FR-005 | Tags, directives, anchors, aliases, merge keys, multi-document streams, complex keys, tab indentation, duplicate keys, and unterminated constructs MUST produce a positioned error with a stable code. | AC-004, AC-005, AC-006 |
| FR-006 | `yaml::Error` MUST expose `code`, `message`, `line`, `column`, and `offset`, and MUST implement `Display` and `std::error::Error`. | AC-011, AC-013 |
| FR-007 | The parser MUST enforce a default nesting depth limit of 128 and return a positioned error when exceeded. | AC-009 |
| FR-008 | The parser MUST NOT perform I/O, spawn a runtime, read URLs, or write files. | AC-035 |
| FR-009 | The `yaml` feature MUST remain additive and independent of `diag`, `graph`, `emit`, and `glob`. | AC-036 |
| FR-010 | The crate MUST retain `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`. | AC-037 |
| FR-011 | The crate MUST build with `--no-default-features` and with each feature enabled independently. | AC-036 |
| FR-012 | The repository MUST include a GitHub Actions workflow that runs format, clippy, test, docs, package, and MSRV jobs on push and pull request. | AC-014 |
| FR-013 | CI MUST run stable Rust on ubuntu, windows, and macos, and Rust 1.85 on ubuntu. | AC-014, AC-018 |
| FR-014 | CI MUST use concurrency cancellation per ref. | AC-021 |
| FR-015 | The repository MUST include `CONTRIBUTING.md`, `SECURITY.md`, `.github/ISSUE_TEMPLATE/bug_report.yml`, `.github/ISSUE_TEMPLATE/feature_request.yml`, `.github/ISSUE_TEMPLATE/config.yml`, and `.github/pull_request_template.md`. | AC-022, AC-023, AC-024, AC-025 |
| FR-016 | The repository MUST include `LICENSE-MIT` and `LICENSE-APACHE`. | AC-026 |
| FR-017 | The repository MUST include `RELEASING.md` and `crates/dado/CHANGELOG.md`. | AC-027, AC-030 |
| FR-018 | The published package metadata MUST resolve `repository` to the GitHub URL, include `readme`, `keywords`, `categories`, `license`, and `rust-version`, and set `[package.metadata.docs.rs] all-features = true`. | AC-032, AC-033, AC-034 |
| FR-019 | The release runbook MUST cover pre-flight, version and changelog, `cargo publish --dry-run`, tag naming, publish, verification, and yank or rollback. | AC-027, AC-028, AC-031 |
| FR-020 | The release runbook MUST document `CARGO_REGISTRY_TOKEN` as an environment or CI secret and MUST NOT include a real token. | AC-031 |
| FR-021 | The release tag MUST be `dado-v0.1.0`. | AC-030 |

### Non-Functional Requirements

| ID | Requirement | Linked ACs |
|---|---|---|
| NFR-001 | The parser MUST treat all input as untrusted and MUST NOT panic, abort, or hang on malformed input. It MUST return a positioned error. | AC-006, AC-008, AC-009 |
| NFR-002 | Every parse error MUST include line, column, and byte offset. | AC-011, AC-012 |
| NFR-003 | `dado` MUST remain I/O-free: no sockets, URLs, runtime, or file writes. | AC-035 |
| NFR-004 | The crate MUST compile on Rust 1.85 with edition 2021. | AC-018 |
| NFR-005 | The crate MUST keep `forbid(unsafe_code)` and `deny(missing_docs)`, and docs MUST build with warnings denied. | AC-019, AC-037 |
| NFR-006 | Features MUST be additive; enabling `yaml` MUST NOT enable or disable another feature. | AC-036 |
| NFR-007 | Error messages and logs MUST NOT echo scalar values or credentials. The feature MUST NOT introduce secret files or committed tokens. | AC-013, AC-031 |
| NFR-008 | Release and CI credentials MUST come from environment variables or GitHub Secrets only. | AC-031 |
| NFR-009 | Every acceptance criterion MUST be covered by at least one test, supplied by `/sdrive:test`. | AC-001 through AC-037 |
| NFR-010 | CI MUST use dependency caching and concurrency cancellation to keep runs practical. | AC-014, AC-021 |

## UI/UX & Data Requirements

### UI/UX

None. This repository is a library workspace and has no application UI (CON-301). The developer-facing surfaces are the crate API, rustdoc, the README, CI output, and error messages.

### Data

- **Input:** a UTF-8 `&str` YAML document.
- **Intermediate:** an ordered YAML value tree using `indexmap` for mappings.
- **Output:** any `T: DeserializeOwned` via `serde`.
- **Error:** `code: &'static str`, `message: String`, `line: usize`, `column: usize`, `offset: usize`.
- **Storage:** none. `dado` does not persist or transmit data.
- **Data flow:** source text -> lexer and parser -> YAML value tree -> `serde` deserializer -> `T`.

### Public API Shape

```rust
pub fn from_str<T: serde::de::DeserializeOwned>(src: &str) -> Result<T, Error>;

pub struct Error {
    pub code: &'static str,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}
```

## Edge Cases & Error States

| ID | Case | Expected Behavior |
|---|---|---|
| EC-001 | Empty or comment-only document | Parses as null; succeeds for `Option<T>` or unit-like types, otherwise returns a type-mismatch error. |
| EC-002 | Duplicate mapping key | Positioned error with a stable code; no value produced. |
| EC-003 | Tab used for indentation | Positioned error naming tab indentation. |
| EC-004 | Unterminated single- or double-quoted scalar | Positioned error at the opening quote. |
| EC-005 | Unterminated flow collection | Positioned error at the opening bracket or brace. |
| EC-006 | Block scalar with `|`, `>`, `|-`, `|+`, `>-`, or `>+` | Correct trailing-newline behavior for the chomping indicator. |
| EC-007 | Multi-line plain scalar | Folds line breaks into spaces according to the documented subset. |
| EC-008 | Inline comment versus `#` inside a scalar | `foo # comment` strips the comment; `foo#bar` and quoted `"#"` preserve the hash. |
| EC-009 | CRLF and lone CR line endings | Line and column numbers remain correct. |
| EC-010 | UTF-8 BOM at the start | A single leading BOM is skipped; it is not part of the first scalar. |
| EC-011 | Nesting deeper than 128 | Positioned depth-limit error, not a stack overflow. |
| EC-012 | `.inf` or `.nan` in any casing | Positioned error; no float is produced. |
| EC-013 | Malformed numeric literal (`0x`, `0o8`, `0b2`, `1_000`) | Positioned error; no silent fallback to a string. |
| EC-014 | Flow collection spanning multiple lines | Positioned error; only single-line flow collections are supported. |
| EC-015 | Multi-document stream (`---` or `...`) | Positioned error; only one document is supported. |
| EC-016 | Tag, directive, anchor, alias, or merge key | Positioned error naming the unsupported construct. |
| EC-017 | Complex key (`? key`) | Positioned error; only scalar keys are supported. |
| EC-018 | Trailing content after the document | Positioned error; trailing comments and whitespace are allowed. |
| EC-019 | Non-string mapping key deserialized into a string-keyed map | Positioned error explaining the key type mismatch. |
| EC-020 | Empty mapping `{}` or sequence `[]` | Parses successfully as an empty collection. |
| EC-021 | Scalar `yes`, `no`, `on`, or `off` | Remains a string. |
| EC-022 | Very long scalar | Parses within memory proportional to input; no arbitrary truncation. |

## Out of Scope

- `wirekit`, its provider modules, and all other crates in the workspace.
- Changes to `diag`, `graph`, `emit`, or `glob` beyond documentation and tests required by the baseline.
- YAML tags, anchors, aliases, merge keys, directives, multi-document streams, complex keys, multi-line flow collections, `.inf`, `.nan`, and numeric underscores.
- YAML serialization or emission.
- `from_slice` or `from_reader` APIs.
- Benchmarks, fuzzing, and property tests authored by this feature. `/sdrive:test` owns test authoring.
- Automated publishing from CI, `cargo-release`, or `release-plz`.
- `CODE_OF_CONDUCT.md`, a CLA, or a DCO.
- A documentation site, Turborepo, or JavaScript tooling.
- Publishing `dado` from the feature branch. Publish is a manual post-merge step.

## Dependencies & Risks

### Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `serde` 1.0.229 | library | no_change | Already optional under the `yaml` feature. |
| `winnow` 1.0.4 | library | no_change | Already optional under the `yaml` feature. |
| `indexmap` 2.14.2 | library | no_change | Already optional under the `yaml` feature. |
| `actions/checkout` | external | add | CI checkout action. |
| `dtolnay/rust-toolchain` | external | add | Stable, MSRV, and component installation. |
| `Swatinem/rust-cache` | external | add | Cargo dependency caching. |
| `crates.io` | service | external | Publication target; name check required immediately before publish. |
| GitHub Actions | service | external | CI execution. |

No new Rust library dependencies are planned.

### Risks

| ID | Risk | Mitigation |
|---|---|---|
| RISK-001 | YAML grammar complexity causes parser bugs or ambiguous behavior. | Implement the documented subset only; reject everything else with a positioned error; test every edge case in this specification. |
| RISK-002 | MSRV 1.85 breaks because a dependency raises its minimum Rust version. | Run an MSRV CI job on every push; pin or downgrade a dependency only with a recorded decision. |
| RISK-003 | `dado` is taken on crates.io before publication. | Check the name immediately before publish (CON-111); if taken, stop and ask for a fallback name. |
| RISK-004 | Publishing is irreversible and a bad release cannot be deleted. | Require `cargo publish --dry-run`, a clean `main`, review, and a manual runbook; use `cargo yank` for rollback. |
| RISK-005 | Three-OS CI increases cost and runtime. | Use dependency caching and concurrency cancellation; keep the matrix to stable plus one MSRV job. |
| RISK-006 | Scope creeps into `wirekit` or other crates. | Enforce the out-of-scope list; later work becomes a new feature. |
| RISK-007 | Release or CI credentials leak. | Use `CARGO_REGISTRY_TOKEN` from the environment or GitHub Secrets; never commit `.env` or tokens. |
| RISK-008 | Deeply nested input overflows the stack. | Enforce a depth limit of 128 and return a positioned error. |

## Security Baseline

This feature touches user input (YAML) and release credentials, so the baseline is expanded below. Controls that do not apply to an offline parser are recorded with a rationale rather than silently skipped.

| Control | Baseline Status | Feature Applicability | Artifact / Rationale |
|---|---|---|---|
| CON-501 | REQUIRED | Applicable | No secret or private file is read, modified, or committed. |
| CON-502 | REQUIRED | Applicable | The feature does not touch `.env`, `*.pem`, `*.key`, `credentials.json`, `config/secrets/*`, or `.git/config`. |
| CON-503 | REQUIRED | Applicable | Ignored files remain private; CI does not print ignored files. |
| CON-504 | REQUIRED | Applicable | No `.env` file is created. The release runbook documents environment variables instead. |
| CON-505 | REQUIRED | Applicable | No token, key, or password is committed, including in examples or tests. |
| CON-511 | REQUIRED | Not applicable to this feature | No authentication flow. Provider authentication belongs to `wirekit` (BP-005 and later). |
| CON-512 | REQUIRED | Not applicable to this feature | No tenants, capabilities, or authorization decisions. |
| CON-513 | NOT APPLICABLE | Not applicable | No HTTP server or response pipeline. |
| CON-514 | REQUIRED | Not applicable to this feature | The parser performs no network calls, so there is nothing to rate-limit. |
| CON-515 | REQUIRED | Applicable | NFR-001, NFR-002, FR-005, FR-007, and EC-001 through EC-022 define strict input validation. |
| CON-516 | REQUIRED | Not applicable to this feature | No database, ORM, or query surface. |
| CON-517 | NOT APPLICABLE | Not applicable | No HTML rendering. |
| CON-518 | NOT APPLICABLE | Not applicable | No browser session or cookie state. |
| CON-519 | REQUIRED | Applicable | NFR-007, NFR-008, FR-020, and AC-031 require environment or secret-store credentials only. |
| CON-520 | REQUIRED | Not applicable to this feature | No HTTP client or server in this feature. |

## Traceability

`.sdrive/governance/m0-foundation/v1/traceability.json` maps every functional and non-functional requirement to its acceptance criteria. Plan sections and task IDs are added after Gate 1 approval. Test IDs are filled by `/sdrive:test`.

## Gate 1

Specification and traceability draft complete. Review and approve to proceed to plan and tasks.
