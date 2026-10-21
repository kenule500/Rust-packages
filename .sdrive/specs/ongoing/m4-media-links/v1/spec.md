---
title: M4 Media and Links - six media and link crates 0.1
status: backlog
version: v1
created: 2026-10-20
updated: 2026-10-20
---

# M4 Media and Links - six media and link crates 0.1

## Overview

### Problem Statement

The workspace has one published-quality crate, `dado`, and the M0 baseline. The six M4 crates described in the catalogue — `ffscene`, `mixbus`, `replay-recorder`, `studio-protocol`, `rtmp-publish`, and `signedlink` — do not exist: there is no scene compiler, no audio mixing graph, no recording/replay layer, no studio control protocol, no egress destination model, and no signed-link layer.

Without them, applications that run a live studio must each re-implement scene-to-FFmpeg compilation, real-time-safe audio mixing, replay buffering, preview/program permissions, RTMP/SRT/WHIP destination handling, and signed attribution links. Every re-implementation risks the same failures: silently overflowing encoders, allocations on the audio thread, disks filled by recordings, permission checks in the client, leaked stream keys, and forgeable or non-expiring links.

### Proposed Solution

Deliver M4 as one feature on one branch, `feature/m4-media-links`, adding six standalone crates under `crates/*`:

1. **`ffscene`** — compile a declarative `Scene` into a deterministic FFmpeg `filter_complex` graph and argument vector; validate before FFmpeg starts; expand encoder ladders; probe encoders with explicit downgrade; emit output descriptors only.
2. **`mixbus`** — a real-time-safe mixing graph with gain, constant-power pan, gate, compressor, sidechain ducker, EBU R128 loudness, latency reporting, and lock-free meters; no allocation, locking, logging, or `await` on the audio thread.
3. **`replay-recorder`** — packet-in multi-track recording with isolated tracks, a segment-rotation replay ring, segment/tee sinks, a hard disk budget, crash recovery, and clip extraction.
4. **`studio-protocol`** — envelope, message set, role-to-permission mapping, and a pure `(state, message, actor)` state machine for preview/program control; JSON only, integer protocol version.
5. **`rtmp-publish`** — destination model for RTMP, RTMPS, SRT, and WHIP with key-shape validation, redaction by construction, shallow health probes, bounded retry with a circuit breaker, failover with cool-down, and bitrate hints.
6. **`signedlink`** — HMAC-SHA256 signed, expiring links with base62 short codes, UTM normalisation, an attribution window, atomic single use through a mandatory `LinkStore`, and an optional `axum` redirect handler.

Each crate ships `forbid(unsafe_code)`, `deny(missing_docs)`, a README, a CHANGELOG, at least one runnable example, and package metadata. Publication to crates.io happens from `main` after review and archive, exactly as M0 and BP-010.

### Target Audience

- **Primary:** external Rust developers building live-studio, recording, streaming, or affiliate-link products who need composable building blocks rather than a framework.
- **Secondary:** maintainers of this workspace who must keep the six crates consistent with `platform-core` IDs and conventions without adding a framework dependency.

### Assumptions

- The six crate names (`ffscene`, `mixbus`, `replay-recorder`, `studio-protocol`, `rtmp-publish`, `signedlink`) are free on crates.io as of 2026-10-20; each MUST be re-checked immediately before publication (CON-111).
- The workspace remains rooted at the repository root with `members = ["crates/*"]`, so new crates need no root manifest change (CON-101).
- Rust 1.85 remains the MSRV and edition 2021 remains in force (CON-102).
- The six crates do not depend on `wirekit`; they may depend on shared conventions only (backlog M4 notes; CON-107).
- No crate ships application UI, product nouns, or a global singleton (CON-106, CON-109, CON-301).
- Publication is a manual post-merge step from `main`; this feature makes the crates release-ready.
- Test authoring and test execution belong to `/sdrive:test`, not to this feature's implementation tasks.
- Real FFmpeg, audio devices, and network endpoints are not required on the build machine; the crates compile graphs, process buffers, and probe connections through injectable interfaces.

### Confirmed Decisions

