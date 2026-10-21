# syncforge

A durable action queue with UUID v7 idempotency, per-action batch results, and
explicit conflict strategies for offline-first Rust clients.

The action is persisted before it is acknowledged, replayed in order, and
settled per action so one poison or retrying action cannot block the rest.

## Features

This crate has no optional features in 0.1. `Storage` and `Transport` are
traits; `MemoryStorage` and `MockTransport` are the reference implementations.
Both are injected explicitly, so there is no ambient global queue.

## Flow

1. `SyncEngine::enqueue` persists the action and returns only after the storage
   write succeeds.
2. `SyncEngine::replay` snapshot-drains pending actions in bounded batches.
3. Each action receives exactly one `ActionResult`: `Applied`, `Duplicate`,
   `Conflict`, `Rejected`, or `Retry`.
4. `Applied` and `Duplicate` settle the action, a resolved `Conflict` settles
   it, and `Rejected` marks it failed.
5. A retryable action stays pending with an incremented attempt count. At
   `max_attempts` it is marked failed and reported as `Rejected`; later actions
   still replay in the same run.

The contract is at-least-once, not exactly-once. Storage settlement and the
transport call are not transactional, so a crash between a successful send and
the local settle can replay an action. Server handlers must be idempotent on the
action id or idempotency key.

## Conflict strategies

| Strategy | Decision |
| --- | --- |
| `LastWriteWins` | The later client or server timestamp wins; ties favour the server. |
| `ServerWins` | The server state is kept unchanged. |
| `ClientWins` | The client payload is kept. |
| `Custom` | The caller's `CustomMerger` returns the resolution. |

A conflict must include the server state. A conflict without it returns
`SyncError::ConflictWithoutServerState`. A custom merger can return
`Resolution::Merged(value)`; the merged value is recorded in the result for the
caller to apply. The engine does not push the merged value back to the server.

## Timestamps and tolerance

The client timestamp is captured at action creation and is never rewritten
during replay. The server's timestamp is recorded in
`ActionResult::server_timestamp` and in `ConflictInfo`. The default tolerance is
five minutes; `SyncEngine::timestamp_decision` returns the signed clock skew and
whether the pair is within tolerance. `LastWriteWins` still chooses the later
timestamp regardless of tolerance; the tolerance is audit data.

## Retries and backoff

Retries use a capped exponential delay exposed by `SyncEngine::retry_delay`:
250 ms for the first retry, doubling per attempt, capped at 30 seconds. The
engine never sleeps or blocks; the host scheduler decides when to call `replay`
again. A dead-lettered action does not stop the remaining pending actions.

## Tracing

With a `tracing` subscriber installed, the engine emits:

- `syncforge.enqueue`: action id and queue depth.
- `syncforge.replay.start` and `syncforge.replay.finish`: pending depth, batch
  size, result count, conflict count, final queue depth, and duration.
- `syncforge.action.outcome`, `syncforge.action.retry`, and
  `syncforge.action.dead_letter`: per-action disposition and attempt count.
- `syncforge.conflict`: strategy, resolution, clock skew, and tolerance.

## Non-goals

- No SQLite storage or HTTP transport in 0.1.
- No connectivity detection and no WASM packaging.
- No server-side conflict database; the consumer supplies server state.
- No automatic merge application; the resolution is returned to the caller.

## License

MIT OR Apache-2.0.
