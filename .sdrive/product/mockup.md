# Conceptual Product Map

> **Status:** Approved product plan
> **Date:** 2026-10-20
> **Scope:** Developer experience, not application UI.
> **Important:** This is a conceptual, low-fidelity map. It contains no HTML, CSS, JavaScript, interactive prototype, production UI, or image mockup.

## Why this is not a screen mockup

GIA Packages is a library ecosystem. Its "interface" is the crate API, the README, docs.rs pages, examples, and error messages. The equivalent of screens is the developer's path from discovery to production.

This document maps:

1. the crate families a developer encounters;
2. the entry points that lead to them;
3. the onboarding journey;
4. the documentation navigation;
5. the failure states a developer sees.

## Product map

```
                         +----------------------+
                         |   GIA Packages       |
                         | one dependency,      |
                         | one model            |
                         +----------+-----------+
                                    |
        +---------------------------+---------------------------+
        |                           |                           |
  +-----v------+             +------v------+             +------v------+
  |  HTTP      |             |  Stateful   |             |  Media      |
  |  clients   |             |  primitives |             |  and links  |
  +-----+------+             +------+------+             +------+------+
        |                           |                           |
   wirekit                    platform-core                ffscene
   (26 providers)             quota-ledger                 mixbus
                              hookgate                     replay-recorder
                              syncforge                    studio-protocol
                                                           rtmp-publish
                                                           signedlink
        |                           |                           |
        +---------------------------+---------------------------+
                                    |
                              dado-yaml (foundation)
                       strict YAML, diagnostics, graph,
                       emitter, glob - no I/O
```

### Reading the map

- **`dado-yaml`** is the base layer. Most developers will not install it directly; `wirekit` and generators use it.
- **`wirekit`** is the front door. It carries the provider modules and the shared HTTP model.
- **Stateful primitives** are opt-in. A single-binary tool can use `wirekit` without a database.
- **Media and links** are independent of the HTTP client. A studio can use `ffscene` with no network.

## Entry points

| Entry point | What the developer sees | Primary action |
|---|---|---|
| crates.io | Crate name, description, downloads, repository link | Open docs |
| docs.rs | API docs, feature flags, examples | Copy a quickstart |
| GitHub README | Promise, crate table, provider matrix, status | `cargo add` |
| Examples folder | One runnable example per provider or crate | Run locally |
| Error reference | Stable codes and recovery steps | Diagnose a failure |
| Changelog | Breaking changes and migrations | Upgrade safely |

## README text wireframe

```
+------------------------------------------------------------------+
|  GIA Packages                                                    |
|  One Rust dependency for every external API.                     |
|  One error type. One retry policy. One auth layer.               |
|                                                                  |
|  [docs.rs]  [GitHub]  [crates.io]  [Examples]  [Changelog]       |
+------------------------------------------------------------------+
|  Quickstart                                                      |
|                                                                  |
|  cargo add wirekit --features twilio,sendgrid                    |
|                                                                  |
|  let client = wirekit::Client::builder()                         |
|      .timeout(Duration::from_secs(15))                           |
|      .build()?;                                                  |
|                                                                  |
|  let message = client.twilio().messages().send(req).await?;      |
+------------------------------------------------------------------+
|  Crates                                                          |
|  wirekit          Typed HTTP clients, 26 provider modules        |
|  syncforge        Offline-first action queue                     |
|  quota-ledger     Per-tenant, per-provider quota                 |
|  hookgate         Inbound webhook edge                           |
|  platform-core    Provider traits and registry                   |
|  ffscene          Scene to FFmpeg filter graph                   |
|  mixbus           Real-time audio graph                          |
|  replay-recorder  Tracks and replay ring                         |
|  studio-protocol  Preview/program control                        |
|  rtmp-publish     Egress destinations and health                 |
|  signedlink       Signed links and attribution                   |
|  dado-yaml             Strict YAML and config tooling                 |
+------------------------------------------------------------------+
|  Provider matrix                                                 |
|  Provider   Feature      Auth             Status                  |
|  NOAA       noaa         none             first publish           |
|  Twilio     twilio       account SID      first publish           |
|  SendGrid   sendgrid     API key          first publish           |
|  Slack      slack        OAuth / bot      first publish           |
|  Meta       meta         app review       critical path           |
|  ...        ...          ...              ...                     |
+------------------------------------------------------------------+
|  Status: 0.x. APIs may change. 1.0 freezes after external usage. |
+------------------------------------------------------------------+
```

## Developer journey

### Journey A: "Add Twilio SMS to a Rust service"

