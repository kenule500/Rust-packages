---
title: QuickBooks provider module — execution tasks
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Execution Tasks: QuickBooks provider module

> Production code only. Tests are authored and executed by `/sdrive:test`.

## P1 — Foundation

- [ ] **T1.1 — Add the `quickbooks` feature and wiring**
  - Description: `quickbooks = []` in `Cargo.toml` (additive, default-off) and `pub mod quickbooks` behind `#[cfg(feature = "quickbooks")]`.
  - Requirements: FR-001 · AC: AC-001

- [ ] **T1.2 — Define `QuickBooksConfig` and the client**
  - Description: realm id, minor version, `Secret` credentials, HTTPS-only base URL, `Debug` redaction.
  - Requirements: FR-002, NFR-001, NFR-002, NFR-004 · AC: AC-001

## P2 — OAuth2

- [ ] **T2.1 — Token manager**
  - Description: refresh before expiry, at most once per request, retry the request once, surface the rotated refresh token; a persistent 401 fails.
  - Requirements: FR-004, NFR-005 · AC: AC-002, AC-003

## P3 — Entities

- [ ] **T3.1 — Customer create and update**
  - Description: POST to `customer` with display name and contact fields; update carries `Id` and `SyncToken`.
  - Requirements: FR-003, FR-005 · AC: AC-004, AC-005

- [ ] **T3.2 — Invoice create, void, send**
  - Description: ordered lines with amount, description, item reference; void operation; send operation.
  - Requirements: FR-006 · AC: AC-006, AC-007, AC-008

- [ ] **T3.3 — Payments**
  - Description: record a payment against an invoice reference; surface provider validation faults for over-balance amounts.
  - Requirements: FR-007 · AC: AC-009, AC-010

- [ ] **T3.4 — Accounts and items**
  - Description: query endpoint with a bounded result size returning typed `Account` and `Item` entries.
  - Requirements: FR-008 · AC: AC-011

## P4 — Webhooks, faults, retries

- [ ] **T4.1 — `intuit-signature` verification and dedupe**
  - Description: HMAC-SHA256 over raw bytes in constant time, fail closed, duplicate detection by event id.
  - Requirements: FR-009, FR-010, NFR-003 · AC: AC-012, AC-013

- [ ] **T4.2 — Fault mapping, retry policy, idempotency**
  - Description: `Fault.Error[]` → `WirekitError::Api` preserving codes; honour `Retry-After`; never retry validation faults; pre-create idempotency check.
  - Requirements: FR-011, FR-012 · AC: AC-014, AC-015, AC-016

## P5 — Docs and verification

- [ ] **T5.1 — Document the module**
  - Description: module docs with scope and non-goals, rustdoc on public items, README provider row and example, CHANGELOG entry.
  - Requirements: NFR-006 · AC: —

- [ ] **T5.2 — Run formatter, linter, docs, governance**
  - Description: `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`, `node .sdrive/scripts/validate-governance.js`.
  - Requirements: NFR-006 · AC: —