| Decision | Choice |
|---|---|
| Scope | BP-022 to BP-027 in one `m4-media-links` feature |
| Branch / base | `feature/m4-media-links` from `main`, merge back to `main` |
| Crate names | All six catalogue names retained; re-check before publish |
| Dependencies | Minimal and feature-gated; no `unsafe`; std where practical |
| `mixbus` | DSP core only; caller-owned audio thread; no VST3/CLAP hosting (BP-036 stays deferred) |
| `ffscene` | Compile, validate, ladder, encoder probe, output descriptors; schema stays in the app |
| `replay-recorder` | Packet-in, MKV default, segment-rotation ring |
| `studio-protocol` | Messages + pure state machine; JSON; no transport in 0.1 |
| `rtmp-publish` | RTMP/RTMPS/SRT/WHIP; shallow probe; no deep handshake; no key rotation |
| `signedlink` | HMAC-SHA256; mandatory store; optional `axum` redirect |
| Security baseline | Expand all REQUIRED controls (CON-511, 512, 514, 515, 516, 519, 520) |
| Publication | Release-ready now; publish from `main` after review and archive |

### Sources

- `.sdrive/product/backlog.md` (BP-022 to BP-027), `.sdrive/product/roadmap.md` (§M4)
- `catalogue/32_ffscene.md`, `33_mixbus.md`, `34_replay_recorder.md`, `35_studio_protocol.md`, `36_rtmp_publish.md`, `37_signedlink.md`
- `.sdrive/constitution.md` (CON-1xx to CON-7xx), `.sdrive/context/code-standards.md`, `.sdrive/context/architecture-context.md`
- FFmpeg filter documentation: <https://ffmpeg.org/ffmpeg-filters.html>
- ITU-R BS.1770-4 (loudness and true peak): <https://www.itu.int/rec/R-REC-BS.1770>
- EBU R 128 (loudness normalisation): <https://tech.ebu.ch/publications/r128>
- RFC 2104 (HMAC): <https://www.rfc-editor.org/rfc/rfc2104>
- RFC 8446 (TLS 1.3): <https://www.rfc-editor.org/rfc/rfc8446>
- RFC 9725 (WHIP): <https://www.rfc-editor.org/rfc/rfc9725>
- W3C Referrer Policy: <https://www.w3.org/TR/referrer-policy/>
- OWASP ASVS: <https://owasp.org/www-project-application-security-verification-standard/>
- Rust API Guidelines: <https://rust-lang.github.io/api-guidelines/checklist.html>
- Cargo features: <https://doc.rust-lang.org/cargo/reference/features.html>

No Context7 or `exa` research was available in this session; external references above are stable standards documents, and every project-specific claim is sourced from the repository.

### Codebase Context

The repository contains `crates/dado` only. There is no existing scene compiler, audio graph, recorder, studio protocol, destination model, or link signer to reuse. This specification is therefore based on the catalogue, the constitution, and user-confirmed decisions; no existing module, endpoint, or pattern is assumed beyond the M0 workspace baseline and CI.

## User Stories

### US-001: Compile a scene into a deterministic FFmpeg graph

As a live-studio developer, I want to compile a declarative scene into an FFmpeg filter graph and argument vector, so that the same scene always produces the same pipeline and invalid scenes fail before FFmpeg starts.

**Acceptance Criteria:**

- AC-001: Given a fixed `Scene`, when `compile` is called twice in the same process or in separate processes, then the argument vector, `filter_complex` string, and label mapping are byte-identical.
- AC-002: Given a scene with a cycle, an unknown source, an out-of-range transform, or an unsupported blend mode, when `compile` is called, then it returns a `CompileError` naming the offending layer and produces no graph.
- AC-003: Given a 1080p scene and a ladder containing 720p and 1080p rungs, when the ladder is expanded, then the graph contains ordered `split` and `scale` nodes, one output per rung, and each rung compiles successfully.
- AC-004: Given a requested encoder profile, when the encoder probe finds `h264_nvenc`, `hevc_qsv`, `h264_amf`, or only `libx264`, then the result names the selected encoder and the fallback reason explicitly, and never downgrades silently.
- AC-005: Given `Output::File`, `Output::Rtmp`, `Output::Srt`, or `Output::Whip`, when the scene compiles, then it returns a descriptor only and performs no socket, DNS, or process action.
- AC-006: Given a 4K/60 scene with overlapping layer names and many transforms, when compiled, then it produces no numeric overflow and no duplicate filter labels.

### US-002: Mix live audio without breaking real-time guarantees

As a live-audio developer, I want a mixing graph that processes buffers in place with gain, pan, gating, compression, ducking, and loudness, so that I can ship broadcast-quality audio without allocating or locking on the audio thread.

