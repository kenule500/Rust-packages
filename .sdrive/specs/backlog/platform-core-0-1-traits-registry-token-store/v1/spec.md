---
title: platform-core 0.1 — traits, registry, token store
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# platform-core 0.1 — traits, registry, token store

## Overview

- **Problem Statement**: Every multi-provider product reinvents the same substrate: who the tenant is, which provider is being called, what that provider can do, and where its token lives. `wirekit` answers "how do I call Twilio?" but nothing answers "which providers does this tenant have, and where is the token?". Without a shared trait crate each consumer invents incompatible identities, so `quota-ledger` (BP-018) and `hookgate` (BP-020) cannot share a tenant model.
- **Proposed Solution**: Ship `platform-core` 0.1 as a standalone trait crate: `TenantId`, `ProviderId`, `ProvisioningMode`, `Capability`, the `Provider` trait, an explicit `Registry`, and an async `TokenStore` contract with `MemoryTokenStore` for tests. It ships no OAuth, no quota client, no webhook client, and no product vocabulary.
- **Target Audience**: Rust teams building a multi-tenant product on provider crates, and this project's own `quota-ledger` and `hookgate` features.
- **Assumptions**:
  - A working implementation exists on `feature/m3-substrate` (crate `platform-core`, 14 files, features `registry` and `memory`). This version **ports and adapts** it instead of redesigning it.
  - `main` has no `[workspace.dependencies]`, so the ported manifest declares `async-trait`, `secrecy`, `thiserror`, and `uuid` (plus `tokio` for dev) directly, matching the current workspace style.
  - The branch's `tests/m3-substrate/**` are **not** ported; tests are authored by `/sdrive:test` against this specification.
  - The unmerged `m3-substrate/v1` spec covers four crates and is not reused as governance here.

## User Stories

### US-001: Identify tenants and providers
As a service author, I want value types for tenant and provider identity, so that every crate in the stack speaks the same identity model.

**Acceptance Criteria:**
- AC-001: Given the crate built with `--no-default-features`, when it compiles, then the identity, error, and token modules are available without the registry.
- AC-002: Given `TenantId` and `ProviderId`, when they are used, then both are cheap value types: `TenantId` wraps a `Uuid`, `ProviderId` wraps a `&'static str`, and both implement `Debug`, `Clone`, `Copy`, `Eq`, and `Hash`.
- AC-003: Given a provider's provisioning path, when `ProvisioningMode` is inspected, then it distinguishes `Api`, `Partner`, `Manual`, and `Unsupported`.

### US-002: Describe capabilities
As a service author, I want providers to publish what they can do, so that a router can reason about capability without knowing the vendor.

**Acceptance Criteria:**
- AC-004: Given a `Provider` implementation, when `capabilities()` is called, then it returns a static slice of `Capability` values each pairing an opaque key with a `ProvisioningMode`.
- AC-005: Given a capability key, when it is compared, then the crate treats it as opaque and defines no product vocabulary for it (CON-106).

### US-003: Register providers explicitly
As a service author, I want an explicit registry, so that provider wiring is visible and testable instead of global.

**Acceptance Criteria:**
- AC-006: Given an empty `Registry`, when a provider is registered, then `get`, `iter`, and `capabilities` return it.
- AC-007: Given a registry that already holds a provider id, when the same id is registered again, then registration fails with a typed error naming the duplicate id.
- AC-008: Given the crate, when its public surface is inspected, then there is no global mutable singleton and no `static mut` (CON-109).

### US-004: Store tokens behind a contract
As a service author, I want a token-store trait, so that storage and key management stay in my adapter.

**Acceptance Criteria:**
- AC-009: Given a `TokenStore` implementation, when `get`, `put`, and `revoke` are called, then they are async and keyed by the `(TenantId, ProviderId)` pair.
- AC-010: Given the `memory` feature, when `MemoryTokenStore` is used, then it satisfies the trait and is absent when the feature is off.
- AC-011: Given a stored secret, when it is formatted with `Debug`, then the value is redacted and no plaintext token appears.
- AC-012: Given a token-store failure, when it is returned, then it is a `TokenError` with a stable message and no secret material.
- AC-013: Given the crate, when it is built on Rust 1.85 with all features, then it compiles with `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` and ships the workspace licence files.

## Requirements

