# Technical Plan - m4-media-links v1

> **Feature:** `m4-media-links`
> **Version:** v1
> **Status:** backlog
> **Spec:** `.sdrive/specs/backlog/m4-media-links/v1/spec.md`
> **Branch:** `feature/m4-media-links`
> **Base:** `main`

## Summary

This plan turns BP-022 to BP-027 into six production crates, in dependency-free phases, on one branch:

1. `ffscene` — deterministic scene-to-FFmpeg compilation, validation, encoder ladder, encoder probe, output descriptors.
2. `mixbus` — real-time-safe gain, pan, gate, compressor, ducker, loudness, latency reporting, lock-free meters.
3. `replay-recorder` — packet-in isolated tracks, segment/tee/ring sinks, disk budget, crash recovery, clip extraction.
4. `studio-protocol` — envelope, messages, permissions, pure state machine, version negotiation, throughput budget.
5. `rtmp-publish` — destination model, redaction, shape validation, probes, retry, circuit breaker, failover, bitrate hints.
6. `signedlink` — HMAC-SHA256 signing, base62 codes, UTM normalisation, attribution window, store-backed revocation and single use.

A seventh phase enforces the workspace baseline: lints, rustdoc, package metadata, examples, feature independence, and packaging. The feature stops at **release-ready**; publication from `main` is a post-merge action.

Each crate is independent: the phases can be reviewed separately, and every phase leaves the workspace compiling.

## Architecture Approach

### Workspace shape

The root `Cargo.toml` already declares `members = ["crates/*"]` (CON-101), so adding six directories needs no root manifest change. Every crate inherits `edition`, `rust-version`, `license`, and `repository` from `[workspace.package]` (CON-102) and follows the `dado` conventions: `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, `//!` module docs that state scope and non-goals, and manual error types with stable codes (CON-201, CON-202, CON-203, CON-205).

The existing `.github/workflows/ci.yml` runs `--workspace`, so new crates are covered without a workflow edit. The plan keeps that file as `reuse`.

### `ffscene` — scene to filter graph

```
Scene { canvas, background, layers }
  -> validate (cycles, sources, transforms, blend modes)
  -> normalise labels (deterministic, collision-free)
  -> build nodes (scale, crop, overlay, blend, setpts, format)
  -> emit filter_complex + args + LayerLabelMap
  -> Ladder::expand -> one graph per rung (split + scale)
  -> EncoderProbe -> selected encoder + explicit fallback reason
  -> Output::{File, Rtmp, Srt, Whip} descriptor (data only)
```

- `compile(&Scene, &Output) -> Result<CompiledGraph, CompileError>` is a pure function. No I/O, no clock, no process (FR-005, NFR-001).
- Node order is derived from `(z, layer id, source id)`, never from hash iteration order (FR-006).
- Labels are `v_<layer>`, `a_<layer>`, `out_<rung>` after normalisation; collisions get a deterministic numeric suffix (EC-004).
- `Ladder::expand` emits `split` followed by ordered `scale` nodes, one output per rung (FR-003).
- The encoder probe takes an injected list of available encoders, so a build machine needs no FFmpeg binary (AC-004, EC-005).

Modules: `scene`, `transform`, `validate`, `compile`, `ladder`, `probe`, `output`, `label`.

### `mixbus` — real-time mixing graph

```
Control thread: MixerConfig -> MixPlan (build, validate, pre-allocate)
                       |  swap at block boundary
Audio thread:   process(&mut Mixer, &[f32], &mut [f32]) -> MeterSnapshot (lock-free publish)
```

- `Mixer::process` walks a pre-built plan over pre-allocated buffers. No `Vec` growth, no `Mutex`, no `log`, no `await` (FR-007, NFR-002).
- Control operations (`set_gain`, `set_pan`, `set_mute`, `set_solo`, `reroute`) push commands onto a bounded queue; the audio thread drains it at a block boundary (FR-009).
- Blocks: `Gain`, `Pan` (constant power), `Gate` (hysteresis + hold), `Compressor`, `Ducker` (sidechain key), `Loudness` (K-weighted integrated LUFS + true-peak limiter) (FR-008).
- Each insert reports `latency_frames`; the plan sums them and delays parallel paths so outputs stay aligned (FR-011).
- Meters are written to a double-buffered snapshot (`AtomicBool` swap plus fixed arrays); readers never touch the audio thread (FR-010).
- NaN/infinite input is sanitised and flagged (EC-007).
- No device, decoder, thread, or runtime dependency (FR-012, CON-108 analogue).