**Acceptance Criteria:**

- AC-007: Given a block of silence, when `process` runs, then the output is silence with no DC offset and no drift across repeated blocks.
- AC-008: Given a known input and a gain of `-6 dB` with constant-power pan, when processed, then the output matches the reference gain and pan law within `0.1 dB`.
- AC-009: Given a ducker keyed by a speech source, when the key crosses the threshold, then the music source is attenuated within the configured attack window and released within the release window.
- AC-010: Given a noisy speech signal, when the gate runs, then it does not chatter: gain transitions are bounded to the configured hysteresis and hold time.
- AC-011: Given an integrated loudness target, when a program-length signal is processed, then integrated loudness is within `±0.5 LU` of target and true peak does not exceed the configured ceiling.
- AC-012: Given a routing change, when the new plan is installed, then the swap happens at a block boundary with no click or dropout, and a custom allocator that aborts on allocation observes no allocation, lock, log, or `await` on the audio thread.
- AC-013: Given metering is enabled, when blocks are processed, then peak, RMS, LUFS, and gain reduction are published per block through a lock-free snapshot and are never read from inside `process`.

### US-003: Record isolated tracks and keep a replay ring

As a studio operator, I want to record isolated tracks and keep the last N seconds on disk, so that I can extract a clip after an event without stopping the recording or filling the volume.

**Acceptance Criteria:**

- AC-014: Given a process killed mid-segment, when the recorder restarts and scans the spool, then every complete segment is recovered, the partial segment is discarded or finalised deterministically, and `index.json` is consistent.
- AC-015: Given a ring configured for 120 seconds, when `clip(last: 30s)` is requested, then it returns a decodable clip within one frame of 30 seconds and recording continues uninterrupted.
- AC-016: Given `max_bytes` and `min_free_bytes`, when the budget is reached, then the configured pressure policy applies, an event is emitted, and free space never falls below `min_free_bytes`.
- AC-017: Given a tee sink with two outputs, when a segment closes, then both outputs are written and the duplicate is byte-identical to the local segment.
- AC-018: Given 16 enabled tracks under sustained packet load, when recording runs, then no track drops packets and each track's index lists its own segments only.
- AC-019: Given a wall-clock change or non-monotonic timestamps, when segments close, then segment durations and the index remain monotonic and uncorrupted.

### US-004: Enforce studio control permissions server-side

As a studio service developer, I want a pure control-plane state machine with explicit permissions, so that preview/program changes are authorised on the server and replaying a message log reproduces the same state.

**Acceptance Criteria:**

- AC-020: Given every message variant, when serialised to JSON and deserialised, then the variant, IDs, timestamps, and payload round-trip unchanged.
- AC-021: Given the matrix of roles and message variants, when each message is applied, then the result is allowed or denied exactly as the permission table declares, and a denied message leaves state unchanged.
- AC-022: Given a program cut, when it is applied, then the transition is atomic: no observable state has both the outgoing and incoming scene on program.
- AC-023: Given duplicate or out-of-order messages, when they are applied, then duplicates are rejected idempotently and out-of-order messages are rejected or buffered deterministically as documented.
- AC-024: Given a recorded message log, when it is replayed from a known initial state, then the final `StudioState` equals the final state of the live run.
- AC-025: Given an envelope with an unknown major version, when it is applied, then it is rejected; given a newer minor version with additional fields, then it is accepted.
- AC-026: Given one participant sending 50 messages per second, when the service applies them, then it sustains the rate without unbounded queue growth, and excess messages are rejected with a typed error.

### US-005: Model egress destinations without leaking stream keys

As a streaming developer, I want to model RTMP, RTMPS, SRT, and WHIP destinations with validated shape, health, retry, and failover, so that stream keys never reach logs and failures are handled honestly.

**Acceptance Criteria:**