### Functional Requirements
- FR-001: The crate MUST expose `TenantId`, `ProviderId`, `ProvisioningMode`, `Capability`, `Provider`, `Registry`, `TokenStore`, `TokenError`, and `Secret`, matching `catalogue/31_platform_core.md` (AC-001, AC-002, AC-004, AC-009).
- FR-002: Features MUST be additive and default to `registry` + `memory`; `--no-default-features` MUST compile (AC-001, AC-010).
- FR-003: `Registry` MUST reject a duplicate provider id with a typed error (AC-007).
- FR-004: Registration MUST be explicit; the crate MUST NOT expose a global mutable registry (AC-008).
- FR-005: `TokenStore` MUST be async, keyed by tenant and provider, and cover get/put/revoke (AC-009).
- FR-006: `MemoryTokenStore` MUST be gated behind the `memory` feature (AC-010).
- FR-007: Secrets MUST render redacted in `Debug` (AC-011).
- FR-008: Errors MUST carry stable messages and no secret material (AC-012).
- FR-009: The ported crate MUST declare `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` and include the licence files (AC-013).

### Non-Functional Requirements
- NFR-001: Tenant data MUST be isolated by the `(TenantId, ProviderId)` key; no API may return another tenant's token (CON-512, AC-009).
- NFR-002: Credentials MUST come from the caller's `TokenStore` implementation; the crate MUST NOT read the environment (CON-519, AC-012).
- NFR-003: The crate MUST compile on Rust 1.85 with all features (CON-102, AC-013).
- NFR-004: The public surface MUST contain no product vocabulary and no vendor workflow code (CON-106, AC-005).
- NFR-005: Every public item MUST carry rustdoc, and every module MUST state scope and non-goals (CON-202, CON-203, AC-013).

### Security Baseline
| Control | Status | Decision for this feature |
|---|---|---|
| CON-511 Authentication | REQUIRED | Tokens are opaque `Secret` values behind `TokenStore`; the crate never acquires or refreshes them. |
| CON-512 Authorization | REQUIRED | Every token operation is keyed by `(TenantId, ProviderId)`; a store cannot satisfy one tenant with another's token without breaking the contract. |
| CON-513 Security Headers | NOT APPLICABLE | No HTTP server and no responses. |
| CON-514 Rate Limiting | NOT APPLICABLE | No outbound calls; rate limiting lives in `wirekit` and `quota-ledger`. |
| CON-515 Input Validation | REQUIRED | `ProviderId` is `&'static str` and duplicate registration is a typed error. |
| CON-516 Injection Protection | NOT APPLICABLE | No database or command backend in 0.1. |
| CON-517 XSS / CON-518 CSRF | NOT APPLICABLE | No markup and no browser session. |
| CON-519 Environment Security | REQUIRED | No environment reads; `MemoryTokenStore` is explicitly not for production secrets. |
| CON-520 HTTPS / HSTS | NOT APPLICABLE | No HTTP client in this crate. |

## Edge Cases & Error States
- EC-001: Registering the same provider id twice -> typed error naming the id, registry unchanged.
- EC-002: `get` for an absent `(tenant, provider)` -> `Ok(None)`, never a fabricated token.
- EC-003: `--no-default-features` build -> identity, error, and token modules compile without the registry.
- EC-004: Secret formatted in logs -> redacted placeholder, never the value.

## Out of Scope
- OAuth flows, token refresh, and any provider client (those are `wirekit` or the adapter's job).
- Quota reservation (BP-018) and webhook delivery (BP-020).
- Database or HTTP framework features in 0.1.
- Product nouns and vendor workflow code (CON-106).

## Dependencies & Risks
- **Dependencies**: `async-trait`, `secrecy`, `thiserror`, `uuid` as normal dependencies and `tokio` for dev. All are declared directly in the crate manifest because `main` has no `[workspace.dependencies]`. No HTTP or database dependency.
- **RISK-001**: Porting stale code may drag in APIs that no longer match the workspace (the branch predates `wirekit`). Mitigation: port file by file, adapt the manifest, and let the MSRV and clippy jobs prove it.
- **RISK-002**: `secrecy`'s API differs across majors. Mitigation: pin one major and cover redaction with AC-011.
- **RISK-003**: A trait crate invites scope creep into implementations. Mitigation: NFR-004 and the out-of-scope list are enforced at review.


