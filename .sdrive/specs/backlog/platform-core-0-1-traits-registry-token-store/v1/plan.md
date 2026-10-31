---
title: platform-core 0.1 — technical plan
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Technical Plan: platform-core 0.1

## 1. Approach

Port the existing crate from `feature/m3-substrate` file by file into `crates/platform-core/`, adapting the manifest and any API that drifted since that branch was cut. The crate is standalone: it depends on no workspace crate, and `members = ["crates/*"]` picks it up without touching the root manifest.

The port is a lift with deliberate edits:
- dependencies declared directly (the workspace has no `[workspace.dependencies]`),
- tests excluded (the test agent owns them),
- module docs restated against this specification's non-goals.

## 2. Components

| Component | Action | Notes |
|---|---|---|
| `crates/platform-core/Cargo.toml` | create | Package metadata, `registry`/`memory` features, direct dependencies |
| `crates/platform-core/src/lib.rs` | create | Lints, feature-gated module declarations, re-exports, `VERSION` |
| `crates/platform-core/src/id.rs` | create | `TenantId(Uuid)`, `ProviderId(&'static str)` |
| `crates/platform-core/src/error.rs` | create | `ErrorCode`, `IntegrationError` |
| `crates/platform-core/src/token.rs` | create | `Secret`, `TokenStore`, `TokenError`, `MemoryTokenStore` behind `memory` |
| `crates/platform-core/src/capability.rs` | create | `ProvisioningMode`, `Capability` (behind `registry`) |
| `crates/platform-core/src/provider.rs` | create | `Provider`, `ProviderRef` (behind `registry`) |
| `crates/platform-core/src/registry.rs` | create | `Registry` with duplicate detection (behind `registry`) |
| `crates/platform-core/README.md` | create | Scope, non-goals, example |
| `crates/platform-core/LICENSE-MIT`, `LICENSE-APACHE` | create | Copies of the workspace licences |
| `Cargo.toml` (workspace) | reuse | `members = ["crates/*"]` already includes the new crate |

Referenced requirements: FR-001 … FR-009, NFR-001 … NFR-005.

## 3. Data model

| Entity | Action | Fields |
|---|---|---|
| `TenantId` | create | `0: Uuid` |
| `ProviderId` | create | `0: &'static str` |
| `ProvisioningMode` | create | `Api`, `Partner`, `Manual`, `Unsupported` |
| `Capability` | create | `key: &'static str`, `mode: ProvisioningMode` |
| `Provider` (trait) | create | `id()`, `capabilities()` |
| `Registry` | create | providers map, `register`, `get`, `iter`, `capabilities` |
| `TokenStore` (trait) | create | `get`, `put`, `revoke` (async, tenant+provider key) |
| `Secret` | create | opaque wrapper with redacted `Debug` |
| `TokenError`, `IntegrationError` | create | stable codes and messages |

Flow: the application builds a `Registry`, registers its `Provider` values, and obtains a `TokenStore` implementation. Reads and writes are always keyed by `(TenantId, ProviderId)`; the crate stores nothing itself.

## 4. API contracts

None. This crate performs no HTTP calls and exposes no endpoints.

## 5. Security considerations

- **NFR-001 / CON-512:** every token operation is keyed by tenant and provider, so one tenant cannot be served another's token through a well-behaved implementation.
- **NFR-002 / CON-519:** the crate never reads the environment; `MemoryTokenStore` is documented as unsuitable for production secrets.
- **FR-007:** `Secret` renders redacted; `TokenError` carries no token material.
- **FR-003:** duplicate provider registration is a typed error rather than a silent overwrite.

## 6. Performance considerations

- Identity types are `Copy` and allocation-free.
- `Registry` lookups are keyed by `ProviderId`; no global lock and no singleton.
- `TokenStore` is a trait, so storage latency stays the adapter's concern.

## 7. Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `async-trait` | library | add | Async trait methods; pinned at a 1.85-compatible version |
| `secrecy` | library | add | Redacted secret wrapper |
| `thiserror` | library | add | Error derive |
| `uuid` | library | add | `TenantId` payload |
| `tokio` | library | add | Dev only (test agent's async tests) |

## 8. Risks

- **RISK-001** stale port → port file by file; MSRV and clippy jobs prove compatibility.
- **RISK-002** `secrecy` API drift → pin one major; AC‑011 covers redaction.
- **RISK-003** scope creep into implementations → out-of-scope list enforced at review.