- AC-027: Given any destination and any key value, when `Debug`, `Display`, error payloads, and tracing events are produced, then the key never appears (property test over generated keys and formats).
- AC-028: Given a probe against a resolvable host that refuses, a host that times out, a nonexistent DNS name, and an endpoint that rejects credentials, then `Health` distinguishes `Unreachable`, `Unreachable` with timeout, DNS failure, and `AuthRejected`.
- AC-029: Given repeated failures, when the retry policy runs, then backoff is bounded, jittered, and capped, and the per-destination circuit breaker opens and later closes.
- AC-030: Given a primary with an open breaker and a backup destination, when failover occurs, then the backup is promoted, no flapping occurs before the cool-down, and the primary is restored only after the cool-down.
- AC-031: Given observed throughput, when a bitrate hint is produced, then the hint never exceeds the configured maximum or drops below the configured floor.
- AC-032: Given an invalid scheme, a missing host, a bad port, or an empty key, when the destination is validated, then a typed error is returned before any connection attempt.

### US-006: Issue signed, expiring, attributable links

As an affiliate or checkout developer, I want short signed links with UTM parameters and an attribution window, so that I can prove a click was ours and reject tampered, expired, or reused links.

**Acceptance Criteria:**

- AC-033: Given a signed URL, when the target, campaign, or tenant is modified, then verification fails with a typed error and returns no claims.
- AC-034: Given an expired link or a single-use link whose nonce is consumed, when it is verified, then it fails closed and no claims are returned.
- AC-035: Given a `clock_skew` window, when the current time is outside the window, then verification fails; when it is inside, then verification succeeds.
- AC-036: Given UTM parameters, when they are normalised, then `utm_source`, `utm_medium`, `utm_campaign`, `utm_term`, and `utm_content` are sorted deterministically, and any unknown key is rejected rather than dropped.
- AC-037: Given a code collision, when the store reports the code exists, then generation retries up to a bounded limit and then returns a typed error instead of looping.
- AC-038: Given the optional redirect handler, when it redirects, then the response sets a no-referrer policy so the signature is not leaked to the target.
- AC-039: Given a revoked code, when it is verified after revocation, then it fails immediately without waiting for expiry.

### US-007: Keep the six crates consistent and release-ready

As a workspace maintainer, I want all six crates to meet the repository baseline, so that they can be reviewed as one unit and published from `main`.

**Acceptance Criteria:**

- AC-040: Given each crate, when built with `--no-default-features` and with each feature enabled independently, then it compiles, and enabling one feature does not disable another.
- AC-041: Given each crate, when compiled on Rust 1.85, then it builds, declares `forbid(unsafe_code)` and `deny(missing_docs)`, and documents every public item.
- AC-042: Given each crate, when its example is built, then at least one runnable example compiles and demonstrates the crate's primary operation without network or device access.
- AC-043: Given the workspace, when `cargo package --workspace` and `cargo doc --workspace --all-features` run with warnings denied, then they succeed and every crate carries description, licence, repository, readme, keywords, categories, and docs.rs all-features metadata.
- AC-044: Given the workspace, when CI runs and the dependency tree is inspected, then the new crates are covered by the existing workspace jobs and none of them depends on `wirekit` or exposes application domain types.

## Requirements

### Functional Requirements