Modules: `mixer`, `plan`, `block`, `gain`, `pan`, `gate`, `compressor`, `ducker`, `loudness`, `meter`, `error`.

### `replay-recorder` — tracks, ring, disk budget

```
Packet stream -> TrackWriter -> Segment { path, index entry }
                            -> TeeWriter (local + duplicate)
                            -> RingWriter (rotation, clip window)
                            -> BudgetGuard (max_bytes, min_free_bytes, policy)
Recovery: scan spool -> validate index -> finalise or discard partial segments
```

- The crate accepts already-encoded packets (`TrackCodec`, `Container`); it does not encode (FR-013, FR-019).
- Spool paths come from validated `SessionId`/`TrackId` newtypes; `..`, separators, and absolute paths are rejected (NFR-015, EC-010).
- `index.json` is written to `index.json.tmp` then renamed after each segment close (FR-018, EC-009).
- The budget guard checks before opening a segment and while writing; `PressurePolicy` decides what stops (FR-016, NFR-005).
- The ring keeps a bounded set of recent segments and `clip(last)` concatenates the covering range without stopping the writer (FR-015).
- Timeline uses per-track monotonic sequence numbers, not wall-clock deltas (FR-013, EC-011).

Modules: `recorder`, `track`, `sink`, `segment`, `ring`, `budget`, `spool`, `index`, `error`.

### `studio-protocol` — control plane

```
Envelope { v, id, at, from, body }
  -> decode (serde_json, deny_unknown_fields on payloads)
  -> version check (major must match, minor may add fields)
  -> duplicate check (message id set)
  -> permission check (role -> permission set, server-side)
  -> apply (pure: state + message + actor -> new state | rejection)
```

- `StudioState` holds program scene, preview scene, participants, and overlay stack (FR-022).
- A program cut is one transition that swaps program and preview atomically; no intermediate state exposes both (AC-022).
- The permission table is a single `match` over `(Role, MessageKind)`; the check runs before `apply` (FR-021, NFR-012).
- Duplicate IDs are rejected idempotently; out-of-order messages are rejected unless they are `Ping`/`Pong`, as documented (FR-024).
- The budget is a per-participant fixed window; excess returns `Rejected::RateLimited` (FR-025, NFR-013).
- JSON only in 0.1; no transport (FR-023).

Modules: `envelope`, `message`, `permission`, `state`, `transition`, `version`, `budget`, `error`.

### `rtmp-publish` — egress destinations

```
Destination { id, label, protocol, url, key: SecretString, region, backup }
  -> validate shape (scheme, host, port, key)
  -> probe (TCP connect | HTTP OPTIONS/HEAD via reqwest rustls)
  -> Health { Unknown, Reachable { rtt }, AuthRejected, Unreachable { kind, reason } }
  -> RetryPolicy (bounded backoff + jitter) -> CircuitBreaker (per destination)
  -> Failover (backup promotion, cool-down demotion)
  -> bitrate_hint(observed) -> bounded hint
```

- `Debug`/`Display` are implemented by hand; the key prints `***`. `Destination` does not implement `Serialize` for the key; a separate `RedactedDestination` view is the only serialisable shape (FR-026, NFR-004).
- Validation rejects `http://` for WHIP and any plaintext credential transport (FR-027, NFR-017, EC-015).
- Probes use an injected connector/clock; the default TCP probe and the optional `reqwest` feature are separable (FR-028).
- Backoff uses `base * 2^n` capped, with jitter from an injected `Jitter` source so behaviour is reproducible (FR-029).
- The breaker is per destination and per backup; failover refuses self-referential backups (FR-030, EC-018).
- No wire protocol, muxing, encoding, or CDN orchestration (FR-032).

Modules: `destination`, `secret`, `validate`, `probe`, `retry`, `breaker`, `failover`, `bitrate`, `error`.

