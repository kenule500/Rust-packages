# Product Overview

> **Status:** Approved product plan
> **Date:** 2026-10-20
> **Product:** GIA Packages (public Rust package ecosystem)
> **Umbrella brand:** `[to be confirmed]`
> **Source of truth for rules:** `.sdrive/constitution.md` v1.0.0

## How to read this document

| Label | Meaning |
|---|---|
| **Confirmed** | The user explicitly decided this in the product-plan interview. |
| **Recommendation** | The agent proposes this; it is not binding until approved. |
| **Assumption** | Needed to plan, but not verified. Revisit before it becomes a commitment. |
| **Open question** | Unresolved; recorded rather than guessed. |

Dates and time horizons are **tentative** unless a later approval makes them commitments.

## Vision

**One Rust dependency for every external API: one error type, one retry policy, one auth layer, one observability story.**

A Rust team should be able to add Twilio, SendGrid, NOAA, and Slack without learning four different client crates, four error models, and four retry policies. The same team should be able to add offline sync, quota, or webhook handling without rebuilding those primitives per application.

## Problem

Rust teams integrating many external APIs face the same recurring costs:

1. **Fragmented clients.** Each provider crate has its own API shape, error type, retry behavior, and auth flow.
2. **No shared observability.** Tracing, metrics, and retry visibility are rebuilt per integration.
3. **State rebuilt per app.** Offline queues, idempotency, quota ledgers, webhook deduplication, and token stores are reimplemented slightly wrong each time.
4. **Missing clients.** Several providers named in the catalogue have no maintained Rust client.
5. **Permanent API burden.** Publishing app-specific code creates a public API and support obligation for something no external developer can reuse.

The repository already states the thesis: "Every GIA service that talks to Twilio or Stripe uses the same client, the same error type, and the same retry policy." The product plan turns that thesis into a publishable ecosystem.

Source: `catalogue/README.md` sections "Why these are separate from the app" and "Public crates.io, not app-only".

## Target users

### Primary

**External Rust developers building multi-tenant SaaS integrations.** They want a typed client, a predictable error model, and a documented path from install to production.

### Secondary

- **GIA and OmniStream engineering**, the first production consumers.
- **Open-source contributors** who add provider modules, docs, or tests.

### Not the target

- Teams looking for a hosted integration platform or BaaS.
- Teams looking for a full application framework.
- Users who need a UI component library.

## Jobs to be done

| # | Job | Success looks like |
|---|---|---|
| JTBD-1 | Add a provider in minutes | `cargo add wirekit --features twilio` then a typed call compiles |
| JTBD-2 | Handle failures once | One `WirekitError` match handles every provider |
| JTBD-3 | Observe calls | Tracing spans show request, retry, rate limit, and error |
| JTBD-4 | Add stateful primitives | Quota, webhook, token, and sync crates compose without a framework |
| JTBD-5 | Trust the security posture | Secrets are redacted, signatures are verified on raw bytes, tenants are isolated |
| JTBD-6 | Upgrade safely | A 0.x changelog and a 1.0 migration guide exist |

## Product scope

### In scope

The 11 public crates:

| Crate | Job |
|---|---|
| `dado` | Strict YAML, diagnostics, dependency graph, code emitter, glob |
| `wirekit` | Typed HTTP clients, with 26 provider modules behind feature flags |
| `syncforge` | Offline-first action queue, idempotency, conflict handling |
| `quota-ledger` | Per-tenant, per-provider quota reservation and ledger |
| `hookgate` | Inbound webhook verification, dedupe, replay protection, outbox |
| `platform-core` | Generic provider traits, registry, token-store trait |
| `ffscene` | Scene graph to FFmpeg filter graph, encoder ladder, GPU probe |
| `mixbus` | Real-time audio mixing graph |
| `replay-recorder` | Multi-track capture, replay ring, segment/tee muxing |
| `studio-protocol` | Preview/program control protocol and state machine |
| `rtmp-publish` | Egress destination model, key redaction, health, failover |
| `signedlink` | Signed short links, UTM builder, expiry, attribution |

`wirekit` carries the 26 provider modules. The other crates are standalone because they own state, media, or a different failure mode.

### First public release

`dado` 0.1.0, then `wirekit` 0.1.0 with four providers:

- **NOAA** (no auth, fastest proof)
- **Twilio** (SMS, voice, MFA, lookup)
- **SendGrid** (transactional email)
- **Slack** (notifications)

### v0.1 program vs first publish