- FR-001: `ffscene::compile(&Scene, &Output)` MUST return a `CompiledGraph` with `filter_complex`, input labels, output labels, and per-layer mapping, and MUST be deterministic for identical input.
- FR-002: `ffscene` MUST validate cycles, missing sources, out-of-range transforms, and unsupported blend modes before producing a graph, returning a typed `CompileError` that names the layer.
- FR-003: `ffscene` MUST expand a `Ladder` into one encoder profile per rung with correct `split` and `scale` nodes and stable ordering.
- FR-004: `ffscene` MUST probe for `h264_nvenc`, `hevc_qsv`, `h264_amf`, and `libx264` and MUST report the selected encoder plus any fallback reason explicitly.
- FR-005: `ffscene` MUST emit `Output::File`, `Output::Rtmp`, `Output::Srt`, and `Output::Whip` descriptors without opening sockets, resolving DNS, or spawning processes.
- FR-006: `ffscene` MUST assign deterministic layer and filter labels with a stable node order and MUST NOT embed timestamps or process-specific values in the graph.
- FR-007: `mixbus::Mixer::process(&mut self, input, output)` MUST process interleaved `f32` frames in place with no allocation, lock, log, or `await` on the audio thread.
- FR-008: `mixbus` MUST provide gain, constant-power pan, gate with hysteresis, compressor, sidechain ducker, and EBU R128 loudness with true-peak limiting.
- FR-009: `mixbus` MUST build routing changes off the audio thread and swap the new plan at a block boundary.
- FR-010: `mixbus` MUST publish peak, RMS, LUFS, and gain reduction per block through a lock-free snapshot.
- FR-011: `mixbus` inserts MUST report their latency and the graph MUST compensate so summed outputs stay aligned.
- FR-012: `mixbus` MUST accept caller-owned interleaved `f32` buffers and MUST NOT open devices, decode files, own a thread, or depend on a runtime.
- FR-013: `replay-recorder` MUST accept already-encoded packets per track, allow start and stop per track, and preserve monotonic per-track timestamps across clock changes.
- FR-014: `replay-recorder` MUST support `Sink::Segment`, `Sink::Tee`, and `Sink::Ring` with isolated per-track output.
- FR-015: `replay-recorder` MUST expose `clip(last: Duration) -> ClipHandle` from the ring without stopping recording.
- FR-016: `replay-recorder` MUST enforce `max_bytes` and `min_free_bytes` and apply `PressurePolicy::{StopOldest, StopNewest, Refuse}` with an emitted event.
- FR-017: `replay-recorder` MUST recover deterministically from a crash by scanning the spool and finalising or discarding partial segments.
- FR-018: `replay-recorder` MUST write `spool/<session-id>/track-<id>/seg-<n>.<ext>` plus `index.json`, updating the index after each segment close.
- FR-019: `replay-recorder` MUST default to MKV and MUST offer MP4 per track.
- FR-020: `studio-protocol` MUST define the `Envelope`, the `Preview`, `Program`, `ScenePatch`, `Overlay`, `Role`, `Whisper`, `Teleprompter`, `Ping`, and `Pong` messages, and the shared IDs.
- FR-021: `studio-protocol` MUST map roles to permission sets and MUST check `Permission` server-side for every message.
- FR-022: `studio-protocol` MUST implement transitions as pure functions of `(state, message, actor)` returning a new state or a typed rejection.
- FR-023: `studio-protocol` MUST serialise with JSON and MUST use a single integer protocol version where minor additions are backward compatible and unknown majors are rejected.
- FR-024: `studio-protocol` MUST reject duplicate message IDs idempotently and MUST define deterministic handling for out-of-order messages.
- FR-025: `studio-protocol` MUST bound per-participant message handling at 50 messages per second and MUST reject excess messages with a typed error instead of growing a queue without limit.
- FR-026: `rtmp-publish` MUST model `Destination` with protocol, URL, secret key, optional region, and optional backup, and MUST redact the key in `Debug`, `Display`, tracing, and error payloads while NOT implementing `Serialize` for the key.
- FR-027: `rtmp-publish` MUST validate scheme, host, port, and key presence before any connection attempt and MUST return a typed error otherwise.
- FR-028: `rtmp-publish` MUST probe RTMP/RTMPS/SRT with a TCP connect and WHIP with an HTTP `OPTIONS` or `HEAD` request, distinguishing DNS failure, refused, timeout, and auth rejection.
- FR-029: `rtmp-publish` MUST use bounded exponential backoff with jitter, retry transport failures, never retry `AuthRejected`, and maintain a per-destination circuit breaker.
- FR-030: `rtmp-publish` MUST promote a backup destination when the primary breaker opens and MUST demote it only after a cool-down.
- FR-031: `rtmp-publish` MUST produce bitrate hints bounded by configured ceilings and floors.
- FR-032: `rtmp-publish` MUST NOT implement RTMP, SRT, or WHIP wire protocols, muxing, encoding, or CDN orchestration.
- FR-033: `signedlink` MUST sign with HMAC-SHA256 and append a compact URL-safe token that `verify` checks at a supplied instant, rejecting tampering, expiry, and clock skew outside the window.
- FR-034: `signedlink` MUST generate base62 short codes, collision-check them through the store, and bound retries.
- FR-035: `signedlink` MUST normalise the five UTM keys deterministically and MUST reject unknown UTM keys.
- FR-036: `signedlink` claims MUST carry the attribution window and MUST NOT invent a default.
- FR-037: `signedlink` MUST support optional single use by consuming a nonce atomically through the store.
- FR-038: `signedlink` MUST require a `LinkStore` implementation for revocation and single use; stateless verification alone MUST NOT be presented as revocable.
- FR-039: `signedlink` MUST provide the redirect handler behind an optional `axum` feature and MUST set a no-referrer policy on the redirect response.
- FR-040: The six crates MUST be workspace members under `crates/*` and MUST NOT require a root manifest edit.
- FR-041: Every crate MUST declare `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`.
- FR-042: Every crate MUST ship at least one runnable example that needs no network, device, or external process.
- FR-043: Every crate MUST carry description, licence, repository, readme, keywords, categories, and docs.rs all-features metadata, and `cargo package --workspace` MUST succeed.
- FR-044: No M4 crate MUST depend on `wirekit`, expose provider modules, or add a global mutable singleton.
- FR-045: Each crate MUST have exactly one job and MUST NOT expose application domain types, product nouns, UI, billing rules, or vendor workflow code.

