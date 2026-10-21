# quota-ledger

Per-tenant, per-provider quota reservations with commit, release, exhaustion
policies, and an optional Postgres ledger.

`quota-ledger` answers one question before an outbound call: may this tenant
spend one more unit of this provider's quota right now, and what happens if
they cannot?

## Features

| Feature | Default | Adds |
|---|---|---|
| `postgres` | no | `PostgresQuotaStore` and the SQL schema |

The default build is dependency-light and uses `MemoryQuotaStore`. The
Postgres backend is opt-in and never compiled by default.

## Flow

1. `reserve` checks capacity and records a hold atomically.
2. The caller performs the outbound call.
3. `commit` consumes the reservation exactly once, or `release` returns it.
4. `usage` reports the limit, used, held, overage, and the next reset instant.

Reservations carry an optional caller idempotency key. Repeating the same key
with the same payload returns the same reservation; repeating it with a
different payload is an `IdempotencyConflict`.

## Non-goals

- No process-local rate limiting.
- No billing, invoicing, currency conversion, or tax logic.
- No Redis backend in 0.1.
- No `no_std` core in 0.1.

## Postgres

Enable the `postgres` feature. The backend adds three tables:

- `quota_budget` - one row per tenant, provider, metric, and window, with
  `used_units`, `held_units`, and overage counters.
- `quota_reservation` - one row per hold, with a partial unique index on
  `(tenant_id, provider_id, metric, idempotency_key)` and an index for TTL
  reclaim.
- `quota_dead_hold` - one row per reclaimed hold, used to return held units
  exactly once.

Reserve is a single transaction: a per-budget advisory lock, a conditional
`UPDATE` that only succeeds while `used + held + units <= limit`, and the hold
insert. Commit and release settle the reservation once; a repeated or late
settlement is a typed no-op.

Apply `src/postgres/schema.sql` with your migration tool. For local development
and tests, `PostgresQuotaStore::apply_schema` executes the same file.

```rust
use quota_ledger::postgres::PostgresQuotaStore;

let store = PostgresQuotaStore::connect("postgres://user:pass@db/app").await?;
store.apply_schema().await?;
```

`connect` requires TLS. `connect_insecure_for_tests` disables TLS explicitly and
must never be used in production. Connection strings are never stored in the
store or rendered in `Debug`, `Display`, or errors.

The default reservation TTL is five minutes; set it per request with
`ReserveRequest::with_ttl`. Run `PostgresQuotaStore::reclaim_expired` from a
periodic job. Reclaim is idempotent: the hold is settled once, recorded in
`quota_dead_hold` once, and its units are returned once.

## Related crates

- [`platform-core`](../platform-core/README.md) - tenant, provider, capability,
  and token-store contracts reused here.
- [`hookgate`](../hookgate/README.md) - inbound webhook verification and
  dedupe.
- [`syncforge`](../syncforge/README.md) - durable action replay for offline
  clients.
- [Workspace README](../../README.md) - package table, status, and reading
  order.

## License

MIT OR Apache-2.0.