**Confirmed:** all 11 crates are in the v0.1 product scope. They are **sequenced**, not published simultaneously. The first publish is deliberately narrow so the ecosystem has a credible, tested starting point.

### Out of scope

- Application UI, view code, or framework-coupled presentation.
- Application domain types, billing rules, and product workflows.
- A hosted sync relay, BaaS, CRDT, or full framework.
- A live Angi client (the provider has no public API).
- 1.0 API stability during the 0.x series.
- Shipping all 26 providers at once.

## Goals (next 12 months)

| Goal | Measure |
|---|---|
| G1 | Publish `dado` 0.1.0 |
| G2 | Publish `wirekit` 0.1.0 with Twilio, SendGrid, NOAA, Slack |
| G3 | Publish `wirekit` 0.2.x with Meta, EagleView, QuickBooks, Stripe, and the Google OAuth helper |
| G4 | Publish all 11 crates at 0.x |
| G5 | Three external projects use `wirekit` in production |
| G6 | GIA uses `wirekit` in production |
| G7 | Freeze public APIs at 1.0 after external usage |
| G8 | One runnable example per provider, a provider matrix, and a 0.x to 1.0 migration guide |

## Non-goals

- Becoming a hosted integration platform.
- Becoming a web framework or application runtime.
- Owning a tenant's business logic, pricing, or UI.
- Publishing code that only one application can use.
- Promising semver stability before 1.0.
- Pretending a provider integration exists when the provider has no public API.

## Constraints

| Constraint | Source |
|---|---|
| Rust edition 2021, MSRV 1.85 | `.sdrive/constitution.md` CON-102; `catalogue/Cargo.toml` |
| `MIT OR Apache-2.0` | CON-102; `catalogue/Cargo.toml` |
| Single Cargo workspace at `catalogue/`, members `crates/*` | CON-101 |
| One job per package; no framework | CON-103 |
| Providers are `wirekit` modules, not crates | CON-104 |
| No application domain types in public crates | CON-106 |
| Stateful concerns live in their own crates | CON-107 |
| crates.io names are permanent | CON-111 |
| No CI, tests, `rustfmt.toml`, or `clippy.toml` exist today | `.sdrive/context/project-overview.md` |
| `Cargo.lock` is ignored and untracked | `catalogue/.gitignore` |
| One team member (`kenule500`, admin) | `.sdrive/team.json` |
| Provider access approvals are external | `catalogue/00_provider_index.md` |
| No UI or Figma data | `.sdrive/context/ui-context.md` |

## Confirmed decisions

These were explicitly chosen in the product-plan interview:

1. The product is the **public Rust package ecosystem**, not the GIA application.
2. The 6-12 month outcome is to **publish `dado` + `wirekit` on the critical path**.
3. The primary users are **external Rust developers**.
4. The existing catalogue docs are **reference material**; this plan is written fresh.
5. The product promise is **"one dependency, one model"**.
6. **All 11 crates** are in the v0.1 product scope, sequenced.
7. The **first publish providers** are Twilio, SendGrid, NOAA, and Slack.
8. The stability model is **0.x, freeze at 1.0 later**.
9. Success is **adoption + production use**: three external projects and GIA in production.
10. **Turborepo is not adopted now**; it is a deferred candidate (BP-037).

## Recommendations

These are proposals, not commitments:

- Build the quality baseline in M0 before shipping: CI, tests, release checklist.
- Publish `dado` before `wirekit`; `wirekit` will consume it.
- Keep the first publish to four providers; add the critical path in 0.2.
- Run M2 (providers) and M3 (substrate) in parallel; they share only `wirekit` core.
- Add a companion crate if `wirekit` feature flags become a release bottleneck.
- Recruit design partners before the 1.0 freeze.
- Treat the 1.0 freeze as an audit, not a date.

## Assumptions

- [to be confirmed] The umbrella public brand name; "GIA Packages" is the repository name, not necessarily the public product name.
- [to be confirmed] Provider access (Meta, EagleView, QuickBooks, Google LSA) can be obtained within the roadmap window.
- [to be confirmed] External developers will accept a 0.x API in exchange for early access.
- [to be confirmed] MSRV 1.85 remains acceptable for the first year.
- [to be confirmed] The candidate dependencies in this document will be approved crate by crate.
- [to be confirmed] No JavaScript/TypeScript surface is planned; if one appears, re-evaluate Turborepo.
- [to be confirmed] A single maintainer is sufficient until external contributors join.

## Existing repository facts

