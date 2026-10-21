# Execution Tasks - m4-media-links v1

> **Feature:** `m4-media-links`
> **Version:** v1
> **Status:** backlog
> **Plan:** `.sdrive/specs/backlog/m4-media-links/v1/plan.md`
> **Branch:** `feature/m4-media-links`

Production-code tasks only. Test authoring and test execution belong to `/sdrive:test` (CON-404, No Test Tasks Rule).

## P1 - ffscene

| ID | Task | Requirements | Acceptance criteria |
|---|---|---|---|
| T1.1 | Scaffold `crates/ffscene` and define the core scene types | FR-001, NFR-003 | AC-001 |
| T1.2 | Implement scene validation before compilation | FR-002, NFR-003, NFR-014 | AC-002 |
| T1.3 | Implement deterministic graph and argument compilation | FR-001, FR-006, NFR-001 | AC-001, AC-006 |
| T1.4 | Implement encoder ladder expansion | FR-003, FR-006 | AC-003 |
| T1.5 | Implement the encoder probe and explicit downgrade report | FR-004 | AC-004 |
| T1.6 | Implement output descriptors without transport | FR-005 | AC-005 |
| T1.7 | Add the ffscene README, changelog, and compile example | FR-042 | AC-042 |

### T1.1 — Scaffold `crates/ffscene` and define the core scene types

Create `Cargo.toml` with workspace inheritance, `#![forbid(unsafe_code)]`, `#![deny(missing_docs)]`, and a `//!` crate doc that states scope and non-goals. Define `Canvas`, `Rational`, `Background`, `Scene`, `Layer`, `LayerId`, `SourceId`, `Transform`, `BlendMode`, `EncoderProfile`, `Codec`, `RateControl`, `Ladder`, `Rung`, `Output`, `CompiledGraph`, and `CompileError` with stable error codes. Keep the crate dependency-free.

### T1.2 — Implement scene validation before compilation

Add a validation pass that walks `Scene.layers` and rejects cycles, unknown `SourceId`s, transforms outside the canvas bounds, non-finite opacity, and unsupported blend modes. Return `CompileError` variants that carry the layer ID and a stable code, and never produce a partial graph.

### T1.3 — Implement deterministic graph and argument compilation

Build the filter graph from a normalised layer order `(z, layer id, source id)`. Emit scale, crop, overlay, blend, format, and setpts nodes with stable labels, then produce `filter_complex`, the input/output label vectors, the argument vector, and the `LayerLabelMap`. Emit no timestamps or process-specific values. Use ordered collections only.

### T1.4 — Implement encoder ladder expansion

Expand a `Ladder` into one `EncoderProfile` per rung, inserting `split` followed by ordered `scale` nodes. Keep rung order stable and produce one output label per rung. Reject empty ladders and duplicate rung names with a typed error.

### T1.5 — Implement the encoder probe and explicit downgrade report

Accept an injected list of available encoders (so no FFmpeg binary is required at build time) and select the requested encoder or report the fallback explicitly. Return a result that names the selected encoder, the requested encoder, and the reason for any downgrade. Never fall back silently.

### T1.6 — Implement output descriptors without transport

Define `Output::{File, Rtmp, Srt, Whip}` as data only. Compilation must not resolve DNS, open sockets, or spawn processes. Validate the descriptor shape (path presence, URL scheme, stream key presence) and keep connection health in `rtmp-publish`.

### T1.7 — Add the ffscene README, changelog, and compile example

Add `README.md` describing scope, non-goals, the FFmpeg 7.x dialect boundary, determinism guarantees, and a usage sketch. Add `CHANGELOG.md`. Add `examples/compile_scene.rs` that builds an in-memory scene, compiles it, and prints the argument vector with no network or process access.

## P2 - mixbus

| ID | Task | Requirements | Acceptance criteria |
|---|---|---|---|
| T2.1 | Scaffold `crates/mixbus` with pre-allocated buffers and typed errors | FR-007, FR-012, NFR-002 | AC-007 |
| T2.2 | Implement gain and constant-power pan | FR-008 | AC-008 |
| T2.3 | Implement the gate and compressor | FR-008 | AC-010 |
| T2.4 | Implement the sidechain ducker | FR-008 | AC-009 |
| T2.5 | Implement EBU R128 loudness and true-peak limiting | FR-008 | AC-011 |
| T2.6 | Implement off-thread plan rebuild, block-boundary swap, and latency compensation | FR-009, FR-011, NFR-002 | AC-012 |
| T2.7 | Implement lock-free meter snapshots | FR-010 | AC-013 |
| T2.8 | Add the mixbus README, changelog, and process example | FR-042 | AC-042 |

