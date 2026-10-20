# Technical Plan - m0-foundation v1

> **Feature:** `m0-foundation`
> **Version:** v1
> **Status:** backlog
> **Spec:** `.sdrive/specs/backlog/m0-foundation/v1/spec.md`
> **Branch:** `feature/m0-foundation`
> **Base:** `main`

## Summary

This plan turns the M0 foundation into working production artifacts:

1. A strict YAML 1.2 core parser in `dado::yaml`, deserialized through `serde`.
2. A self-contained `yaml::Error` with a stable code and byte offset.
3. A GitHub Actions CI workflow for format, clippy, docs, packaging, cross-platform tests, and MSRV 1.85.
4. Contributor, security, issue, and pull request surfaces.
5. MIT and Apache license files.
6. Complete crates.io metadata, a changelog, and a manual release runbook.

The feature stops at **release-ready**. `cargo publish -p dado` is a manual post-merge action documented in `RELEASING.md`.

## Architecture Approach

### Parser pipeline

```
&str
  -> lexer (indentation, tokens, positions)
  -> parser (block and flow structure)
  -> YamlValue tree (IndexMap for mappings)
  -> serde deserializer
  -> T
```

`dado` remains I/O-free (CON-108). The parser never opens a socket, reads a URL, starts a runtime, or writes a file. All input is a borrowed `&str`; all output is a `Result<T, Error>`.

### Error model

`yaml::Error` stays independent of `diag`:

```rust
pub struct Error {
    pub code: &'static str,
    pub message: String,
    pub line: usize,
    pub column: usize,
    pub offset: usize,
}
```

- `line` and `column` are 1-based.
- `offset` is a 0-based byte offset into the input.
- `Display` prints `code: message (line:column, offset N)`.
- The message names the construct and never echoes the scalar value, so configuration secrets are not leaked.
- A caller can build `diag::Span::new(error.offset, error.offset + 1)` without `yaml` depending on `diag`.

### Feature graph

`yaml` stays additive and independent:

```toml
[features]
default = []
yaml = ["dep:serde", "dep:winnow", "dep:indexmap"]
diag = []
graph = []
emit = []
glob = []
```

No feature may enable or disable another (CON-204). `yaml` does not depend on `diag`.

### CI design

One workflow, `.github/workflows/ci.yml`, with:

| Job | Runner | Purpose |
|---|---|---|
| `fmt` | ubuntu-latest | `cargo fmt --all --check` |
| `clippy` | ubuntu-latest | `cargo clippy --workspace --all-features -- -D warnings` |
| `docs` | ubuntu-latest | `cargo doc --workspace --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings` |
| `package` | ubuntu-latest | `cargo package --workspace` |
| `test` | ubuntu, windows, macos | `cargo test --workspace --all-features` |
| `msrv` | ubuntu-latest | Rust 1.85 build and test |

Shared settings:

- Triggers: `push` and `pull_request`.
- Permissions: `contents: read`.
- Concurrency group per workflow and ref, `cancel-in-progress: true`.
- Caching through `Swatinem/rust-cache`.
- Toolchain through `dtolnay/rust-toolchain`.
- Checkout through `actions/checkout`.

The test jobs are CI configuration, not test authoring. `/sdrive:test` writes and runs the tests; this feature supplies the workflow that will execute them.

## Components

| Component | Action | Description |
|---|---|---|
| `catalogue/crates/dado/src/yaml.rs` | modify | Replace the `D0001` scaffold with the lexer, parser, `YamlValue`, scalar resolution, serde deserializer, and error construction. |
| `catalogue/crates/dado/src/lib.rs` | reuse | Keep the feature gates and crate-level lints; update docs only if the public surface changes. |
| `catalogue/crates/dado/Cargo.toml` | modify | Keep existing dependencies and features; add package metadata and docs.rs configuration. |
| `catalogue/Cargo.toml` | modify | Add shared `repository` metadata if inherited by the crate. |
| `.github/workflows/ci.yml` | create | Format, clippy, docs, package, cross-platform test, and MSRV jobs. |
| `CONTRIBUTING.md` | create | Setup, commands, branch and commit rules, SpecDrive workflow, PR checklist. |
| `SECURITY.md` | create | Private advisory path, supported versions, response targets, disclosure policy. |
| `.github/ISSUE_TEMPLATE/bug_report.yml` | create | Bug report form. |
| `.github/ISSUE_TEMPLATE/feature_request.yml` | create | Feature request form. |
| `.github/ISSUE_TEMPLATE/config.yml` | create | Template chooser configuration. |
| `.github/pull_request_template.md` | create | Summary, linked spec or BP, test evidence, checklist. |
| `LICENSE-MIT` | create | MIT license text. |
| `LICENSE-APACHE` | create | Apache-2.0 license text. |
| `RELEASING.md` | create | Manual release runbook for `dado-v0.1.0`. |
| `catalogue/crates/dado/CHANGELOG.md` | create | Keep a Changelog entry for 0.1.0. |

## Data Model

### `YamlValue`

Internal representation, not public API unless a future feature exposes it.

| Variant | Payload | Purpose |
|---|---|---|
| `Null` | none | `null`, `~`, empty document |
| `Bool` | `bool` | `true`, `false` |
| `Integer` | `i64` | decimal, hex, octal, binary |
| `Float` | `f64` | finite decimal floats |
| `String` | `String` | plain, quoted, literal, folded |
| `Sequence` | `Vec<YamlValue>` | block and flow sequences |
| `Mapping` | `IndexMap<String, YamlValue>` | block and flow mappings; insertion order preserved |

### `yaml::Error`

