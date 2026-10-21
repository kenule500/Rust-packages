# hookgate

Inbound webhook verification on raw bytes, replay windows, durable
deduplication, an outbox, and dead-letter replay.

`hookgate` owns the part of webhook handling that must survive a restart: the
delivery record, the idempotency key, the replay window, and the handoff to a
queue.

## Features

| Feature | Default | Adds |
|---|---|---|
| `presets` | yes | HMAC verifiers for Meta Graph, Stripe, Shopify, Twilio, Slack, GitHub, and a generic HMAC-SHA256 verifier |

Core types and the `Verifier`, `DeliveryStore`, and `Outbox` traits are always
available.

## Presets

| Preset | Header | Digest | Encoding | Signed payload |
|---|---|---|---|---|
| Meta Graph | `X-Hub-Signature-256` | SHA-256 | hex after `sha256=` | raw body |
| Stripe | `Stripe-Signature` | SHA-256 | hex `v1` values | `{t}.{body}` |
| Shopify | `X-Shopify-Hmac-Sha256` | SHA-256 | base64 | raw body |
| Twilio | `X-Twilio-Signature` | SHA-1 | base64 | public URL plus sorted form fields |
| Slack | `X-Slack-Signature` | SHA-256 | hex after `v0=` | `v0:{timestamp}:{body}` |
| GitHub | `X-Hub-Signature-256` | SHA-256 | hex after `sha256=` | raw body |
| Generic | caller-supplied | SHA-256 | hex | raw body |

Twilio signs the externally visible URL. Forward it in `X-Forwarded-Url`, or
override the header with `HmacPreset::signed_payload` and
`SignedPayload::TwilioForm`.

Every preset is an `HmacPreset`, so the header, encoding, prefix, digest,
payload shape, event sources, tolerance, and body limit are all configurable.
Signatures are compared in constant time. The default tolerance is five
minutes and the default body limit is one MiB.

## Flow

1. Build an `InboundRequest` that owns the raw `Bytes` body.
2. Run a `Verifier` over the raw bytes. The body is never re-serialized first.
3. `DeliveryStore::insert_if_new` deduplicates on tenant, provider, and provider
   event id.
4. The delivery and the outbox row commit together.
5. Failures beyond `max_attempts` become a `DeadLetterRecord` that can be
   replayed with the same delivery identity.

The contract is at-least-once. Consumers must be idempotent on the delivery id.

The dedupe key is `(tenant, provider, event_id)`. Some providers omit an event
id on some payloads; supply a provider-specific fallback before building a
`DeliveryRecord`. The delivery id remains the stable identity for replay.

## Dead letters

`MemoryDeliveryStore::fail` increments the attempt count and, at the configured
maximum, writes a `DeadLetterRecord` with the exact raw body and redacted error
text. `replay` re-enqueues the same delivery id; a delivery that already
succeeded returns `Duplicate` and is not enqueued again.

Replay and handler delivery are at-least-once. A crash between the handler and
`mark_processed` causes a retry, so handlers must be idempotent on the delivery
id.

## Non-goals

- No provider client and no outbound HTTP.
- No HTTP server or `axum` integration in 0.1.
- No SendGrid ECDSA verifier in 0.1.
- No standalone binary in 0.1.

## License

MIT OR Apache-2.0.