### `signedlink` — signed links

```
Claims { target, campaign, tenant, attribution_window, single_use }
  -> canonical bytes (sorted UTM, versioned format)
  -> HMAC-SHA256(key)
  -> base64url(payload).base64url(mac)
  -> SignedUrl { code, url }
verify(url, now) -> stateless signature + expiry + skew check -> store lookup (revocation, single use)
```

- `SigningKey` is a secret newtype with redacted `Debug` and no `Serialize` (NFR-004).
- HMAC verification uses constant-time comparison; tamper, expiry, and skew fail closed (FR-033, AC-033..AC-035).
- Codes are base62 from `getrandom`, collision-checked through `LinkStore` with a bounded retry (FR-034, EC-022).
- UTM keys are a closed set; unknown keys are rejected (FR-035, EC-021).
- `LinkStore` is mandatory when revocation or single use is enabled; the store consumes nonces atomically (FR-037, FR-038).
- The `axum` feature adds only the redirect handler: resolve, verify, record, redirect with `Referrer-Policy: no-referrer` and `Cache-Control: no-store` (FR-039, EC-023).

Modules: `claims`, `signer`, `code`, `utm`, `store`, `verify`, `redirect` (feature `axum`), `error`.

### Security baseline implementation map

| Control | Implementation |
|---|---|
| CON-511 Authentication | Caller-supplied `SigningKey`/`SecretString`; presence and shape validation; fail-closed verification. |
| CON-512 Authorization | `permission::check` before every transition; store-enforced revocation and nonce consumption. |
| CON-514 Rate Limiting | Bounded retry + breaker in `rtmp-publish`; per-participant window in `studio-protocol`. |
| CON-515 Input Validation | Validation modules at every public entry point; typed errors only. |
| CON-516 Injection Protection | ID newtypes for spool paths; no SQL/Redis backend; store contract documents parameterised access. |
| CON-519 Environment Security | No `.env`, no embedded keys; keys arrive as arguments and are redacted. |
| CON-520 HTTPS / HSTS | TLS-only schemes for probes and redirects; HSTS noted as an edge responsibility in READMEs. |

CON-513, CON-517, and CON-518 stay NOT APPLICABLE: no HTML, no cookies, and no global header pipeline. The redirect handler sets only its own two headers.

## Anti-Redundancy Check

Performed before planning new components:

- `crates/` contains only `dado`; `Get-ChildItem crates` returns `dado`.
- Content search for `StudioState`, `LinkStore`, `CompiledGraph`, `Mixer`, `DiskBudget`, `Destination`, and `Signer` under `crates/` returns no matches.
- `.sdrive/skills/` does not exist, so there are no project-specific skill rules to read (CON-707).
- `node .sdrive/scripts/anti-redundancy.js` reports `create` for all six packages and all six conceptual services/models, with no existing location.
- Existing assets marked `reuse`: root `Cargo.toml` (`crates/*` members, workspace package fields), `.github/workflows/ci.yml` (workspace-wide jobs), `LICENSE-MIT`, `LICENSE-APACHE`, and the `dado` crate layout/documentation conventions.
- No planned dependency is already installed: the workspace has no shared dependency table and `dado`'s dependencies (`serde`, `winnow`, `indexmap`) are optional to `dado` only.

## Data Model

### `ffscene`

| Entity | Fields | Action |
|---|---|---|
| `Canvas` | `width: u32`, `height: u32`, `fps: Rational` | create |
| `Scene` | `canvas`, `background`, `layers: Vec<Layer>` | create |
| `Layer` | `id`, `source`, `transform`, `z`, `visible`, `blend`, `opacity` | create |
| `EncoderProfile` | `codec`, `rate_control`, `preset`, `keyint`, `profile` | create |
| `Ladder` | `rungs: Vec<Rung>` | create |
| `CompiledGraph` | `filter_complex`, `inputs`, `outputs`, `layer_map`, `args` | create |

### `mixbus`