### T2.1 — Scaffold `crates/mixbus` with pre-allocated buffers and typed errors

Create the crate manifest, lints, crate docs, `Mixer`, `MixerConfig`, `Bus`, `Source`, `Frame`, `BlockSize`, and `MixError`. Pre-allocate all buffers at the maximum block size during construction. Define a bounded command queue for control changes. The crate has no required dependencies.

### T2.2 — Implement gain and constant-power pan

Implement per-source and per-bus gain in dB with a linear conversion, and constant-power stereo panning. Keep the math branch-light and free of allocation. Sanitise NaN and infinite input to finite values and raise a meter flag.

### T2.3 — Implement the gate and compressor

Implement a noise gate with threshold, hysteresis, attack, hold, and release, plus a compressor with threshold, ratio, attack, release, and makeup gain. Keep per-sample state in the pre-allocated plan; do not allocate or log.

### T2.4 — Implement the sidechain ducker

Implement `Duck` with a key source, threshold, ratio, attack, and release. Read only the key source's detector value from the current block, apply gain reduction to the target source, and avoid pumping by using the configured attack and release.

### T2.5 — Implement EBU R128 loudness and true-peak limiting

Implement K-weighting, gated integrated loudness, and a true-peak limiter with a configurable ceiling. Report LUFS and true peak through the meter snapshot. Handle digital silence without violating the ceiling.

### T2.6 — Implement off-thread plan rebuild, block-boundary swap, and latency compensation

Build a new `MixPlan` on the control thread when routing changes, then swap it at the next block boundary through an atomic pointer exchange. Sum insert latency and delay parallel paths so outputs remain aligned. The audio thread must only read the active plan and drain the bounded queue.

### T2.7 — Implement lock-free meter snapshots

Publish peak, RMS, LUFS, gain reduction, and flags per block into a double-buffered snapshot with an atomic index. Readers load a snapshot without locking and never call into the processing path. Include per-bus and per-source meters.

### T2.8 — Add the mixbus README, changelog, and process example

Document the real-time rules, the buffer contract, latency, and the meter model. Add `CHANGELOG.md` and `examples/process_block.rs` that synthesises a block, mixes two sources with ducking, and prints meters. No device access.

## P3 - replay-recorder

| ID | Task | Requirements | Acceptance criteria |
|---|---|---|---|
| T3.1 | Scaffold `crates/replay-recorder` with track types and the packet API | FR-013, FR-019 | AC-018 |
| T3.2 | Implement the spool layout, ID path safety, and atomic index | FR-018, NFR-015 | AC-014 |
| T3.3 | Implement segment writing and monotonic timestamps | FR-013, FR-014, FR-019 | AC-015, AC-018, AC-019 |
| T3.4 | Implement the tee sink | FR-014 | AC-017 |
| T3.5 | Implement ring rotation and clip extraction | FR-014, FR-015 | AC-015 |
| T3.6 | Implement the disk budget and pressure policies | FR-016, NFR-005 | AC-016 |
| T3.7 | Implement crash recovery | FR-017 | AC-014 |
| T3.8 | Add the replay-recorder README, changelog, and record-and-clip example | FR-042 | AC-042 |

### T3.1 — Scaffold `crates/replay-recorder` with track types and the packet API

Create the manifest, lints, crate docs, `Recorder`, `RecorderConfig`, `SessionId`, `TrackId`, `Track`, `TrackCodec`, `Container`, `Packet`, `Sink`, `DiskBudget`, `PressurePolicy`, `ClipHandle`, and `RecorderError`. Define the packet-in API and per-track start/stop.

### T3.2 — Implement the spool layout, ID path safety, and atomic index

Write `spool/<session-id>/track-<id>/seg-<n>.<ext>` and `index.json`. Derive every path from validated newtypes; reject traversal, separators, reserved names, and absolute paths. Write the index to a temporary file and rename it after each segment close.

### T3.3 — Implement segment writing and monotonic timestamps

