# Code Standards

> Context file for `.sdrive/constitution.md`.
> Standards marked "observed" are enforced by existing code or configuration;
> "documented" standards are explicit requirements in the design docs; anything
> inferred is labelled `assumption`.

## Observed Project Facts

### Language and toolchain

| Fact | Value | Source |
|---|---|---|
| Edition | 2021 | `Cargo.toml` |
| MSRV | 1.85 | `Cargo.toml` |
| Resolver | 2 | `Cargo.toml` |
| Licence | `MIT OR Apache-2.0` | `Cargo.toml` |
| Installed toolchain | 1.95.0 | `rustc --version` |

### Crate-level lint attributes (observed in `dado`)

```rust
#![forbid(unsafe_code)]
#![deny(missing_docs)]
```

Source: `crates/dado/src/lib.rs`.

### Documentation style (observed in `dado`)

- Every module starts with a `//!` module doc that states its scope and its
  non-goals.
- Every public struct, enum, field, and method has a `///` doc comment.
- Error types document their stable diagnostic codes.
- The crate-level doc lists the feature-gated modules and the crate's I/O
  boundary.

Source: `crates/dado/src/*.rs`.

### Error style (observed in `dado`)

- Errors carry a `&'static str` stable code (`E1001`, `D1001`, etc.).
- Errors implement `core::fmt::Display` and `core::error::Error`.
- The `yaml` scaffold returns `D0001` rather than guessing at a partial document.

Source: `crates/dado/src/diag.rs`,
`crates/dado/src/yaml.rs`.

### Feature style (observed in `dado`)

- `default = []`.
- Each module is gated by a named feature.
- Optional dependencies use `dep:` in the feature list.
- The crate compiles with zero dependencies when no feature is enabled.

Source: `crates/dado/Cargo.toml`.

### Formatting and linting

- No `rustfmt.toml` exists, so `cargo fmt` uses rustfmt defaults.
- No `clippy.toml` exists and no Clippy lint level is set in the manifest.
- No CI workflow runs `cargo fmt --check`, `cargo clippy`, or `cargo test`.

### Tests

| Fact | Value | Source |
|---|---|---|
| Framework | Rust's built-in test harness (`cargo test`) | Rust toolchain; no external test framework in any manifest |
| Run command | `cargo test --workspace --all-features` from the repository root | `Cargo.toml` |
| Unit test location | Inline `#[cfg(test)] mod tests` in the module under test | Rust convention; no counter-example in the repo |
| Integration test location | `crates/<crate>/tests/` | Rust convention; the folder does not exist yet |
| Existing tests | None | `crates/**` |

### Error, retry, and rate-limit contract (documented)

- One shared error enum, `WirekitError`, across every provider.
  Source: `catalogue/01_wirekit.md`.
- Retry policy: exponential backoff with jitter, 3 attempts by default, retry on
  5xx/429/network errors, respect `Retry-After`, never retry other client errors.
  Source: `catalogue/01_wirekit.md`.
- Rate limiting: token bucket per provider, configurable overrides, metrics via
  tracing. Source: `catalogue/01_wirekit.md`.
- Shared HTTP client pool: one `reqwest` client across providers.
  Source: `catalogue/01_wirekit.md`.
- Observability: tracing events for request start/finish, retries, rate-limit
  hits, and errors. Source: `catalogue/01_wirekit.md`.

### Secret handling (documented)

- Stream keys must never appear in `Debug`, `Display`, logs, traces, or error
  messages; `Debug` prints `***`.
  Source: `catalogue/36_rtmp_publish.md`.
- Sage Intacct credentials "must never appear in logs".
  Source: `catalogue/15_sage_intacct.md`.
- Deepgram keys must never be logged. Source: `catalogue/24_deepgram.md`.
- PagerDuty incident payloads should not contain homeowner PII.
  Source: `catalogue/22_pagerduty.md`.
- GIA never stores a Slack user token. Source: `catalogue/23_slack.md`.

### Protocol and payload handling (documented)

- Preserve raw webhook bytes; a JSON re-serialization must fail verification.
  Source: `catalogue/30_webhook_gateway.md`.
- GraphQL APIs may return HTTP 200 with an `errors[]` array; do not treat 200 as
  success. Source: `catalogue/14_jobber.md`, `catalogue/23_slack.md`.
- OAuth uses Authorization Code + PKCE; never use the implicit flow.
  Source: `catalogue/28_google_oauth.md`.
- Request the minimum scopes per integration.
  Source: `catalogue/28_google_oauth.md`.

### YAML contract (documented and observed)

- Strict YAML 1.2 core: booleans are `true`/`false` only; `yes`, `no`, `on`,
  `off` are strings.
- Tags, directives, anchors, aliases, merge keys, multi-document streams, tabs
  used for indentation, and duplicate keys are rejected with a positioned error.
- Unknown constructs are faults, never silent guesses.

Source: `crates/dado/src/yaml.rs`,
`crates/dado/README.md`.

## Standard Best Practices (from MCP/skills)

- Rust API Guidelines: public types implement `Debug`; errors are meaningful and
  implement `std::error::Error`; crate-level docs are thorough; `Cargo.toml`
  includes description, licence, keywords, categories, and repository metadata.
  Source: <https://rust-lang.github.io/api-guidelines/checklist.html>.
- Cargo features are additive and documented; do not remove a feature in a minor
  release.
  Source: <https://doc.rust-lang.org/cargo/reference/features.html>.
- `cargo fmt` and `cargo clippy` are the standard Rust formatting and lint
  entry points (not currently configured in this repository).
  Source: Rust toolchain documentation; no Context7 or project skill files were
  available.

## Assumptions

- [to be confirmed] Whether `cargo clippy -- -D warnings` should be mandatory.
  It is not configured today.
- [to be confirmed] Whether `cargo fmt --check` should be mandatory in CI. No CI
  exists today.
- [to be confirmed] The exact semantics of the `assumption` tag: inferred rules
  are marked, but no project convention for the tag exists yet.
- [to be confirmed] Whether doc tests are required. `dado` has no examples yet.
