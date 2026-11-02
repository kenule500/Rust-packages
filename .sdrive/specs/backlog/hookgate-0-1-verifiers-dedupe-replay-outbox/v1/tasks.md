---
title: hookgate 0.1 - execution tasks
status: backlog
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# hookgate 0.1 - execution tasks

> Production code only. Test authoring and test execution belong to the test agent (`/sdrive:test`) and are not tasks here.

## P1 - Crate scaffold and core types

- [ ] **T1.1 - Scaffold the hookgate crate** (`FR-001`, `FR-011` / `AC-002`, `AC-014`)
  Create `crates/hookgate` with a manifest that declares dependencies directly (`main` has no `[workspace.dependencies]`), features `default = [presets]` and `presets = []`, the workspace-inherited `edition`, `rust-version`, `license`, and `repository` fields, the licence files, a README, and a `lib.rs` carrying `forbid(unsafe_code)`, `deny(missing_docs)`, and crate docs that state the raw-body rule and the non-goals.
- [ ] **T1.2 - Implement core types and the redacting error type** (`FR-001`, `FR-010`, `NFR-002` / `AC-002`, `AC-008`, `AC-013`)
  Implement `types.rs` with `DeliveryId`, `IdempotencyKey`, `InboundRequest` holding the raw body as `Bytes`, `VerifiedEvent`, `DeliveryRecord`, `InsertOutcome`, and `DeadLetterRecord`. Implement `error.rs` with `HookgateError` and a `redact_error` helper so no signature, shared secret, or credential material can render in `Debug`, `Display`, a log, or an error payload.

## P2 - Verification core and presets

- [ ] **T2.1 - Implement the Verifier trait over raw bytes** (`FR-001`, `FR-002`, `FR-003`, `NFR-003` / `AC-001`, `AC-003`)
  Implement `verify/mod.rs`: the `Verifier` trait, constant-time comparison via `subtle`, and failure that closes on a mismatch. Verification must compute the signature over the exact received bytes and must never parse and re-serialize the body first. Bound and validate the input so an oversized or malformed request is a typed error rather than a panic.
- [ ] **T2.2 - Enforce the timestamp tolerance** (`FR-004`, `NFR-004` / `AC-004`, `AC-013`)
  Add the received-at and signature-timestamp comparison so a request whose timestamp is outside the configurable skew is rejected as stale rather than accepted. The verifier takes its secret from the caller and reads nothing from the environment.
- [ ] **T2.3 - Implement the provider verifier presets** (`FR-008` / `AC-010`)
  Behind the `presets` feature, implement verifiers for Meta (`X-Hub-Signature-256` HMAC-SHA256), Stripe (`Stripe-Signature` with timestamp tolerance), Shopify (`X-Shopify-Hmac-Sha256` base64), Twilio (`X-Twilio-Signature` HMAC-SHA1 over URL plus sorted params), Slack (v0 versioned HMAC-SHA256), and GitHub (`X-Hub-Signature-256`), each over the raw received bytes.
- [ ] **T2.4 - Add the generic HMAC verifier and gate the presets** (`FR-008` / `AC-010`, `AC-011`)
  Add a configurable generic HMAC verifier accepting the caller's header name, encoding, and prefix so an unlisted provider is covered. Ensure the `presets` feature is additive and that a build without it still exposes the core types, traits, and memory store while omitting every preset.

## P3 - Delivery store and outbox

- [ ] **T3.1 - Define DeliveryStore and Outbox** (`FR-001`, `FR-006` / `AC-007`, `AC-008`)
  Define the `DeliveryStore` trait with `insert_if_new` returning an `InsertOutcome`, `mark_processed`, and `mark_failed`, and the `Outbox` trait with `enqueue`. State on the contract that the delivery record and the outbox entry are both written before success is reported, so a crash after acknowledgement leaves the event recoverable.
- [ ] **T3.2 - Implement MemoryDeliveryStore with tenant-scoped dedupe** (`FR-005`, `NFR-001` / `AC-005`, `AC-006`, `AC-012`)
  Implement `memory.rs`. `insert_if_new` is idempotent per tenant and event id and returns `Duplicate` for a repeat, and the uniqueness key includes the tenant so the same event id from two tenants is not a duplicate. Every read and write is scoped by the tenant.
- [ ] **T3.3 - Add dead-letter capture and idempotent replay** (`FR-007` / `AC-008`, `AC-009`)
  Record a failed delivery as a `DeadLetterRecord` carrying its raw body, reason, and attempt count, and make replaying it idempotent against the delivery store so a poison delivery is recoverable rather than blocking the outbox.

## P4 - Wiring, docs, verification

- [ ] **T4.1 - Wire the crate, document it, and run the verification checks** (`FR-009`, `FR-011`, `NFR-005`, `NFR-006` / `AC-011`, `AC-014`)
  Confirm the workspace resolves the new member; write the README covering the ingress flow, the raw-body rule, and the presets; ensure every public item carries rustdoc and every module states scope and non-goals; confirm the crate contains no provider client, outbound HTTP call, HTTP server, or axum integration, and no product vocabulary. Then run `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo doc --workspace --all-features --no-deps` with `RUSTDOCFLAGS=-D warnings`, `cargo check -p hookgate --no-default-features`, and `node .sdrive/scripts/validate-governance.js`.
