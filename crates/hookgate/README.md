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

## Flow

1. Build an `InboundRequest` that owns the raw `Bytes` body.
2. Run a `Verifier` over the raw bytes. The body is never re-serialized first.
3. `DeliveryStore::insert_if_new` deduplicates on tenant, provider, and provider
   event id.
4. The delivery and the outbox row commit together.
5. Failures beyond `max_attempts` become a `DeadLetterRecord` that can be
   replayed with the same delivery identity.

The contract is at-least-once. Consumers must be idempotent on the delivery id.

## Non-goals

- No provider client and no outbound HTTP.
- No HTTP server or `axum` integration in 0.1.
- No SendGrid ECDSA verifier in 0.1.
- No standalone binary in 0.1.

## License

MIT OR Apache-2.0.