| Field | Type | Action | Notes |
|---|---|---|---|
| `code` | `&'static str` | no_change | Stable code such as `D1001`. |
| `message` | `String` | no_change | Construct name and reason; no scalar values. |
| `line` | `usize` | no_change | 1-based. |
| `column` | `usize` | no_change | 1-based. |
| `offset` | `usize` | add | 0-based byte offset. |

### Flow

1. The lexer scans indentation, line breaks, comments, and tokens, tracking byte offsets.
2. The parser builds `YamlValue`, rejecting unsupported constructs.
3. Scalar resolution applies the YAML 1.2 core schema, excluding `.inf` and `.nan`.
4. The serde deserializer walks `YamlValue` and produces `T`.
5. Any failure becomes `yaml::Error` with a code, message, line, column, and offset.

## API Contracts

None. This feature adds no HTTP endpoint, database query, or network surface (CON-301, CON-108).

The public Rust API change is additive:

```rust
pub fn from_str<T: serde::de::DeserializeOwned>(src: &str) -> Result<T, Error>;
```

with `Error` gaining `offset: usize`.

## Security Considerations

| Requirement | Description |
|---|---|
| NFR-001 | Untrusted YAML is parsed with a depth limit and positioned errors; malformed input must not panic or hang. |
| NFR-002 | Every error carries line, column, and byte offset for precise reporting. |
| NFR-003 | `dado` stays I/O-free, so it cannot exfiltrate data or fetch remote content. |
| NFR-007 | Error messages never echo scalar values; no secret file or token is introduced. |
| NFR-008 | `CARGO_REGISTRY_TOKEN` comes from the environment or GitHub Secrets; it is never committed. |

Controls CON-511, CON-512, CON-514, CON-516, and CON-520 are not applicable to this offline parser; the rationale is recorded in `spec.md` under **Security Baseline**.

## Performance Considerations

| Requirement | Strategy |
|---|---|
| NFR-001 | Depth limit 128; recursive descent is bounded; no unbounded backtracking in the supported grammar. |
| NFR-010 | Cargo dependency caching and concurrency cancellation keep CI practical. |

No benchmark is claimed. A performance claim would require a benchmark feature and measured hardware, which is out of scope.

## Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `serde` 1.0.229 | library | no_change | Existing optional dependency under `yaml`. |
| `winnow` 1.0.4 | library | no_change | Existing optional dependency under `yaml`. |
| `indexmap` 2.14.2 | library | no_change | Existing optional dependency under `yaml`. |
| `actions/checkout` | external | add | CI checkout. |
| `dtolnay/rust-toolchain` | external | add | Stable, MSRV, and components. |
| `Swatinem/rust-cache` | external | add | Cargo caching. |
| GitHub Actions | service | add | CI execution. |
| `crates.io` | service | no_change | Existing publication target; name checked before publish. |

No new Rust library dependency is added.

## Risks

| Risk | Mitigation |
|---|---|
| YAML grammar complexity | Implement the documented subset only; reject the rest with positioned errors; test every edge case through `/sdrive:test`. |
| MSRV 1.85 drift | Dedicated MSRV job; pin or downgrade only with a recorded decision. |
| Name taken on crates.io | Check immediately before publish; stop and ask for a fallback. |
| Irreversible publish | Dry-run, clean `main`, manual runbook, `cargo yank` for rollback. |
| Three-OS CI cost | Cache dependencies and cancel superseded runs. |
| Scope creep | Enforce the out-of-scope list; later work becomes a new feature. |
| Credential leak | `CARGO_REGISTRY_TOKEN` from environment or GitHub Secrets only. |
| Stack overflow | Depth limit 128. |

## Implementation Sequence

| Phase | Name | Depends on | Outcome |
|---|---|---|---|
| P1 | YAML parser core | - | Lexer, parser, value tree, scalar resolution, rejection rules, depth limit |
| P2 | Serde and error surface | P1 | `from_str`, `Error.offset`, `diag` interop, purity and feature checks |
| P3 | CI workflow | - | Format, clippy, docs, package, test matrix, MSRV |
| P4 | Contributor and security surfaces | - | CONTRIBUTING, SECURITY, issue/PR templates, licenses |
| P5 | Package metadata and release | P1, P2 | Metadata, changelog, runbook, package dry-run |
| P6 | Verification and handoff | P1-P5 | fmt, clippy, docs, package, governance validation, manual checks |

P3 and P4 can run in parallel with P1/P2. P5 depends on the parser and metadata. P6 runs last.

## Task Summary

| Phase | Tasks |
|---|---|
| P1 | T1.1 to T1.6 |
| P2 | T2.1 to T2.4 |
| P3 | T3.1 to T3.3 |
| P4 | T4.1 to T4.4 |
| P5 | T5.1 to T5.4 |
| P6 | T6.1 to T6.5 |

## Test Boundary

This plan does **not** create tasks that write, add, update, or run tests. Test authoring and test execution belong to `/sdrive:test`. The CI workflow is a production artifact that will execute the tests once they exist.

Verification tasks are limited to formatter, linter, documentation build, packaging, governance validation, and manual content checks.

## Verification Plan

| Check | Command |
|---|---|
| Format | `cargo fmt --all --check` |
| Lint | `cargo clippy --workspace --all-features -- -D warnings` |
| Docs | `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps` |
| Package | `cargo package --workspace` |
| Publish dry-run | `cargo publish -p dado --dry-run` |
| Governance | `node .sdrive/scripts/validate-governance.js` |
| Encoding | `node .sdrive/scripts/check-encoding.js` |
| Manual | Confirm CI triggers, issue templates, release runbook, and license files |

## Traceability

`traceability.json` is updated with the plan section and task IDs for every FR and NFR. NFR-009 (every AC has a test) remains a `/sdrive:test` handoff and has no implementation task in this feature.

## Gate 2

Plan and tasks draft complete. Review and approve to start implementation.