| Fact | Detail |
|---|---|
| Workspace | `catalogue/Cargo.toml`, members `crates/*` |
| Implemented crates | `dado` 0.1.0 only |
| `dado` state | `diag`, `graph`, `emit`, `glob` implemented; `yaml` returns `D0001` |
| Documentation | 39 Markdown files in `catalogue/` |
| Missing referenced doc | `21_Packages_To_Build.md` is cited but absent |
| Feature specs | None; `.sdrive/specs/` is empty |
| Product plan | This document set |
| Default branch | `main` |
| Remote | `https://github.com/kenule500/Rust-packages.git` |
| CI | None |
| Tests | None |

## Architecture and package boundaries

```
catalogue/
  Cargo.toml                  # workspace, members = crates/*
  crates/
    dado/                     # foundation, no I/O
    wirekit/                  # HTTP clients, provider modules (planned)
    platform-core/            # traits and registry (planned)
    syncforge/                # offline queue (planned)
    quota-ledger/             # shared quota state (planned)
    hookgate/                 # webhook edge (planned)
    ffscene/                  # video composition (planned)
    mixbus/                   # audio graph (planned)
    replay-recorder/          # capture and replay (planned)
    studio-protocol/          # control protocol (planned)
    rtmp-publish/             # egress model (planned)
    signedlink/               # signed links (planned)
```

Dependency direction:

```
consuming app / adapter crate
        |
        +--> platform-core   (traits + registry; no product nouns)
        +--> wirekit         (HTTP clients; no app domain types)
        +--> syncforge       (queue + transport traits)
        +--> quota-ledger    (state; optional Postgres/Redis)
        +--> hookgate        (state; optional axum)
        +--> media crates    (ffscene, mixbus, replay-recorder, studio-protocol, rtmp-publish)
        +--> signedlink
```

Rules that constrain the plan:

- `dado` never opens a socket, reads a URL, spawns a runtime, or writes a file.
- `wirekit` owns HTTP, not quota or webhook state.
- `platform-core` ships no product vocabulary or OAuth implementation.
- `mixbus` processes buffers; it does not open audio devices.
- `ffscene` compiles a graph; it does not encode.
- `rtmp-publish` redacts keys and never serializes them.
- `signedlink` treats the signature, not the short code, as the authorization.

Source: `.sdrive/context/architecture-context.md`.

## Candidate tools and dependencies

| Package | Use | Status | Source |
|---|---|---|---|
| `serde` | Serialization | **Confirmed** in `dado` | `crates/dado/Cargo.toml` |
| `winnow` | YAML parsing | **Confirmed** in `dado` | `crates/dado/Cargo.toml` |
| `indexmap` | Ordered maps | **Confirmed** in `dado` | `crates/dado/Cargo.toml` |
| `reqwest` | HTTP client | Proposed | `catalogue/01_wirekit.md` |
| `tokio` | Async runtime | Proposed | Catalogue async examples |
| `tracing` | Observability | Proposed | `catalogue/01_wirekit.md` |
| `thiserror` | Error derive | Proposed | Agent recommendation |
| `uuid` v7 | Idempotency and message IDs | Proposed | `catalogue/02_syncforge.md`, `catalogue/35_studio_protocol.md` |
| `chrono` or `time` | Timestamps | Proposed | `catalogue/29_quota.md` |
| `sqlx` | Postgres backend | Proposed | `catalogue/31_platform_core.md`, `catalogue/29_quota.md` |
| `axum` | Optional server integrations | Proposed | `catalogue/30_webhook_gateway.md`, `catalogue/37_signedlink.md` |
| `hmac` + `sha2` | Webhook and link signatures | Proposed | `catalogue/30_webhook_gateway.md`, `catalogue/37_signedlink.md` |
| `ed25519-dalek` | Link signatures | Proposed | `catalogue/37_signedlink.md` |
| `secrecy` + `zeroize` | Secret redaction | Proposed | `catalogue/36_rtmp_publish.md` |
| `proptest` | Property tests | Proposed | `catalogue/32_ffscene.md`, `catalogue/36_rtmp_publish.md` |
| `wiremock` or `httpmock` | Provider test doubles | Proposed | `catalogue/01_wirekit.md` test plan |
| `turbo` | Task orchestration | **Deferred** (BP-037) | Turborepo 2.11 experimental Cargo support |

No candidate becomes a dependency without the normal review and approval path. The catalogue docs name some of these tools; naming them is not approval.

## Success measures

