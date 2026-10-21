# rtmp-publish

Destination modelling and publishing resilience without a wire protocol.
`rtmp-publish` validates RTMP, RTMPS, SRT, and WHIP destinations, probes them
shallowly, plans bounded retries, breaks circuits per destination, fails over to
a backup, and produces bounded bitrate hints.

## Scope

- `Destination` with protocol, URL, secret key, optional region, and optional
  backup chain.
- Shape validation before any connection attempt.
- TCP probes for RTMP/RTMPS/SRT; HTTP `HEAD` probes for WHIP behind the optional
  `whip` feature.
- Bounded exponential backoff with injected jitter.
- Per-destination circuit breaker.
- Backup promotion and cool-down demotion in one state machine.
- Bitrate hints clamped to a configured floor and ceiling.

## Non-goals

- Implementing RTMP, SRT, or WHIP handshakes, publishing, muxing, encoding, or
  CDN orchestration.
- Persisting keys or reading `.env` files.
- HSTS. HSTS is the deployment edge's responsibility.

## Redaction

The stream key is `SecretString`:

- `Debug` and `Display` print `***`.
- It does not implement `Serialize`, `Deserialize`, or equality.
- `RedactedDestination` is the only serialisable view and carries only
  `has_key`.
- Errors never include the key, the URL query, or userinfo.

The URL must not carry credentials in userinfo or in a `key`, `token`,
`secret`, `password`, `passphrase`, `stream_key`, `streamkey`, or `auth` query
parameter. Those are rejected as `RTMP-009`.

## Supported protocols

| Protocol | Accepted URL scheme | Default port | Probe |
|---|---|---|---|
| `rtmp` | `rtmp` | none; must be explicit | TCP connect |
| `rtmps` | `rtmps` | none; must be explicit | TCP connect |
| `srt` | `srt` | none; must be explicit | TCP connect |
| `whip` | `https`, `whip+https` | 443 | HTTP `HEAD` with `whip` feature |

Plain `http` is not a valid credential-bearing scheme and is rejected. RTMPS and
WHIP are TLS-capable; the caller is responsible for certificate trust. A TCP
probe observes only the TCP layer, so `FailureKind::Tls` is produced by an
injected connector that performs the handshake.

## Probe depth

A probe never publishes and never sends the key.

- TCP: resolve, connect with a timeout, record latency, close.
- WHIP: one HTTP `HEAD`; `401`/`403` becomes `AuthRejected`, any other response
  becomes `Reachable`.
- DNS failure, refusal, timeout, TLS failure, authentication rejection,
  unsupported, and other failures are distinct `FailureKind` values.

`probe(destination, config, connector)` re-validates before probing. Inject a
`Connector` (and, with `whip`, an `HttpProbe`) for deterministic behaviour.

## Retry

`RetryPolicy` computes `base * 2^attempt`, caps it, then scales by injected
jitter in `0.5..=1.0` of that value. It retries transport failures only.
`AuthRejected`, `Reachable`, and `Unknown` produce an empty plan. At most 10
retries are allowed.

## Breaker and failover

`CircuitBreaker` opens after `failure_threshold` consecutive failures and allows
a half-open probe after `cooldown`. A reachable probe closes it. Auth rejection
counts as a failure because the same key cannot succeed.

`BreakerSet` stores one breaker per destination, so memory is bounded by the
destination set. `Failover` promotes the backup when the primary breaker is
open, and demotes only after the cool-down, a closed breaker, and a reachable
primary. It stays on the backup otherwise, avoiding flapping.

## Bitrate hints

`BitrateHint::target` returns `observed.clamp(floor, ceiling)`. The floor must be
non-zero and the ceiling at least the floor. The calculation is pure and
saturating.

## Features

- `serde`: `Serialize` for `RedactedDestination`.
- `whip`: `reqwest` blocking HTTP probe with `rustls-tls`.

## Example

```text
cargo run -p rtmp-publish --example plan_destination
```
