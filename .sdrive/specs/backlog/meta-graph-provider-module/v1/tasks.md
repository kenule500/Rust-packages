---
title: Meta Graph provider module — execution tasks
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# Execution Tasks: Meta Graph provider module

> Test authoring and execution belong to `/sdrive:test`. No task below writes or runs tests.

## P1 — Foundation

- [ ] **T1.1 — Add the `meta` feature and module wiring**
  - Description: add `meta = []` to `crates/wirekit/Cargo.toml` (default-off, additive) and `pub mod meta` behind `#[cfg(feature = "meta")]` in `src/lib.rs`.
  - Requirements: FR-001, FR-003 · AC: AC-001

- [ ] **T1.2 — Define `MetaClient` and typed requests**
  - Description: `MetaClient` with `Secret<T>` credentials, base-URL validation, `SendMessageRequest`, `MessageAttachment`, `WhatsAppTemplate`.
  - Requirements: FR-002, FR-003, FR-004 · AC: AC-001, AC-002, AC-013

## P2 — Sends

- [ ] **T2.1 — Messenger text and media send**
  - Description: `FacebookMessenger::send_text` / `send_media` posting to `/{page-id}/messages` with `messaging_type` and `recipient.id`.
  - Requirements: FR-004 · AC: AC-001, AC-002

- [ ] **T2.2 — Instagram DM send**
  - Description: `InstagramDm` reusing `SendMessageRequest` against the Instagram Graph host.
  - Requirements: FR-003, FR-004 · AC: AC-003

- [ ] **T2.3 — WhatsApp text and template send**
  - Description: `WhatsAppBusiness::send_text` and `send_template`, with ordered parameters and a local parameter-count check.
  - Requirements: FR-005 · AC: AC-004, AC-005

- [ ] **T2.4 — WhatsApp media upload**
  - Description: `upload_media` returning an attachment id usable by a subsequent send.
  - Requirements: FR-006 · AC: AC-002

## P3 — Webhooks

- [ ] **T3.1 — Signature verification and parsing**
  - Description: HMAC-SHA256 over raw bytes, constant-time comparison, fail closed, size-bounded body.
  - Requirements: FR-009, NFR-003 · AC: AC-006, AC-007

- [ ] **T3.2 — Subscription handshake**
  - Description: echo `hub.challenge` when `hub.mode=subscribe` and `hub.verify_token` matches; reject otherwise; subscribe/unsubscribe calls.
  - Requirements: FR-010 · AC: AC-008

- [ ] **T3.3 — Normalise inbound events**
  - Description: map Messenger, Instagram, and WhatsApp payloads into `MetaEvent` with surface, dedupe key, and an unsupported variant.
  - Requirements: FR-011 · AC: AC-009

## P4 — Errors, limits, secrets

- [ ] **T4.1 — Map nested Graph errors**
  - Description: flatten `error.message`, `error.code`, `error.error_subcode`, `error.fbtrace_id` into `WirekitError::Api`.
  - Requirements: FR-007 · AC: AC-010

- [ ] **T4.2 — Surface usage and integrate backoff**
  - Description: parse `X-Business-Use-Case-Usage`, expose it on the response type, and let the existing retry policy back off on 429/5xx.
  - Requirements: FR-008 · AC: AC-011

- [ ] **T4.3 — Redact credentials**
  - Description: hand-written `Debug` for `MetaClient` and config so neither token nor app secret renders.
  - Requirements: NFR-001 · AC: AC-012

- [ ] **T4.4 — Enforce HTTPS and token scope**
  - Description: reject cleartext base URLs at construction and reject a caller identity that contradicts the configured page/phone number.
  - Requirements: NFR-002, NFR-004 · AC: AC-013, AC-001

## P5 — Documentation and verification

- [ ] **T5.1 — Document the module**
  - Description: `//!` module docs with scope and non-goals, rustdoc on every public item, README provider row and example, CHANGELOG entry.
  - Requirements: NFR-005 · AC: —

- [ ] **T5.2 — Run formatter, linter, docs, and governance checks**
  - Description: `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features --no-deps`, `node .sdrive/scripts/validate-governance.js`.
  - Requirements: NFR-005 · AC: —
