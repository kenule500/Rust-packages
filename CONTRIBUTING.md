# Contributing to dado-yaml

`dado-yaml` is the base layer for config-driven Rust tools: a strict YAML parser,
diagnostics, dependency graphs, code emission, and glob matching. The Cargo
workspace lives at the repository root, with the published crate at
[`crates/dado-yaml/`](crates/dado-yaml/).

## Prerequisites

- Rust 1.85 or newer. The workspace's MSRV is declared in
  [`Cargo.toml`](Cargo.toml).
- `rustfmt` and `clippy` components:
  `rustup component add rustfmt clippy`
- Git and a GitHub account.
- No other system toolchain is required. All commands below are `cargo`.

## Build and test

Run commands from the repository root:

```sh
# Build every feature
cargo build --workspace --all-features

# Run the test suite
cargo test --workspace --all-features

# Formatting
cargo fmt --all --check

# Lints
cargo clippy --workspace --all-features -- -D warnings

# Documentation
RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps

# Packaging
cargo package --workspace

# Dependency advisories; install once with
# `cargo install cargo-audit --locked`
cargo audit
```

`cargo audit` reads [`.cargo/audit.toml`](.cargo/audit.toml). Every ignored
advisory must name its RUSTSEC ID and the reason, and the entry must be removed
once the blocker clears.

On PowerShell, set the documentation flag separately:

```powershell
$env:RUSTDOCFLAGS = "-D warnings"
cargo doc --workspace --all-features --no-deps
```

If you touch the parser, also verify each feature builds independently:

```sh
cargo check -p dado-yaml --no-default-features
cargo check -p dado-yaml --no-default-features --features yaml
cargo check -p dado-yaml --no-default-features --features diag
cargo check -p dado-yaml --no-default-features --features graph
cargo check -p dado-yaml --no-default-features --features emit
cargo check -p dado-yaml --no-default-features --features glob
```

## Branches and commits

- Create one branch per feature, named `feature/<name>`.
- Never implement directly on `main`.
- Use conventional commit prefixes. SpecDrive commands use:

  ```text
  specdrive(<command>): <summary>
  ```

  Examples: `specdrive(propose): add m0-foundation specification`,
  `specdrive(apply): implement CI and release readiness`.

- Code changes use the task ID when one exists:
  `feat(T3.2): add CI quality jobs`, `fix(T1.5): reject flow anchors`.

## SpecDrive workflow

This repository uses [SpecDrive](.sdrive/) for spec-driven development.
Changes follow the same path as production code:

1. **Propose.** Run `/sdrive:propose <feature>`. The agent writes the
   specification, plan, and task list on a dedicated `feature/<name>` branch.
   Specification and planning gates require human approval.
2. **Apply.** Run `/sdrive:apply <feature>`. The agent implements the approved
   tasks only, runs formatter, linter, docs, and package checks, and records
   completion in `tasks.json` and `traceability.json`.
3. **Test.** Run `/sdrive:test <feature>`. Tests are authored and executed only
   by the Test Agent, never by the implementation phase.
4. **Review and archive.** Run `/sdrive:review` and then `/sdrive:archive` to
   merge through the governed process.

The governing rules are in [`.sdrive/constitution.md`](.sdrive/constitution.md).
Product work is tracked in [`.sdrive/product/backlog.md`](.sdrive/product/backlog.md).

## Pull request checklist

- [ ] The change is linked to a backlog item or SpecDrive feature.
- [ ] `cargo fmt --all --check` passes.
- [ ] `cargo clippy --workspace --all-features -- -D warnings` passes.
- [ ] `cargo test --workspace --all-features` passes.
- [ ] `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`
      passes.
- [ ] `cargo package --workspace` passes when packaging or metadata changes.
- [ ] `cargo audit` reports no vulnerabilities.
- [ ] New public items have rustdoc and the crate still compiles with
      `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`.
- [ ] No secrets, tokens, credentials, or generated build output are committed.
- [ ] The change stays inside the approved `tasks.md` scope.
- [ ] Security-sensitive changes follow [`SECURITY.md`](SECURITY.md).

## Licensing

No Contributor License Agreement (CLA) and no Developer Certificate of Origin
(DCO) sign-off are required. By contributing, you agree that your contribution
is licensed under the same terms as the project: **MIT OR Apache-2.0**.

See [`LICENSE-MIT`](LICENSE-MIT) and [`LICENSE-APACHE`](LICENSE-APACHE).

## Reporting security issues

Do not open a public issue for a vulnerability. Follow
[`SECURITY.md`](SECURITY.md) and use GitHub private vulnerability reporting.
