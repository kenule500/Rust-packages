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

Enable the `postgres` feature and apply `src/postgres/schema.sql`. Connections
require TLS by default; an explicit test-only insecure mode exists for local
development.

## License

MIT OR Apache-2.0.
