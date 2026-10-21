# syncforge

A durable action queue with UUID v7 idempotency, per-action batch results, and
explicit conflict strategies for offline-first Rust clients.

The action is persisted before it is acknowledged, replayed in order, and
settled per action so one poison or retrying action cannot block the rest.

## Features

This crate has no optional features in 0.1. `Storage` and `Transport` are
traits; `MemoryStorage` and `MockTransport` are the reference implementations.

## Flow

1. `SyncEngine::enqueue` persists the action and returns only after the storage
   write succeeds.
2. `SyncEngine::replay` drains pending actions in bounded batches.
3. Each action receives exactly one `ActionResult`: `Applied`, `Duplicate`,
   `Conflict`, `Rejected`, or `Retry`.
4. Conflicts are resolved by the configured strategy: `LastWriteWins`,
   `ServerWins`, `ClientWins`, or `Custom`.
5. A retryable action stays pending with bounded backoff; after `max_attempts`
   it is marked failed and later actions still replay.

The contract is at-least-once, not exactly-once. Server handlers must be
idempotent on the action id.

## Non-goals

- No SQLite storage or HTTP transport in 0.1.
- No connectivity detection and no WASM packaging.
- No server-side conflict database; the consumer supplies server state.

## License

MIT OR Apache-2.0.