| Dimension | Measure |
|---|---|
| Adoption | Three external projects use `wirekit` in production within 12 months |
| Internal use | GIA runs `wirekit` in production |
| Releases | `dado` 0.1, `wirekit` 0.1 (4 providers), `wirekit` 0.2 (critical path), all 11 crates at 0.x |
| Stability | 1.0 API freeze after external usage, with a migration guide |
| Quality | CI green on fmt, clippy, test, docs; every acceptance criterion has a test; no secret leaks |
| Documentation | One runnable example per provider; provider matrix; error reference; feature-flag table |
| Community | At least one external contributor or design partner per quarter after first publish |

## Risks

| Risk | Impact | Mitigation |
|---|---|---|
| Provider approvals (Meta, EagleView, QuickBooks, Google LSA) | Delays M2 and M5 | Start applications early; keep the first publish to low-friction providers |
| 26 provider modules in one crate | Feature-flag sprawl, slow builds | Measure build times; use a `wirekit-streaming` companion if needed |
| No CI or tests today | Shipping unverified clients | M0 builds fmt, clippy, test, docs, and MSRV |
| `mixbus` plugin hosting needs `unsafe` | Conflicts with CON-201 | BP-036; amendment path already exists |
| 0.x API churn | Adoption friction | Changelog, deprecation policy, migration guide, 1.0 freeze |
| Single maintainer | Bus factor | Contributor docs, design partners, good-first-issues |
| 11 crates is broad | Thin coverage | Sequence strictly; do not start M4 before M3 exit criteria |
| Community crates compete | Users pick alternatives | Compete on the unified model and docs, not on provider count |
| Provider terms of service | Legal exposure | Review each provider's terms before publishing its module |
| Secret leakage | Security incident | CON-214, property tests, redaction wrappers, secret scan in CI |
| Unverified performance claims | Credibility loss | Benchmark before claiming; publish numbers with hardware and setup |
| crates.io name collisions | Forced renames | Check immediately before publish (CON-111) |

## Inspiration and references

### Visual and product references

No Figma file, screenshot, Dribbble link, or design system was provided. There is no application UI in this repository. The conceptual mockup is a developer-experience map, not a screen design.

### Developer-experience references (proposed, not confirmed)

These are high-level patterns the plan borrows from, not copied designs or APIs:

| Reference | Pattern worth adapting |
|---|---|
| `serde` | Additive feature flags, thorough crate docs, clear derive model |
| `reqwest` | A client builder with timeouts, pools, and middleware |
| `sqlx` | Traits for storage plus optional database backends |
| `tracing` | Structured spans and events as a first-class concern |
| `stripe-rust` | Typed request/response objects per resource |
| `aws-sdk-rust` | Modular feature flags for a large provider surface |

The user did not name these references. They are recorded as **proposed inspiration**, not confirmed guidance. Replace or remove them if the user prefers different models.

Source for the product facts: `catalogue/README.md`, `catalogue/00_provider_index.md`, and `catalogue/01_wirekit.md` through `catalogue/37_signedlink.md`.

## Open questions

1. What is the public umbrella brand name, if it is not "GIA Packages"?
2. Where is `21_Packages_To_Build.md`? It is cited as the technical contract but is not in the repository.
3. Should `wirekit::stripe` be built, or should the plan recommend a community crate and remove Stripe from examples?
4. Is Angi permanently out of scope as a live client, or is an import helper still wanted?
5. Can Google LSA Ads API access be obtained, and under what account structure?
6. Are ADP and Sage Intacct partner/enterprise access paths realistic in the next 12 months?
7. Does `mixbus` VST3/CLAP hosting require an `unsafe` exception, or can it stay out of the first release?
8. Which Redis client and minimum Redis version should `quota-ledger` support?
9. GitHub Actions or another CI provider for M0?
10. What release automation (release-plz, cargo-release, manual) should be used?
11. Is `platform-core` a dependency of `wirekit`, or an independent trait crate that both the app and `wirekit` opt into?
12. Should `syncforge` ship before, with, or after `quota-ledger` and `hookgate`?
13. What is the target date for the 1.0 freeze, and what adoption evidence is required?
14. Will the project require a CLA, DCO, or neither for external contributions?
15. What criteria would make Turborepo worth adopting later (CI time, crate count, JS/TS surface)?

## Handoff

- This document is product context, not a feature specification.
- Backlog items live in `.sdrive/product/backlog.md`.
- To turn one approved item into a versioned specification, plan, traceability matrix, and tasks, run:

```
/sdrive:propose "BP-001 finish dado YAML parser and publish dado 0.1.0"
```

- `/sdrive:propose` owns branching and creates the feature branch. This product plan does not create `.sdrive/specs/backlog/<feature>/`, governance JSON, or workflow-state entries.
