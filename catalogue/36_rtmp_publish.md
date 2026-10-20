# 36 — rtmp-publish

**Package:** `rtmp-publish` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library, optional `reqwest` health probe
**Licence:** MIT OR Apache-2.0

`rtmp-publish` models an egress destination for live video — RTMP, RTMPS, SRT,
or WHIP — and provides key-shape validation, health probing, retry/backoff, and
bitrate adaptation hints.

It does not ingest, encode, or speak the wire protocol itself. It prepares and
supervises a destination that FFmpeg or a transport crate connects to.

---

## Why this is not a wirekit module

`wirekit` is an HTTP provider client. RTMP and SRT are not HTTP, and a
destination is usually a bare URL and key with no API at all. The one HTTP-shaped
part — asking a platform for a stream key — is a `wirekit` platform module, not
this crate.

---

## Scope

### Core types

```rust
pub struct Destination {
    pub id: DestinationId,
    pub label: String,
    pub protocol: Protocol,   // Rtmp, Rtmps, Srt, Whip
    pub url: Url,             // redacted in Debug/Display
    pub key: SecretString,
    pub region: Option<String>,
    pub backup: Option<Box<Destination>>,
}

pub enum Health {
    Unknown,
    Reachable { rtt: Duration },
    AuthRejected,
    Unreachable { reason: String },
}
```

### Operations

- **Validate shape.** Scheme, host, port, and key presence. Never log the key;
  `Debug` prints `***`. This is the single most important behaviour in the
  crate.
- **Probe.** TCP connect (RTMP/SRT) or an HTTP `OPTIONS`/`HEAD` (WHIP) with a
  timeout. Distinguish DNS failure, refused, timeout, and auth rejection.
- **Retry policy.** Exponential backoff with jitter, a cap, and a circuit
  breaker per destination. Retry on transport failure; do not retry on
  `AuthRejected`.
- **Failover.** A backup destination is promoted when the primary breaker
  opens, and demoted after a cool-down.
- **Bitrate hints.** Given observed throughput, suggest a ceiling so the
  encoder can adapt before the buffer empties.

### Secret handling

`Destination` never implements `Serialize` for the key field. Use an explicit
`Redacted` view for API responses. A key that reaches a log, a trace span, or an
error message is a bug, not a nuisance.

---

## Non-goals

- **Not an RTMP/SRT implementation.** No handshake, no muxing.
- **Not an encoder.** No rate control.
- **Not a platform provisioning API.** That is a `wirekit` module.
- **Not a CDN manager.** No multi-region orchestration.
- **Not an ingest server.** Receive-side is a different crate.

---

## Integration

1. The app stores destinations with the key encrypted at rest.
2. Before going live, probe each destination and surface an honest status.
3. The media engine builds an FFmpeg output from the destination.
4. On failure, `rtmp-publish` decides retry, failover, or stop; the encoder
   receives a bitrate hint.

---

## Test plan

- `Debug`/`Display`/error payloads never contain the key (property test).
- Probe distinguishes DNS, refused, timeout, and auth rejection.
- Backoff is bounded and jittered; the breaker opens and closes correctly.
- Failover promotes the backup and does not flap.
- Bitrate hint never exceeds the configured maximum.
- Invalid scheme/port/key shapes are rejected before connecting.

---

## Open questions

1. **Crate name.** `rtmp-publish` is free. Confirm before publish.
2. **Protocol scope.** Is WHIP in 1.0, or later? It is HTTP-shaped and easier
   than SRT.
3. **Probe depth.** A TCP connect is cheap but proves little. Offer an optional
   deeper RTMP handshake probe behind a feature?
4. **Key rotation.** Does the crate support two active keys during rotation?
