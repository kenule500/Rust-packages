# 30 — hookgate

**Package:** `hookgate` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library with an optional `axum` integration
**Licence:** MIT OR Apache-2.0

> **Name note.** `webhook-gateway` is already taken on crates.io (a dormant
> GitHub-event ingress service, MIT OR Apache-2.0, v2.17.2). The published crate
> name is therefore `hookgate`. This document still describes the
> “webhook gateway” concept; the file name is the catalogue number.

`webhook-gateway` is the inbound edge for provider webhooks. It verifies the
signature, rejects replays, deduplicates deliveries, persists the raw event,
and hands it to an outbox for reliable processing.

It is not a provider client. `wirekit` verifies signatures for the providers it
wraps. `webhook-gateway` owns the part that must survive a restart: the
delivery record, the idempotency key, the replay window, and the handoff to a
queue.

---

## Why this is not a wirekit module

| Concern | `wirekit` | `webhook-gateway` |
|---|---|---|
| Scope | Per-provider signature helper | Provider-agnostic ingress |
| State | None | Delivery store, outbox, dead-letter |
| Ordering | Not its job | Per-tenant ordering guarantees |
| Failure | Returns an error | Retries, dedupes, dead-letters |
| Deployment | Library in the caller | Often a separate edge service |

A provider client should not force a database, an outbox, and a queue on every
consumer. A webhook edge should not reimplement Meta, Stripe, and Shopify
signature quirks. Split them.

---

## Scope

### Core types

```rust
pub struct DeliveryId(pub Uuid);
pub struct ProviderId(pub &'static str);
pub struct IdempotencyKey(pub String);

pub struct InboundRequest {
    pub provider: ProviderId,
    pub headers: HeaderMap,
    pub body: Bytes,          // raw bytes, never re-serialized before verification
    pub received_at: DateTime<Utc>,
}

pub struct VerifiedEvent {
    pub delivery: DeliveryId,
    pub provider: ProviderId,
    pub event_id: Option<String>,
    pub event_type: String,
    pub payload: serde_json::Value,
    pub raw: Bytes,
}
```

### Traits

```rust
pub trait Verifier {
    fn verify(&self, req: &InboundRequest) -> Result<Verified, VerifyError>;
}

pub trait DeliveryStore {
    async fn insert_if_new(&self, delivery: &DeliveryRecord) -> Result<InsertOutcome, StoreError>;
    async fn mark_processed(&self, id: DeliveryId) -> Result<(), StoreError>;
    async fn mark_failed(&self, id: DeliveryId, err: &str) -> Result<(), StoreError>;
}

pub trait Outbox {
    async fn enqueue(&self, event: VerifiedEvent) -> Result<(), OutboxError>;
}
```

### Flow

```
receive raw bytes
  -> resolve tenant + provider
  -> verify signature with the provider verifier (constant-time)
  -> check timestamp tolerance / replay window
  -> insert delivery if new (unique on provider + event id)
  -> enqueue to outbox in the same transaction
  -> return 2xx only after the insert/enqueue commits
  -> processor consumes, marks processed, or dead-letters
```

The **raw body** is preserved. Parsing before verification is a security bug;
re-serializing changes the bytes and breaks signatures.

---

## Provider presets

The crate ships verifier presets, not full clients:

| Provider | Verification |
|---|---|
| Meta Graph | `X-Hub-Signature-256` HMAC-SHA256, `hub.challenge` echo |
| Stripe | `Stripe-Signature` HMAC-SHA256 with timestamp tolerance |
| Shopify | `X-Shopify-Hmac-Sha256` HMAC-SHA256, base64 |
| Twilio | `X-Twilio-Signature` HMAC-SHA1 over URL + sorted params |
| SendGrid | ECDSA event-webhook signature |
| Slack | `X-Slack-Signature` + `X-Slack-Request-Timestamp` |
| GitHub | `X-Hub-Signature-256` HMAC-SHA256 |
| Generic | HMAC-SHA256 hex/base64, configurable header and prefix |

A preset is a small `Verifier` implementation, not a provider client.

---

## Non-goals

- **Not a business handler.** It does not know what an `order.paid` means.
- **Not a queue.** It writes to an outbox; the queue is the caller's choice.
- **Not an auth system.** Tenant resolution is supplied by the caller.
- **Not a public webhook sender.** Outbound webhooks are a separate concern
  (signing, retries, customer endpoints).
- **Not a replay cache only.** A cache TTL is not a durable dedupe guarantee;
  the delivery store is.

---

## Integration

1. The edge service resolves the tenant from the path or a provider-specific
   lookup, then constructs `InboundRequest`.
2. `webhook-gateway` verifies, dedupes, and writes the delivery + outbox row in
   one transaction.
3. A worker consumes the outbox, calls the domain handler, and marks the
   delivery processed.
4. A failure goes to the dead-letter table with the raw body and error; the
   dashboard can replay it.
5. Metrics: received, verified, duplicate, rejected, processed, dead-lettered,
   p95 processing latency.

The OmniStream `webhook-ingress` service is a thin deployment around this
crate; the crate owns the invariants.

---

## Test plan

- Valid, tampered, and stale signatures per provider preset.
- Raw body preservation: a JSON re-serialization must fail verification.
- Duplicate event ID returns `Duplicate`, not a second enqueue.
- Same event ID from two tenants is not a duplicate.
- Crash between insert and enqueue: the outbox row is still committed.
- Crash after enqueue before processed: replay is idempotent.
- Dead-letter and manual replay.
- Timestamp tolerance and clock skew.
- Large body limits and content-type handling.
- Tenant isolation: a delivery cannot be read by another tenant.

---

## Open questions

1. **At-least-once or exactly-once?** At-least-once plus idempotent handlers is
   the only honest contract. Document it.
2. **Ordering.** Per-tenant, per-provider ordering is useful; global ordering
   is not. Define the partition key.
3. **Delivery store.** Postgres is the default; is a Redis-only mode worth it?
4. **Edge vs library.** Some consumers will embed it in an axum app; others
   want a standalone binary. Ship the library first, a binary later.
5. **Outbound webhooks.** Keep out of scope, or add a signing/retry companion?