| Step | Developer action | What the product provides | Failure state |
|---|---|---|---|
| 1. Discover | Finds `wirekit` on crates.io or through a search | Clear promise, crate table, provider matrix | README does not explain the feature-flag model |
| 2. Install | `cargo add wirekit --features twilio` | Copy-pasteable command | Feature name typo produces a cryptic Cargo error |
| 3. Configure | Builds a client with timeout and credentials | Builder with typed config, no global singleton | Missing credential returns `InvalidConfig`, not a panic |
| 4. Call | Sends a typed request | Provider module with typed request/response | HTTP 200 with a provider error body maps to `Api`, not success |
| 5. Fail | Matches on `WirekitError` | One enum: `Api`, `Auth`, `RateLimit`, `Timeout`, `Network`, `InvalidConfig` | A provider-specific error leaks through |
| 6. Observe | Enables `tracing` | Spans for request, retry, rate limit, error | Logs contain a token or phone number |
| 7. Operate | Adds quota or webhook handling if needed | `quota-ledger` or `hookgate` composes without a framework | Forced database or runtime |
| 8. Upgrade | Reads the changelog | Deprecation notes and migration guide | Silent breaking change in a patch release |

### Journey B: "Add offline job completion to a field app"

| Step | Developer action | What the product provides |
|---|---|---|
| 1. Model | Defines an action type | Shared `Action`, `ActionId`, `Batch`, `BatchResult` types |
| 2. Persist | Writes before the UI says "saved" | Durable queue; SQLite by default |
| 3. Replay | Reconnects | Idempotency key checked before execution |
| 4. Conflict | Server has a newer version | Declared strategy: `LastWriteWins`, `ServerWins`, `ClientWins`, `Custom` |
| 5. Audit | Needs the original time | Client timestamp preserved with server receipt time |
| 6. Test | Wants deterministic replay | In-memory storage and mock transport |

### Journey C: "Add webhook handling without losing events"

| Step | Developer action | What the product provides |
|---|---|---|
| 1. Receive | Gets a provider webhook | `InboundRequest` keeps raw bytes |
| 2. Verify | Checks the signature | `Verifier` trait; raw body is never re-serialized |
| 3. Deduplicate | Sees the same delivery twice | Delivery ID and idempotency key |
| 4. Replay-protect | Sees an old timestamp | Replay window |
| 5. Forward | Hands off to a queue | Outbox with retries and dead-letter |
| 6. Isolate | Serves multiple tenants | A delivery cannot be read by another tenant |

## Documentation navigation

```
README
  |
  +-- Quickstart
  |     |
  |     +-- Install
  |     +-- First call
  |     +-- Error handling
  |
  +-- Crates
  |     |
  |     +-- wirekit
  |     |     +-- Core model
  |     |     +-- Provider matrix
  |     |     +-- Feature flags
  |     |     +-- Twilio / SendGrid / NOAA / Slack ...
  |     |
  |     +-- syncforge
  |     +-- quota-ledger
  |     +-- hookgate
  |     +-- platform-core
  |     +-- media and link crates
  |     +-- dado-yaml
  |
  +-- Examples
  |     |
  |     +-- One runnable example per provider
  |     +-- One end-to-end webhook example
  |     +-- One offline sync example
  |
  +-- Reference
  |     |
  |     +-- Error codes
  |     +-- Feature flags
  |     +-- Provider access requirements
  |
  +-- Operations
  |     |
  |     +-- Quota
  |     +-- Webhooks
  |     +-- Observability
  |
  +-- Upgrading
        |
        +-- Changelog
        +-- 0.x to 1.0 migration
```

## Provider matrix (conceptual)

The matrix is the main discovery surface for `wirekit`. It should answer four questions per provider:

| Column | Question | Example |
|---|---|---|
| Provider | Who is it? | Twilio |
| Feature | What do I enable? | `twilio` |
| Auth | What access do I need? | Account SID + auth token |
| Status | Can I use it today? | first publish / critical path / backlog / needs discovery |

Status labels must be honest. A provider with no public API is marked `deferred`, not `coming soon`.

## Failure states as "screens"

Because there is no UI, the failure states are error messages and docs pages:

| State | What the developer sees | Required behavior |
|---|---|---|
| Feature not enabled | Cargo or compile-time message | Docs explain the exact feature name |
| Missing credential | `WirekitError::InvalidConfig` | No panic, no secret in the message |
| Auth rejected | `WirekitError::Auth` | Actionable: refresh, re-consent, or check scopes |
| Rate limited | `WirekitError::RateLimit { retry_after }` | Retry hint, no busy loop |
| Timeout | `WirekitError::Timeout` | Distinguish from network failure |
| Provider error in 200 | `WirekitError::Api { status, code, message }` | Never treat 200 as success |
| Webhook signature invalid | `VerifyError` | Fail closed, no partial processing |
| Secret in log | Test failure | Property test catches it before release |

## What this mockup is not

- Not a visual design.
- Not a component library.
- Not a working prototype.
- Not a commitment to a docs-site framework.
- Not a promise that every provider will reach `first publish`.

## Open questions

1. Should the README lead with the provider matrix or the unified model?
2. Is a docs site needed for 0.1, or is docs.rs plus README enough?
3. Should examples live in each crate's `examples/` folder or in a top-level `examples/` workspace member?
4. Should the provider matrix be generated from Cargo features to prevent drift?
5. What is the public brand name shown in the README header?
6. Should error codes be stable public identifiers, and if so, what numbering scheme?