Open, write, and close segments by duration or size. Track per-track sequence numbers so a wall-clock change cannot produce negative or non-monotonic durations. Default to MKV and allow MP4 per track. Record codec, container, start, duration, and byte length in the index.

### T3.4 — Implement the tee sink

Duplicate each encoded segment to the configured outputs. Write the local segment first, then the duplicate, and record both paths in the index. Propagate write failures as typed errors without leaving an index entry for a missing file.

### T3.5 — Implement ring rotation and clip extraction

Keep a bounded set of recent segments per ring track. `clip(last)` selects the covering range, concatenates the segment payloads into a self-contained file, and returns a `ClipHandle` without pausing the writer. Bound the window by both seconds and bytes.

### T3.6 — Implement the disk budget and pressure policies

Check `max_bytes` and `min_free_bytes` before opening a segment and periodically during writes. Apply `StopOldest`, `StopNewest`, or `Refuse`, emit a typed `PressureEvent`, delete partial files, and never let free space fall below `min_free_bytes`.

### T3.7 — Implement crash recovery

Scan the spool on construction, validate the index against the files, finalise a segment that has a complete header and trailer, and discard a partial segment deterministically. Rewrite the index atomically and expose a recovery report.

### T3.8 — Add the replay-recorder README, changelog, and record-and-clip example

Document the spool layout, the single-writer-per-track contract, the budget semantics, and recovery. Add `CHANGELOG.md` and `examples/record_and_clip.rs` that records synthetic packets into a temporary spool, extracts a clip, and prints the paths.

## P4 - studio-protocol

| ID | Task | Requirements | Acceptance criteria |
|---|---|---|---|
| T4.1 | Scaffold `crates/studio-protocol` with the envelope, IDs, messages, and JSON serde | FR-020, FR-023 | AC-020 |
| T4.2 | Implement permissions and the role table | FR-021, NFR-012 | AC-021 |
| T4.3 | Implement the pure state machine and atomic program cut | FR-022 | AC-022, AC-024 |
| T4.4 | Implement version negotiation | FR-023 | AC-025 |
| T4.5 | Implement duplicate and out-of-order handling | FR-024 | AC-023, AC-024 |
| T4.6 | Implement the per-participant message budget | FR-025, NFR-006, NFR-013 | AC-026 |
| T4.7 | Add the studio-protocol README, changelog, and message-log example | FR-042 | AC-042 |

### T4.1 — Scaffold `crates/studio-protocol` with the envelope, IDs, messages, and JSON serde

Create the manifest with optional `serde`, `serde_json`, `uuid` (`v7`), and `chrono` dependencies. Define `Envelope`, `MessageId`, `ParticipantId`, `TenantId`, `Message`, the payload structs, and the JSON encoding. Use `deny_unknown_fields` on payloads and keep the wire shape documented.

### T4.2 — Implement permissions and the role table

Define `Permission`, `Role`, and a single role-to-permission table. Expose `check(role, message) -> Result<(), Rejected>` and call it before every transition. Document that the check is server-side and that a client button state is never enforcement.

### T4.3 — Implement the pure state machine and atomic program cut

Define `StudioState` with program, preview, participants, overlays, and seen IDs. Implement `apply(state, envelope, actor) -> Result<StudioState, Rejected>` as a pure function. The program cut swaps program and preview in one assignment so no observable state has both.

### T4.4 — Implement version negotiation

Encode the protocol version as one integer where the major is `v / 100` and the minor is `v % 100` (documented in the crate). Reject an unknown major and accept a higher minor with additive fields. Cover the `Envelope` and every payload.

### T4.5 — Implement duplicate and out-of-order handling

Reject a duplicate `MessageId` idempotently with `Rejected::Duplicate`. Reject scene and overlay messages older than the last applied sequence for that actor, except `Ping`/`Pong`, and document the rule. Keep the seen-ID set bounded.

### T4.6 — Implement the per-participant message budget

Implement a fixed-window counter per participant with the 50 messages-per-second budget. Return `Rejected::RateLimited` when exhausted. Do not grow a queue; the caller decides whether to drop or retry.

### T4.7 — Add the studio-protocol README, changelog, and message-log example

Document the message table, the permission matrix, the version rule, and the ordering rule. Add `CHANGELOG.md` and `examples/replay_log.rs` that applies a short JSON message log and prints the final state.

## P5 - rtmp-publish

