---
title: platform-core 0.1 — execution tasks
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Execution Tasks: platform-core 0.1

> Porting tasks only. Tests are authored and executed by `/sdrive:test`.

## P1 — Crate scaffold and identity

- [ ] **T1.1 — Create the crate manifest and licence files**
  - Description: `crates/platform-core/Cargo.toml` with package metadata, `default = ["registry", "memory"]`, and direct dependencies (`async-trait`, `secrecy`, `thiserror`, `uuid`; `tokio` as dev). Copy `LICENSE-MIT` and `LICENSE-APACHE`.
  - Requirements: FR-002, FR-009, NFR-003 · AC: AC-001, AC-013

- [ ] **T1.2 — Port `id.rs` and `error.rs`**
  - Description: `TenantId`, `ProviderId`, `ErrorCode`, `IntegrationError`, adapted to the current workspace lints.
  - Requirements: FR-001, FR-008 · AC: AC-002, AC-012

- [ ] **T1.3 — Port the token module**
  - Description: `Secret`, async `TokenStore` via `async-trait`, `TokenError`, and `MemoryTokenStore` behind the `memory` feature.
  - Requirements: FR-005, FR-006, FR-007, NFR-001, NFR-002 · AC: AC-009, AC-010, AC-011, AC-012

## P2 — Capabilities and registry

- [ ] **T2.1 — Port `capability.rs` and `provider.rs`**
  - Description: `ProvisioningMode`, `Capability`, `Provider`, `ProviderRef`, all behind `registry`.
  - Requirements: FR-001, NFR-004 · AC: AC-003, AC-004, AC-005

- [ ] **T2.2 — Port `registry.rs`**
  - Description: `Registry` with explicit `register`, `get`, `iter`, `capabilities`, and a typed duplicate-id error; no global state.
  - Requirements: FR-003, FR-004 · AC: AC-006, AC-007, AC-008

## P3 — Wiring, docs, verification

- [ ] **T3.1 — Port `lib.rs` and the README**
  - Description: `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, feature-gated modules and re-exports, `VERSION`; README with scope, non-goals, and an example.
  - Requirements: FR-001, FR-009, NFR-005 · AC: AC-001, AC-013

- [ ] **T3.2 — Verify the workspace**
  - Description: `cargo check -p platform-core --no-default-features`, `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`, `cargo +1.85.0 check --workspace --all-features`, and `node .sdrive/scripts/validate-governance.js`.
  - Requirements: FR-002, NFR-003 · AC: AC-001, AC-013