### Non-Functional Requirements

- NFR-001: `ffscene` output MUST be byte-identical for identical input, independent of hash-map iteration order, wall-clock time, and process identity.
- NFR-002: `mixbus` MUST never allocate, lock, log, or `await` on the audio thread; buffers MUST be pre-allocated at the maximum block size.
- NFR-003: All crates MUST treat input as untrusted and MUST return typed errors with stable codes instead of panicking, aborting, or hanging.
- NFR-004: Stream keys, signing keys, and signed tokens MUST NOT appear in `Debug`, `Display`, logs, traces, error payloads, or `Serialize` output (CON-214).
- NFR-005: `replay-recorder` MUST NOT fill the volume: `min_free_bytes` and `max_bytes` MUST be enforced before and during writes.
- NFR-006: `studio-protocol` MUST sustain 50 messages per second per participant within its documented latency budget without unbounded memory growth.
- NFR-007: Every crate MUST build on Rust 1.85 with edition 2021, no `unsafe`, and complete rustdoc (CON-102, CON-201, CON-202).
- NFR-008: Optional functionality MUST be behind additive Cargo features; defaults MUST be documented and no feature may disable another (CON-204).
- NFR-009: Registration and configuration MUST be explicit; no crate may expose a global mutable singleton (CON-109).
- NFR-010: Public APIs MUST follow the Rust API Guidelines and MUST NOT ship UI, framework-coupled presentation, or application domain types (CON-106, CON-216, CON-301).
- NFR-011: **Authentication (CON-511)** — Signing keys and stream keys MUST be caller-supplied, MUST NOT be persisted by any crate, and MUST be validated for presence and shape before use. `signedlink` MUST fail closed on any unverifiable link, and `rtmp-publish` MUST treat a missing key as an invalid destination.
- NFR-012: **Authorization (CON-512)** — `studio-protocol` MUST enforce every permission check server-side for every message, and `signedlink` revocation and single use MUST be enforced by the store rather than by client behaviour.
- NFR-013: **Rate limiting (CON-514)** — `rtmp-publish` MUST bound retries and circuit-breaker transitions per destination, and `studio-protocol` MUST enforce the per-participant message budget with explicit rejection (CON-209 applies to HTTP only and is not extended to these crates).
- NFR-014: **Input validation (CON-515)** — Every public entry point accepting a URL, key, scene, packet, message, or link MUST validate it and return a typed error; no input may cause a panic or unbounded allocation.
- NFR-015: **Injection protection (CON-516)** — `replay-recorder` MUST derive spool paths only from validated IDs and MUST reject path traversal; the crates ship no SQL or Redis backend, and the `LinkStore` contract documents that implementations MUST use parameterised queries or equivalent.
- NFR-016: **Environment security (CON-519)** — Credentials and signing keys MUST come from caller-supplied configuration or secret stores; the feature MUST NOT add `.env` files, committed keys, or tokens.
- NFR-017: **HTTPS / TLS (CON-520)** — `rtmp-publish` health probes and redirects MUST use TLS-capable schemes (`https`, `rtmps`, `whips`), MUST reject plaintext credential transport, and MUST document that HSTS is the deployment edge's responsibility. `signedlink`'s redirect handler MUST NOT downgrade the target scheme.
- NFR-018: Every acceptance criterion MUST be covered by at least one test supplied by the Test agent (CON-404).

## Security Baseline

The feature touches user input, credentials, authorisation decisions, and HTTP/TCP surfaces. Each REQUIRED control from CON-51x is expanded here and carried into `plan.json` and `tasks.json`.