| ID | Task | Requirements | Acceptance criteria |
|---|---|---|---|
| T5.1 | Scaffold `crates/rtmp-publish` with destination, secret, and redacted types | FR-026, FR-032, NFR-003, NFR-004, NFR-016 | AC-027 |
| T5.2 | Implement destination shape validation with the TLS policy | FR-027, NFR-011, NFR-014, NFR-017 | AC-032 |
| T5.3 | Implement TCP and WHIP probes with health classification | FR-028, FR-032, NFR-017 | AC-028 |
| T5.4 | Implement bounded retry with jitter and the circuit breaker | FR-029, NFR-013 | AC-029 |
| T5.5 | Implement failover and cool-down | FR-030 | AC-030 |
| T5.6 | Implement bounded bitrate hints | FR-031 | AC-031 |
| T5.7 | Add the rtmp-publish README, changelog, and destination example | FR-042 | AC-042 |

### T5.1 — Scaffold `crates/rtmp-publish` with destination, secret, and redacted types

Create the manifest, lints, crate docs, `DestinationId`, `Destination`, `Protocol`, `SecretString`, `Health`, `RetryPolicy`, `CircuitBreaker`, `BitrateHint`, and `PublishError`. Implement `Debug`/`Display` by hand so the key prints `***`; do not implement `Serialize` for the key. Provide `RedactedDestination` as the only serialisable view.

### T5.2 — Implement destination shape validation with the TLS policy

Validate protocol, scheme, host, port, and key presence. Accept `rtmp`, `rtmps`, `srt`, `srt://`, `whip+https`, and `https` for WHIP; reject plaintext credential schemes. Return a typed `PublishError` before any connection attempt. Document HSTS as an edge responsibility.

### T5.3 — Implement TCP and WHIP probes with health classification

Probe RTMP/RTMPS/SRT with a TCP connect and WHIP with an HTTP `OPTIONS` or `HEAD` request behind the optional `reqwest` feature with `rustls-tls`. Classify DNS failure, refused, timeout, TLS failure, and auth rejection into distinct `Health` variants. Accept an injected connector and timeout for deterministic behaviour.

### T5.4 — Implement bounded retry with jitter and the circuit breaker

Implement `base * 2^attempt` with a cap and injected jitter. Retry transport failures only; never retry `AuthRejected`. Track failures per destination, open the breaker, and close it after a success or a cool-down.

### T5.5 — Implement failover and cool-down

Promote the backup when the primary breaker opens and demote it only after the cool-down. Refuse self-referential backups and backup cycles. Keep promotion decisions in one state machine so the result does not flap.

### T5.6 — Implement bounded bitrate hints

Given observed throughput, produce a hint between the configured floor and ceiling. Never exceed the ceiling or drop below the floor. Keep the calculation pure and documented.

### T5.7 — Add the rtmp-publish README, changelog, and destination example

Document redaction guarantees, the supported protocols, probe depth, retry semantics, and failover. Add `CHANGELOG.md` and `examples/plan_destination.rs` that validates a destination, prints the redacted view, and computes a health plan without connecting.

## P6 - signedlink

| ID | Task | Requirements | Acceptance criteria |
|---|---|---|---|
| T6.1 | Scaffold `crates/signedlink` with claims, signer, code, store, and typed errors | FR-033, FR-038, NFR-003, NFR-004, NFR-011, NFR-015 | AC-033 |
| T6.2 | Implement HMAC-SHA256 signing, constant-time verification, expiry, and clock skew | FR-033 | AC-033, AC-035 |
| T6.3 | Implement base62 code generation with bounded collision retry | FR-034 | AC-037 |
| T6.4 | Implement UTM normalisation and the attribution window | FR-035, FR-036, NFR-014 | AC-036 |
| T6.5 | Implement single-use nonce consumption and revocation | FR-037, FR-038, NFR-012 | AC-034, AC-039 |
| T6.6 | Implement the optional axum redirect handler with no-referrer | FR-039, NFR-004, NFR-017 | AC-038 |
| T6.7 | Add the signedlink README, changelog, and sign-and-verify example | FR-042 | AC-042 |

### T6.1 — Scaffold `crates/signedlink` with claims, signer, code, store, and typed errors

Create the manifest with `hmac`, `sha2`, `base64`, `getrandom`, `url`, optional `serde`, and optional `axum`. Define `Signer`, `SigningKey`, `Claims`, `Code`, `SignedUrl`, `LinkStore`, `LinkError`, and the canonical claim encoding. Redact the key and token in `Debug`.