| Entity | Fields | Action |
|---|---|---|
| `Mixer` | `sample_rate`, `block`, `buses`, `sources`, `plan` | create |
| `Bus` | `id`, `gain_db`, `mute`, `solo`, `inserts`, `sends` | create |
| `Source` | `id`, `gain_db`, `pan`, `duck` | create |
| `Duck` | `key`, `threshold_db`, `ratio`, `attack_ms`, `release_ms` | create |
| `MeterSnapshot` | `peak`, `rms`, `lufs`, `gain_reduction`, `flags` | create |

### `replay-recorder`

| Entity | Fields | Action |
|---|---|---|
| `Recorder` | `tracks`, `sink`, `budget`, `session` | create |
| `Track` | `id`, `source`, `codec`, `container`, `enabled` | create |
| `Sink` | `Segment`, `Tee`, `Ring` | create |
| `DiskBudget` | `max_bytes`, `min_free_bytes`, `on_pressure` | create |
| `SegmentIndex` | `session`, `track`, `segments` | create |

### `studio-protocol`

| Entity | Fields | Action |
|---|---|---|
| `Envelope` | `v`, `id`, `at`, `from`, `body` | create |
| `Message` | `Preview`, `Program`, `ScenePatch`, `Overlay`, `Role`, `Whisper`, `Teleprompter`, `Ping`, `Pong` | create |
| `Permission` | `View`, `ControlScene`, `ControlOverlay`, `ControlProgram`, `ControlRoles`, `ControlTeleprompter` | create |
| `StudioState` | `program`, `preview`, `participants`, `overlays`, `seen_ids` | create |

### `rtmp-publish`

| Entity | Fields | Action |
|---|---|---|
| `Destination` | `id`, `label`, `protocol`, `url`, `key`, `region`, `backup` | create |
| `Health` | `Unknown`, `Reachable`, `AuthRejected`, `Unreachable` | create |
| `RetryPolicy` | `base`, `max_attempts`, `cap` | create |
| `CircuitBreaker` | `state`, `failures`, `opened_at` | create |
| `BitrateHint` | `floor`, `ceiling`, `observed` | create |

### `signedlink`

| Entity | Fields | Action |
|---|---|---|
| `Signer` | `key`, `ttl`, `clock_skew` | create |
| `Claims` | `target`, `campaign`, `tenant`, `attribution_window`, `single_use` | create |
| `SignedUrl` | `code`, `url` | create |
| `LinkStore` | `put`, `get`, `revoke`, `consume` | create |

## API Contracts

No HTTP service is defined by this feature. `signedlink`'s optional `axum` feature exposes a router helper for `GET /l/:code`; it is a library integration point, not a new service endpoint. Public Rust APIs are specified in the spec and summarised per module in the plan above.

## Security Considerations

| Requirement | Implementation |
|---|---|
| NFR-004 | Hand-written `Debug`/`Display` for secrets; no `Serialize` for keys; tracing fields use the redacted view. |
| NFR-011 | Key presence and shape validated before use; `verify` fails closed; missing key is `InvalidKey`. |
| NFR-012 | Permission check precedes every transition; revocation and single use require the store. |
| NFR-013 | Backoff and breaker bounded per destination; fixed per-participant message window. |
| NFR-014 | Validation at every public constructor and decode path; no `unwrap` on untrusted input. |
| NFR-015 | Newtype IDs sanitise spool paths; no SQL/Redis backend ships. |
| NFR-016 | No `.env`; no key material in the repository; keys are arguments. |
| NFR-017 | TLS-only probe and redirect schemes; plaintext WHIP rejected. |

## Performance Considerations

| Requirement | Strategy |
|---|---|
| NFR-001 | Deterministic ordering via sorted keys; no timestamps or addresses in the graph. |
| NFR-002 | Pre-allocated buffers; fixed-capacity command queue; atomic meter swap. |
| NFR-005 | Budget checked before and during writes; segments bounded; free-space check uses metadata, not a directory walk per packet. |
| NFR-006 | Fixed-size duplicate window; constant-time per-message work; no unbounded queue. |
| NFR-013 | Bounded exponential backoff; breaker transitions are O(1). |

## Phases