| Control | Decision | Requirement | Concrete obligation |
|---|---|---|---|
| CON-511 Authentication | REQUIRED | NFR-011 | Caller-supplied signing/stream keys; fail closed on unverifiable links; reject empty keys before connect. |
| CON-512 Authorization | REQUIRED | NFR-012 | Server-side permission checks in `studio-protocol`; store-enforced revocation and single use in `signedlink`. |
| CON-513 Security Headers | NOT APPLICABLE | — | No crate owns an HTTP response pipeline. The optional `axum` redirect handler sets only `Referrer-Policy: no-referrer` and `Cache-Control: no-store`; it does not impose a global header policy. |
| CON-514 Rate Limiting | REQUIRED | NFR-013 | Bounded retry/breaker per destination; 50 msg/s/participant budget with typed rejection. |
| CON-515 Input Validation | REQUIRED | NFR-014 | Typed validation at every public entry point; no panics. |
| CON-516 Injection Protection | REQUIRED | NFR-015 | Validated spool paths; no SQL/Redis backend; parameterised-query obligation in the `LinkStore` contract. |
| CON-517 XSS Protection | NOT APPLICABLE | — | No crate renders HTML or user-facing markup. The redirect handler emits a status, `Location`, and headers only. |
| CON-518 CSRF Protection | NOT APPLICABLE | — | No crate owns a browser session or cookie-based state; signed links are capabilities and the redirect handler does not accept state-changing requests. |
| CON-519 Environment Security | REQUIRED | NFR-016 | Caller-supplied secrets; no `.env`; no committed keys. |
| CON-520 HTTPS / HSTS | REQUIRED | NFR-017 | TLS-capable schemes only for probes and redirect targets; HSTS documented as an edge obligation. |

<!-- source: .sdrive/constitution.md §Security Baseline (CON-51x); catalogue/36_rtmp_publish.md; catalogue/37_signedlink.md; catalogue/35_studio_protocol.md -->

## UI/UX & Data Requirements

- **UI/UX:** None. This repository ships libraries only (CON-301). The optional `axum` redirect handler is an HTTP response, not UI. Disabled-button and other presentation obligations remain the consuming application's responsibility (CON-302).
- **Data:** The crates own in-memory scene, mix, recording, protocol, destination, and link-claims types. Durable data is limited to the `replay-recorder` spool (`seg-*.mkv|mp4`, `index.json`) and whatever a consumer implements behind `LinkStore`. No crate owns a database, migrations, or a persistence schema.
- **IDs:** `LayerId`, `SourceId`, `BusId`, `TrackId`, `DestinationId`, `MessageId`, `ParticipantId`, and `TenantId` are caller-provided or generated newtypes; the crates do not prescribe a storage format beyond `Display`/`FromStr`.

## Edge Cases & Error States

- EC-001: A layer references itself or a cycle of layers → `CompileError::Cycle` naming the cycle members; no graph.
- EC-002: A layer references an unknown `SourceId` → `CompileError::UnknownSource`; no graph.
- EC-003: Transform values overflow `u32` or exceed canvas bounds at 4K/60 → `CompileError::TransformOutOfRange`; no wrap-around.
- EC-004: Two layers normalise to the same label → deterministic suffixing; no duplicate labels in the graph.
- EC-005: No FFmpeg binary or no hardware encoder is present → explicit `EncoderSupport::Unsupported` with the probed list; never a silent switch.
- EC-006: `mixbus` receives a block larger than the pre-allocated maximum → typed error; no reallocation.
- EC-007: `mixbus` receives NaN or infinite samples → output is sanitised to finite values and a meter flag is set.
- EC-008: Loudness measurement of digital silence → LUFS reports the gated silence value without violating the true-peak ceiling.
- EC-009: Crash during `index.json` write → the last segment is discarded on recovery; the index is rewritten atomically.
- EC-010: Disk full during a segment write → the pressure policy fires, the partial segment is removed, and an event is emitted.
- EC-011: Wall clock moves backwards during recording → per-track timestamps remain monotonic; no negative durations.
- EC-012: Duplicate `MessageId` → rejected idempotently; state unchanged.
- EC-013: Unknown protocol major version → rejected with `UnsupportedVersion`; newer minor fields are accepted.
- EC-014: Participant lacks the required permission → `Denied`; state unchanged and the attempt is observable.
- EC-015: Destination URL uses an unsupported scheme or a plaintext `http://` WHIP endpoint → rejected before connecting (CON-520).
- EC-016: Destination key is missing, empty, or whitespace → `InvalidKey`; no connection attempt.
- EC-017: Probe hits DNS failure, refused connection, or timeout → distinct `Health` variants, never a generic failure.
- EC-018: Backup destination is the same URL/key as the primary, or two backups point at each other → failover refuses to loop.
- EC-019: Link expires exactly at `now == exp` → expired; the boundary is closed.
- EC-020: Single-use link is replayed → `AlreadyUsed`; no claims.
- EC-021: Unknown UTM key in a link → `UnknownUtmKey`; nothing is dropped silently.
- EC-022: Code collision persists past the retry limit → `CodeExhausted`; no infinite loop.
- EC-023: Redirect target receives the request → the signature is not present in `Referer` because the response sets `Referrer-Policy: no-referrer`.

