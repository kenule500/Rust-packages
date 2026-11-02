# Project Overview

> Context file for `.sdrive/constitution.md`.
> Created: 2026-10-20. Every fact below was read from the repository; nothing is inferred except the lines under **Assumptions**.

## Observed Project Facts

### Identity and location

| Fact | Value | Source |
|---|---|---|
| Repository | `Rust-packages` | `git remote -v` |
| Remote | `https://github.com/kenule500/Rust-packages.git` | `git remote -v` |
| Default branch | `main` | `git branch -a` |
| Commit history | 15 commits on `feature/meta-stripe-platform-core`, 14 ahead of `origin/main` | `git log --oneline` |
| SpecDrive root | `C:\Users\KENULE-NWINEE\Documents\vscode-projs\Projects\Packages` | `.sdrive/` location |
| Code root | `Packages/` | `Cargo.toml` |
| Product name in docs | "GIA Packages" | `README.md` |

The repository is the standalone home of public Rust packages consumed by GIA and
other projects. GIA is described as one consumer, not the owner of the ecosystem.

### Purpose

- Publish generic, documented Rust crates to crates.io under `MIT OR Apache-2.0`.
- Keep one job per package instead of one framework.
- Let the consuming product keep its own domain types, billing rules, and service
  composition.

Source: `README.md` sections "GIA Packages", "Why these are separate
from the app", and "Public crates.io, not app-only".

### Technology stack

| Fact | Value | Source |
|---|---|---|
| Language | Rust | `Cargo.toml` |
| Edition | 2021 | `Cargo.toml` `[workspace.package]` |
| Resolver | 3 (MSRV-aware) | `Cargo.toml` |
| MSRV | 1.87 | `Cargo.toml` `rust-version` |
| Licence | `MIT OR Apache-2.0` | `Cargo.toml` `license` |
| Workspace members | `crates/*` | `Cargo.toml` |
| Installed toolchain | rustc 1.95.0, cargo 1.95.0 | `rustc --version`, `cargo --version` |
| Node tooling | Node 24.20.0; SpecDrive CLI 0.1.44 | `node --version`, `sdrive --version` |

### Current crates

Three crates exist today:

| Crate | Version | State | Source |
|---|---|---|---|
| `dado-yaml` | 0.1.0 | Implemented: strict YAML subset, diagnostics, graph, emit, and glob; 38 tests (36 pass, 2 ignored pending publish and tag checks). | `crates/dado-yaml/README.md`, `src/yaml.rs` |
| `platform-core` | 0.1.0 | Implemented: tenant and provider identity, capabilities, registry, and token-store contract. `registry` and `memory` features, both on by default. | `crates/platform-core/Cargo.toml`, `src/lib.rs` |
| `wirekit` | 0.1.0 | Implemented: shared HTTP core plus nine provider modules behind additive features (`noaa`, `twilio`, `sendgrid`, `slack`, `google-oauth`, `meta`, `angi`, `google-lsa`, `sage-intacct`). The `adp` feature is declared but has no module. Stripe is documented as the community crate `async-stripe` rather than wrapped. | `crates/wirekit/Cargo.toml`, `src/lib.rs` |

`cargo test --workspace --all-features` passes 175 tests with 0 failures.

`dado-yaml` is described as "the base layer for config-driven tools: strict YAML,
diagnostics, dependency graphs, code emission, and glob matching", with five
feature flags and zero dependencies by default.

### Planned crates (not yet implemented)