| Phase | Name | Depends on | Requirements |
|---|---|---|---|
| P1 | `ffscene` | — | FR-001..FR-006, NFR-001 |
| P2 | `mixbus` | — | FR-007..FR-012, NFR-002 |
| P3 | `replay-recorder` | — | FR-013..FR-019, NFR-005, NFR-015 |
| P4 | `studio-protocol` | — | FR-020..FR-025, NFR-006, NFR-012 |
| P5 | `rtmp-publish` | — | FR-026..FR-032, NFR-011, NFR-013, NFR-017 |
| P6 | `signedlink` | — | FR-033..FR-039, NFR-004, NFR-012 |
| P7 | Workspace baseline and hardening | P1..P6 | FR-040..FR-045, NFR-003, NFR-007..NFR-010, NFR-014, NFR-016 |

P1 to P6 are independent and can be implemented in any order. P7 runs last because it checks the whole workspace.

## Task Summary

| Phase | Tasks | Focus |
|---|---|---|
| P1 | T1.1–T1.7 | Types, validation, deterministic compile, ladder, probe, outputs, example |
| P2 | T2.1–T2.8 | Types and buffers, gain/pan, gate/compressor, ducker, loudness, plan swap/latency, meters, example |
| P3 | T3.1–T3.8 | Types and packet API, spool/index, segments, tee, ring, budget, recovery, example |
| P4 | T4.1–T4.7 | Envelope/messages, permissions, state machine, versioning, ordering, budget, example |
| P5 | T5.1–T5.7 | Destination/redaction, validation, probe, retry/breaker, failover, bitrate, example |
| P6 | T6.1–T6.7 | Types/store, HMAC, codes, UTM, single use/revocation, redirect, example |
| P7 | T7.1–T7.7 | Metadata, examples, feature matrix, lints/docs, packaging, security review, CI coverage |

## Dependencies

| Name | Type | Action | Notes |
|---|---|---|---|
| `serde` | library | add | Optional; derive for JSON-facing crates. |
| `serde_json` | library | add | Optional; index, messages, claims. |
| `uuid` | library | add | Optional, `v7`; protocol IDs. |
| `chrono` | library | add | Optional; protocol and link time. |
| `url` | library | add | Destination and target parsing. |
| `hmac` | library | add | HMAC-SHA256. |
| `sha2` | library | add | HMAC digest. |
| `base64` | library | add | URL-safe token encoding. |
| `getrandom` | library | add | OS entropy for base62 codes. |
| `reqwest` | library | add | Optional, `rustls-tls`; WHIP probe only. |
| `axum` | library | add | Optional; redirect handler. |
| `crates.io` | service | no_change | Publish after merge; re-check names. |

No dependency is enabled by default unless the crate's primary operation needs it. Every dependency must build on Rust 1.85.

## Risks

- RISK-001: Six crates in one review. Mitigation: independent phases, each compiling on its own.
- RISK-002: Real-time guarantees. Mitigation: fixed buffers, bounded queue, aborting-allocator test by `/sdrive:test`.
- RISK-003: FFmpeg dialect drift. Mitigation: target FFmpeg 7.x and document the boundary in the `ffscene` README.
- RISK-004: Key leakage. Mitigation: redaction by construction and a property test over key formats.
- RISK-005: Disk-budget races. Mitigation: single-writer per track, pre-write checks, atomic index rename.
- RISK-006: Permission drift. Mitigation: one table, one check call site, matrix test.
- RISK-007: MSRV break. Mitigation: optional dependencies, MSRV job.
- RISK-008: Stateless-link confusion. Mitigation: store mandatory for revocation, documented boundary.
- RISK-009: Duplication across crates. Mitigation: accept small duplication; no shared framework crate.
- RISK-010: Scope creep. Mitigation: out-of-scope list enforced in review.

## Definition of Done

- [ ] Six crates exist under `crates/*` and build individually and as a workspace.
- [ ] Every FR and NFR has a linked AC; every FR has at least one task (NFR-018 is satisfied by `/sdrive:test`).
- [ ] Every AC has a tentative test row for `/sdrive:test`.
- [ ] Security baseline controls are implemented or documented as not applicable.
- [ ] `cargo fmt --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo doc --workspace --all-features`, and `cargo package --workspace` pass.
- [ ] `/sdrive:test` supplies the test evidence before `/sdrive:review`.
