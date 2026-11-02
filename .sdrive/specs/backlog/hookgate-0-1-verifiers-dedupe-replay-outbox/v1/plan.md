---
title: hookgate 0.1 - technical plan
status: backlog
version: v1
created: 2026-11-02
updated: 2026-11-02
---

# hookgate 0.1 - technical plan

## Approach

Port the `hookgate` crate from `feature/m3-substrate` as prior art and adapt it to the current workspace: one library crate at `crates/hookgate`, feature-gated with `default = [presets]` and an additive `presets` feature that ships the provider verifiers. The core is four value types (`DeliveryId`, `IdempotencyKey`, `InboundRequest`, `VerifiedEvent`) plus `DeliveryRecord`, `InsertOutcome`, and `DeadLetterRecord`, a redacting error type, and three traits: `Verifier` (synchronous, because signature checking does no I/O), `DeliveryStore`, and `Outbox` (both async). The load-bearing invariant is that verification runs over the exact received bytes; `InboundRequest` therefore holds `Bytes` and never a parsed document, and the module docs state that re-serialization is a security bug. `MemoryDeliveryStore` is the reference implementation. The crate has no provider client, no outbound HTTP, no HTTP server, and no axum integration, and it depends on `platform-core` only for tenant identity.

## Components

| Component | Action | Notes |
|---|---|---|
| `crates/hookgate/src/types.rs` | create | `DeliveryId`, `IdempotencyKey`, `InboundRequest` (raw `Bytes`), `VerifiedEvent`, `DeliveryRecord`, `InsertOutcome`, `DeadLetterRecord`. |
| `crates/hookgate/src/error.rs` | create | `HookgateError` plus `redact_error`, so a failure never renders a signature or a shared secret. |
| `crates/hookgate/src/verify/mod.rs` | create | The `Verifier` trait, constant-time comparison, and the timestamp tolerance, operating on raw bytes only. |
| `crates/hookgate/src/verify/presets.rs` | create | The `presets` feature: Meta, Stripe, Shopify, Twilio, Slack, GitHub, and a configurable generic HMAC verifier. |
| `crates/hookgate/src/store.rs` | create | The `DeliveryStore` trait (`insert_if_new`, `mark_processed`, `mark_failed`) and the `Outbox` trait. |
| `crates/hookgate/src/memory.rs` | create | `MemoryDeliveryStore`: non-durable reference implementation and deterministic test seam. |
| `crates/hookgate/src/lib.rs` | create | Module declarations, re-exports, crate docs with the raw-body rule and non-goals, lint attributes. |
| `crates/hookgate/Cargo.toml` | create | Dependencies declared directly. Features `default = [presets]`, `presets = []`. |
| `crates/hookgate/README.md` | create | Ingress flow, the raw-body rule, the presets, and the non-goals. |
| `LICENSE-MIT`, `LICENSE-APACHE` | reuse | Copied from the workspace root. |
| `crates/platform-core` | reuse | Tenant identity is consumed, not redefined. |
| `Cargo.toml` | modify | Verify the workspace resolves the new member. |

## Data model

- **`InboundRequest`** — provider, headers (`HeaderMap`), body (`Bytes`, verbatim), received_at.
- **`VerifiedEvent`** — delivery, provider, optional event id, event type, parsed payload, raw bytes.
- **`DeliveryRecord`** — id, tenant, provider, optional event id, event type, raw bytes, received_at, state.
- **`DeadLetterRecord`** — delivery, raw bytes, reason, attempts.

**Flow.** Receive the raw bytes → resolve the tenant and provider → verify the signature against those exact bytes with a constant-time comparison → reject a timestamp outside the tolerance as stale → insert the delivery if it is new → enqueue the verified event → only then report success. A duplicate insert returns `InsertOutcome::Duplicate` rather than a second row, and uniqueness includes the tenant so the same event id from two tenants is not a duplicate. The processor consumes the outbox, marks the delivery processed, or dead-letters it with the raw body and reason. Replay is idempotent against the delivery store.

## Security considerations

- **NFR-001** — every delivery read and write is scoped by tenant; another tenant cannot observe or replay a delivery that is not theirs.
- **NFR-002** — signature material and shared secrets never render; the error type is redacted and `InboundRequest` holds raw bytes without a secret field.
- **NFR-003** — headers, raw bodies, and event ids are validated and bounded before use; an oversized or malformed request is a typed error rather than a panic.
- **NFR-004** — the crate reads no secret from the environment; the caller constructs each verifier with its own secret.

## Performance considerations

- **NFR-003** — signature comparison is constant-time (`subtle`) so verification cost does not leak the expected value through timing.
- **NFR-005** — `InboundRequest` keeps the body as `Bytes` and clones by reference counting, so the raw bytes are not copied per verification stage.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `platform-core` | internal | no_change | Path dependency already on `main`; supplies `TenantId`. |
| `async-trait` | library | add | Declared directly. |
| `bytes` | library | add | Raw body bytes preserved verbatim. |
| `http` | library | add | `HeaderMap` for `InboundRequest`. |
| `hmac` | library | add | Provider signature presets. |
| `sha1` | library | add | Twilio's signature uses HMAC-SHA1. |
| `sha2` | library | add | HMAC-SHA256 presets. |
| `subtle` | library | add | Constant-time comparison. |
| `base64` | library | add | Shopify and generic base64 signatures. |
| `hex` | library | add | Hex-encoded signatures. |
| `chrono` | library | add | Received-at and tolerance. |
| `serde_json` | library | add | Parsed payload after verification. |
| `uuid` | library | add | `DeliveryId`. |
| `thiserror` | library | add | The redacting error type. |
| `tracing` | library | add | Lifecycle events. |
| `tokio` | library | add | Dev-dependency only. |

No dependency skill is available for Rust crate dependencies; docs-only, per the workspace precedent.

## Risks

- **The raw-body rule is easy to violate by a well-meaning refactor that parses first**, which silently breaks every signature. AC-001 and AC-003 assert it directly and EC-001 names the failure so a test can pin it.
- **The delivery-and-outbox-before-acknowledge guarantee is weak when the two writes are not transactional in the memory store.** State the guarantee explicitly on the store contract and pin it with AC-007 and EC-003.
- **Porting stale code may drag in a cross-crate dev-dependency on `quota-ledger`** from the prior art. Declare dependencies directly and drop the cross-crate dev-dependency unless the test actually needs it.