| Crate | Job | Source |
|---|---|---|
| `syncforge` | Offline-first sync engine | `catalogue/02_syncforge.md` |
| `quota-ledger` | Per-tenant, per-provider quota reservation and ledger | `catalogue/29_quota.md` |
| `hookgate` | Inbound webhook edge: verify, dedupe, replay-protect, forward | `catalogue/30_webhook_gateway.md` |
| `ffscene` | Scene graph to FFmpeg filter graph | `catalogue/32_ffscene.md` |
| `mixbus` | Real-time audio mixing graph | `catalogue/33_mixbus.md` |
| `replay-recorder` | Multi-track capture and replay ring | `catalogue/34_replay_recorder.md` |
| `studio-protocol` | Preview/program control protocol | `catalogue/35_studio_protocol.md` |
| `rtmp-publish` | Egress destination model, key redaction, health | `catalogue/36_rtmp_publish.md` |
| `signedlink` | Signed short links, UTM, expiry, attribution | `catalogue/37_signedlink.md` |

`wirekit` and `platform-core` were on this list and are now implemented; see
"Current crates" above. The 26 provider modules are `wirekit` feature flags, not
separate crates. Nine are built; `adp` is declared but has no module.

The published names differ from the working names where crates.io already owns
them: `quota` -> `quota-ledger`, `webhook-gateway` -> `hookgate`,
`media-engine` -> `ffscene`, `audio-engine` -> `mixbus`, `recorder` ->
`replay-recorder`.

### Publishing status

- Three crates are implemented and tested; none is published to crates.io.
- `dado-yaml` 0.1.0, `platform-core` 0.1.0, and `wirekit` 0.1.0 all pass
  `cargo test --workspace --all-features` (175 passed, 0 failed).
- `syncforge` is specified but not implemented.
- The media and link packages are public candidates whose APIs must freeze before
  publish.
- crates.io names are permanent; availability was checked October 2026.

Source: `README.md` "Status"; `catalogue/00_provider_index.md` "Status".

### Repository hygiene observed

| Fact | Value | Source |
|---|---|---|
| Ignored paths | `target/`, `**/*.rs.bk`, `Cargo.lock`, `.DS_Store` | `.gitignore` |
| `Cargo.lock` on disk | Present, untracked | `git ls-files Cargo.lock` (empty) |
| CI configuration | `.github/workflows/ci.yml` | `.github/workflows/ci.yml` |
| Test files | `crates/dado-yaml/tests/m0-foundation/` | `crates/**` |
| `rustfmt` / `clippy` config | None found | repository tree |
| `CONTRIBUTING.md` | Present | `CONTRIBUTING.md` |
| Tracked files | Cargo workspace at the repository root; provider docs under `catalogue/` | `git ls-files` |

### Governance state at discovery time

- `.sdrive/constitution.md` did not exist.
- `.sdrive/context/` was empty.
- `.sdrive/skills/` was empty.
- `.sdrive/workflow-state.json` contains `{ "features": [] }`.
- `.sdrive/config.json` sets `requireTestEvidence`, disallows stale evidence, and
  requires structured review.
- `.sdrive/team.json` lists `kenule500` as `admin`; approval roles are `admin` and
  `reviewer`; implementation roles are `developer` and `admin`.

Source: `.sdrive/config.json`, `.sdrive/team.json`, `.sdrive/workflow-state.json`.

## Standard Best Practices (from MCP/skills)

- The Cargo Book requires feature sets to be additive and documented; removing a
  feature or moving public code behind one is normally a breaking change.
  Source: <https://doc.rust-lang.org/cargo/reference/features.html>.
- The Rust API Guidelines require common Cargo metadata, thorough crate-level
  documentation, meaningful error types implementing `std::error::Error`, a
  permissive licence, and `Debug` on public types.
  Source: <https://rust-lang.github.io/api-guidelines/checklist.html>.
- crates.io names are permanent and cannot be reused after publication.
  Source: `README.md` "Crate names that are already taken".
- No Context7 or project skill files were available in this environment.

## Assumptions

- [to be confirmed] The release cadence and versioning policy per crate.
- [to be confirmed] Whether CI (GitHub Actions) will be added; none exists today.
- [to be confirmed] Whether `Cargo.lock` should stay ignored for every crate,
  including any future binary crate in this workspace.
- [to be confirmed] Whether crates will be published from this repository or from
  a mirror.