### T6.2 — Implement HMAC-SHA256 signing, constant-time verification, expiry, and clock skew

Canonicalise claims with sorted UTM parameters and a format version, sign with HMAC-SHA256, and encode URL-safe without padding. Verify with a constant-time comparison, reject a modified payload or MAC, enforce expiry with a closed boundary, and accept only the configured clock skew.

### T6.3 — Implement base62 code generation with bounded collision retry

Generate base62 codes from OS entropy through `getrandom`. Check existence through `LinkStore::get`, retry up to a documented limit, and return `CodeExhausted` after the limit. Document that the code is an identifier and the signature is the authorisation.

### T6.4 — Implement UTM normalisation and the attribution window

Accept exactly `utm_source`, `utm_medium`, `utm_campaign`, `utm_term`, and `utm_content`. Sort deterministically, normalise casing consistently, and reject unknown keys. Carry the attribution window in claims without inventing a default.

### T6.5 — Implement single-use nonce consumption and revocation

Add `LinkStore::consume(code) -> bool` for atomic single use and `LinkStore::revoke(code)`. Reject consumed, revoked, and missing codes with distinct errors. Document the parameterised-query obligation for store implementations.

### T6.6 — Implement the optional axum redirect handler with no-referrer

Behind the `axum` feature, add a handler that resolves the code, verifies the link, records the click hook, and redirects. Set `Referrer-Policy: no-referrer` and `Cache-Control: no-store`. Do not downgrade the target scheme and do not leak the token.

### T6.7 — Add the signedlink README, changelog, and sign-and-verify example

Document the token format, the store requirement, revocation semantics, the UTM contract, and the redirect headers. Add `CHANGELOG.md` and `examples/sign_and_verify.rs` with an in-memory store showing sign, verify, tamper rejection, and revocation.

## P7 - Workspace baseline and hardening

| ID | Task | Requirements | Acceptance criteria |
|---|---|---|---|
| T7.1 | Add metadata, READMEs, changelogs, docs.rs configuration, and packaging settings to all six crates | FR-040, FR-043, FR-045, NFR-009, NFR-010 | AC-040, AC-043, AC-044 |
| T7.2 | Verify every example builds without network or device access | FR-042 | AC-042 |
| T7.3 | Verify the feature matrix with `cargo check` | NFR-008 | AC-040 |
| T7.4 | Run the formatter, clippy with `-D warnings`, and rustdoc with warnings denied | FR-041, NFR-007, NFR-010 | AC-041 |
| T7.5 | Perform the manual security baseline review | NFR-016 | AC-027, AC-032 |
| T7.6 | Verify CI coverage and the dependency rules | FR-044, NFR-009 | AC-044 |

### T7.1 — Add metadata, READMEs, changelogs, docs.rs configuration, and packaging settings to all six crates

For each crate, add description, keywords, categories, readme, licence, repository, include list, and `[package.metadata.docs.rs] all-features = true`. Add a README and CHANGELOG. Confirm `cargo package --workspace` succeeds. Do not edit the root manifest: `crates/*` already covers the crates.

### T7.2 — Verify every example builds without network or device access

Build each `examples/*.rs` target and confirm it compiles with no network, device, FFmpeg binary, or external process. This is a manual/compile check; it does not run tests.

### T7.3 — Verify the feature matrix with `cargo check`

Run `cargo check --workspace --no-default-features` and then each crate with each feature enabled independently. Fix any accidental coupling and document the default feature set.

### T7.4 — Run the formatter, clippy with `-D warnings`, and rustdoc with warnings denied

Run `cargo fmt --check`, `cargo clippy --workspace --all-features -- -D warnings`, and `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --all-features`. Fix the findings. Do not invoke a test runner in this task.

### T7.5 — Perform the manual security baseline review

Read every public entry point and confirm: keys are redacted, no `Serialize` for secrets, TLS-only schemes, no `.env` or committed key, path IDs are validated, and the `LinkStore` contract documents parameterised access. Record the review in the pull request description.

### T7.6 — Verify CI coverage and the dependency rules

Confirm `.github/workflows/ci.yml` runs the workspace jobs over the new crates without a workflow edit. Check the dependency tree: no `wirekit`, no global mutable singleton, and one job per crate.