## Out of Scope

- Publishing any crate to crates.io from this feature branch.
- `mixbus` VST3/CLAP plugin hosting (BP-036) and any `unsafe` exception.
- A published scene JSON schema, drag-and-drop layout editor, or incremental subgraph recompilation in `ffscene`.
- Owned encoder selection, hardware encoding, or sparse-file ring storage in `replay-recorder`.
- An `axum`/WebSocket transport, binary encoding, chat, or persistence in `studio-protocol`.
- Deep RTMP handshake probing, key rotation, CDN orchestration, or protocol implementation in `rtmp-publish`.
- Ed25519 signatures, analytics rollups, QR generation, or an anonymous public URL shortener in `signedlink`.
- `platform-core`, `quota-ledger`, `hookgate`, `syncforge`, `wirekit`, and provider modules (M1-M3 features).
- Application UI, product nouns, billing rules, and vendor workflow code (CON-106, CON-301).
- Test authoring and test execution (owned by `/sdrive:test`).

## Dependencies & Risks

### Dependencies

| Dependency | Type | Purpose |
|---|---|---|
| `serde` (optional) | library | JSON/index/message serialisation where a crate opts in |
| `serde_json` (optional) | library | `replay-recorder` index, `studio-protocol` messages, `signedlink` claims |
| `uuid` v7 (optional) | library | `studio-protocol` message and participant IDs |
| `chrono` (optional) | library | `studio-protocol` timestamps and `signedlink` time source |
| `url` | library | `rtmp-publish` destination and `signedlink` target parsing |
| `hmac`, `sha2`, `base64` | library | `signedlink` HMAC-SHA256 tokens |
| `getrandom` | library | `signedlink` base62 code entropy |
| `reqwest` (optional, rustls) | library | `rtmp-publish` WHIP health probe; `signedlink` optional redirect support |
| `axum` (optional) | library | `signedlink` redirect handler |
| crates.io | service | Publication target after merge; name re-check required |

No dependency may require `unsafe` in a public API surface, break MSRV 1.85, or be enabled by default unless the crate's core operation needs it.

### Risks

- RISK-001: Six crates in one feature create a large review unit. Mitigation: phase the plan per crate, keep each crate independent, and make each phase produce a compiling workspace.
- RISK-002: Real-time audio guarantees are hard to prove. Mitigation: an aborting allocator test, injected clock, and explicit no-lock/no-log rules in `NFR-002` and `AC-012`.
- RISK-003: FFmpeg filter syntax changes across versions. Mitigation: target FFmpeg 7.x, golden-file tests, and a documented dialect boundary in `ffscene`.
- RISK-004: A stream key leaks through a derived type or a tracing field. Mitigation: redaction by construction, no `Serialize` for the key, and a property test over key shapes and formats.
- RISK-005: Disk-budget enforcement races with concurrent writers. Mitigation: enforce before and during writes, single-writer contract per track, atomic index writes.
- RISK-006: Permission checks drift from the message table. Mitigation: one permission table in code, a matrix test generated from it, and pure transitions.
- RISK-007: Dependency additions raise MSRV or pull a runtime. Mitigation: optional, feature-gated dependencies; MSRV job in the existing CI.
- RISK-008: `signedlink` is mistaken for stateless revocable links. Mitigation: the store is mandatory for revocation and single use, and the docs state the boundary.
- RISK-009: Code duplication across crates (IDs, errors, redaction). Mitigation: keep each crate single-purpose and accept small, documented duplication rather than introducing a shared framework crate (CON-103).
- RISK-010: Scope creep into M5 providers or `platform-core`. Mitigation: enforce the out-of-scope list; later work becomes a new feature or version.
