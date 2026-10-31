# Roadmap

> **Status:** Approved product plan
> **Date:** 2026-10-20
> **Horizons are tentative.** No date or estimate in this document is a commitment unless a later approval makes it one.

## Roadmap principles

1. **Publish something real early.** `dado-yaml` and `wirekit` with four providers beats a broad, unverified surface.
2. **Sequence by dependency, not enthusiasm.** `dado-yaml` before `wirekit`; `wirekit` core before providers; substrate before media integrations that need shared IDs.
3. **Keep stateful concerns out of the HTTP client.** `quota-ledger` and `hookgate` ship as separate crates.
4. **Do not start a milestone before its exit criteria are met.** Thin coverage across 11 crates is worse than two solid crates.
5. **Treat 1.0 as an audit.** The freeze happens after external usage, not on a calendar date.

## Milestone overview

| Milestone | Tentative window | Outcome | Depends on | Exit criteria |
|---|---|---|---|---|
| **M0 - Foundation** | Weeks 0-2 | `dado-yaml` 0.1.0 published; CI and contributor baseline | - | `dado-yaml` on crates.io; CI green; release runbook; CONTRIBUTING and SECURITY |
| **M1 - First publish** | Weeks 3-8 | `wirekit` core + NOAA, Twilio, SendGrid, Slack; 0.1.0 published | M0 | `wirekit` 0.1.0 on crates.io; four features; examples; tests |
| **M2 - Critical path** | Weeks 9-16 | Meta, EagleView, QuickBooks, Stripe, Google OAuth; 0.2.x published | M1 + provider access | Eight providers + OAuth helper; sandbox tests; access confirmed |
| **M3 - Integration substrate** | Weeks 12-24 | `platform-core`, `quota-ledger`, `hookgate`, `syncforge` 0.1 | M1; parallel with M2 | Four crates published; one integration example |
| **M4 - Media and links** | Weeks 20-36 | Six media/link crates 0.1 | M3 for shared IDs | Six crates published; examples; no unapproved unsafe |
| **M5 - Harden and 1.0** | Weeks 30-52 | Remaining providers, docs site, design partners, 1.0 | M2-M4 + adoption | Three external projects; GIA production; API freeze; migration guide |

## M0 - Foundation (weeks 0-2)

### Goal

Make the repository safe to publish from. Finish the one crate that already exists and put the quality baseline in place.

### Backlog

| ID | Item |
|---|---|
| BP-001 | Finish `dado-yaml` YAML parser; publish `dado-yaml` 0.1.0 |
| BP-002 | CI: fmt, clippy, test, docs, MSRV |
| BP-003 | `CONTRIBUTING.md`, `SECURITY.md`, issue/PR templates |
| BP-004 | Release checklist and crates.io publishing runbook |

### Work detail

- **`dado-yaml` parser:** implement the YAML 1.2 core subset described in `crates/dado-yaml/README.md`; reject unsupported constructs with positioned errors; replace the `D0001` scaffold.
- **CI:** run `cargo fmt --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo test --workspace --all-features`, `cargo doc --workspace --all-features --no-deps`, and an MSRV check on 1.85.
- **Contributor docs:** state the branch policy, the test requirement, the source-citation rule, and how to report a security issue privately.
- **Release runbook:** metadata checklist, version bump, changelog, `cargo publish --dry-run`, publish order, and rollback guidance.

### Exit criteria

- [ ] `dado-yaml` 0.1.0 is published and installable.
- [ ] CI is green on `main`.
- [ ] `CONTRIBUTING.md` and `SECURITY.md` exist.
- [ ] A release checklist exists and has been used once.

### Risks

- The YAML parser is the largest single piece of work in M0. If it slips, split `dado-yaml` into a 0.1.0 with the four implemented modules and ship YAML in 0.2.0.
- CI credentials for crates.io must be set up before the first publish.

## M1 - First publish (weeks 3-8)

### Goal

Ship `wirekit` 0.1.0 with the four lowest-friction providers. Prove the unified model end to end.

### Backlog

| ID | Item |
|---|---|
| BP-005 | `wirekit` core: error, retry, token bucket, pooled client, tracing, auth |
| BP-006 | NOAA provider module |
| BP-007 | Twilio provider module |
| BP-008 | SendGrid provider module |
| BP-009 | Slack provider module |
| BP-010 | Publish `wirekit` 0.1.0 with four features, examples, docs |

### Work detail

- **Core:** `WirekitError`, exponential backoff with jitter, 3 attempts by default, `Retry-After` handling, a per-provider token bucket, one pooled `reqwest::Client`, tracing spans, and auth abstractions for bearer, basic, HMAC, OAuth2 client credentials, OAuth2 authorization code, API key in header, and API key in query.
- **NOAA:** no auth; prove the client with the lowest possible friction.
- **Twilio:** SMS, voice, MFA, lookup; never retry other 4xx; carry the messaging service SID.
- **SendGrid:** transactional email; respect `Retry-After`; separate marketing and transactional unsubscribe groups.
- **Slack:** handle `ok: false` and HTTP 200 error bodies; never treat 200 as success.
- **Publish:** one runnable example per provider; a provider matrix; a feature table; docs on docs.rs.

### Exit criteria

- [ ] `wirekit` 0.1.0 is published with `twilio`, `sendgrid`, `noaa`, and `slack`.
- [ ] One example per provider compiles and runs against a mock or sandbox.
- [ ] Error mapping tests cover 401, 403, 429, 5xx, timeout, and 200-with-error.
- [ ] A developer can go from README to a successful call without reading source.

### Risks

- Twilio and SendGrid sandbox access.
- Slack's body-level error model is easy to get wrong; test it explicitly.
- Feature-flag combinations can explode; test default, each feature, and all features.

## M2 - Critical path (weeks 9-16)

### Goal

Add the providers that GIA's field workflow needs, plus a shared Google OAuth helper, and publish `wirekit` 0.2.x.

### Backlog

| ID | Item | External dependency |
|---|---|---|
| BP-011 | Meta Graph provider module | App review / business verification |
| BP-012 | EagleView provider module | Commercial contract |
| BP-013 | QuickBooks provider module | OAuth app approval |
| BP-014 | Stripe provider module | Decision: build vs community crate |
| BP-015 | Google OAuth shared helper | Google Cloud project |
| BP-016 | Publish `wirekit` 0.2.x | - |

### Work detail

- **Meta:** Messenger, Instagram, WhatsApp; respect the 24-hour messaging window and approved templates.
- **EagleView:** order and poll; the client must make per-report cost explicit; do not assume a webhook exists.
- **QuickBooks:** OAuth refresh; surface sync failures in the dashboard, never swallow them.
- **Stripe:** decide whether to wrap Stripe or keep a community crate and remove Stripe from examples. This is an open question, not a silent choice.
- **Google OAuth:** Authorization Code + PKCE only; minimum scopes; refresh once on 401, never loop.
- **Publish:** 0.2.x with all eight critical-path providers and the OAuth helper.

### Exit criteria

- [ ] All critical-path modules compile behind their features.
- [ ] Provider access and sandbox accounts are confirmed for each module.
- [ ] The Google OAuth flow is tested with a real or documented test project.
- [ ] `wirekit` 0.2.x is published with a migration note for 0.1 users.

### Risks

- Meta, EagleView, and QuickBooks approvals can take longer than the coding.
- If an approval is blocked, ship 0.2.x without that provider and record it as `needs discovery`; do not fake the integration.
- Stripe may be better served by a community crate; decide before writing code.

## M3 - Integration substrate (weeks 12-24)

### Goal

Ship the crates that make the ecosystem more than an HTTP client. This track runs in parallel with M2 and shares only `wirekit` core.

### Backlog

| ID | Item |
|---|---|
| BP-017 | `platform-core` 0.1: traits, registry, token store |
| BP-018 | `quota-ledger` 0.1: core + Postgres backend |
| BP-019 | `quota-ledger` Redis backend |
| BP-020 | `hookgate` 0.1: verifiers, dedupe, replay, outbox |
| BP-021 | `syncforge` 0.1: queue, idempotency, conflicts, traits |

### Work detail

- **`platform-core`:** `TenantId`, `ProviderId`, `ProvisioningMode`, `Capability`, `Provider` trait, `TokenStore` trait, explicit registry. No product nouns, no OAuth implementation.
- **`quota-ledger`:** budgets, reserve/commit/release, exhaustion policies (`HardStop`, `Overage`, `Degrade`, `Queue`), Postgres-backed ledger. Redis is a later backend.
- **`hookgate`:** `Verifier` trait, `DeliveryId`, `IdempotencyKey`, raw-body preservation, replay window, outbox, dead-letter, tenant isolation.
- **`syncforge`:** durable action queue, UUID v7 idempotency, conflict strategies, audit timestamps, `Storage` and `Transport` traits, in-memory test implementations.

### Exit criteria

- [ ] Each crate publishes 0.1.0.
- [ ] One integration example shows `platform-core` + `quota-ledger` + `hookgate` together.
- [ ] Tenant isolation and replay protection have tests.
- [ ] No crate forces a database on a consumer that does not need one.

### Risks

- Database and Redis testing infrastructure.
- The trait surface can become too abstract; keep product nouns out.
- `syncforge` is the most complex design; do not rush it.

## M4 - Media and links (weeks 20-36)

### Goal

Ship the media and link family. These crates are independent of the HTTP client but share IDs and conventions from `platform-core`.

### Backlog

| ID | Item |
|---|---|
| BP-022 | `ffscene` 0.1 |
| BP-023 | `mixbus` 0.1 |
| BP-024 | `replay-recorder` 0.1 |
| BP-025 | `studio-protocol` 0.1 |
| BP-026 | `rtmp-publish` 0.1 |
| BP-027 | `signedlink` 0.1 |

### Work detail

- **`ffscene`:** deterministic scene-to-filter-graph compilation; validate cycles, missing sources, transforms, and blend modes before FFmpeg starts; encoder ladder; GPU probe.
- **`mixbus`:** gain, pan, gate, compressor, ducker, loudness; never lock on the audio thread; meters from snapshots.
- **`replay-recorder`:** isolated tracks, replay ring, segment/tee, disk budget; never fill the volume.
- **`studio-protocol`:** envelope, messages, permissions, pure state transitions; server-side permission checks.
- **`rtmp-publish`:** destination model, key redaction, health, retry with circuit breaker, failover.
- **`signedlink`:** sign/verify, short code, UTM normalisation, attribution window, single use.

### Exit criteria

- [ ] Each crate publishes 0.1.0.
- [ ] Each has at least one runnable example.
- [ ] Secret redaction is property-tested in `rtmp-publish`.
- [ ] `studio-protocol` transitions are pure and replayable.
- [ ] No crate requires `unsafe` unless an amendment approves it.

### Risks

- Media crates are large; each could slip independently.
- `mixbus` plugin hosting may need `unsafe`; BP-036 keeps it out until approved.
- FFmpeg availability in CI.

## M5 - Harden and 1.0 (weeks 30-52)

### Goal

Turn a working ecosystem into a trusted one: remaining providers, docs, design partners, and an API freeze.

### Backlog

| ID | Item |
|---|---|
| BP-028 | Remaining 17 provider modules |
| BP-029 | Public docs site and provider matrix |
| BP-030 | External design partners and case studies |
| BP-031 | API freeze audit and 1.0 release |
| BP-032 | Migration guide 0.x to 1.0 |
| BP-034 | Google LSA module (needs discovery) |
| BP-035 | ADP / Sage Intacct modules (needs discovery) |

### Work detail

- **Remaining providers:** AccuWeather, ATTOM, CoreLogic, BuildZoom, Google Business, Jobber, Sage Intacct, ADP, Gusto, Lob, Yelp, PagerDuty, Deepgram, Klarna, Wisetack, LeadPerfection, Google OAuth (if not already in M2). Each gets a design file before implementation.
- **Docs site:** landing, crate docs, provider matrix, examples, error reference, operations, upgrading.
- **Design partners:** two or three external teams using `wirekit` in production; publish case studies with permission.
- **API freeze:** audit public APIs, remove accidental surface, decide semver policy, publish 1.0.
- **Migration guide:** 0.x to 1.0, with code examples.

### Exit criteria

- [ ] Three external projects use `wirekit` in production.
- [ ] GIA uses `wirekit` in production.
- [ ] All 11 crates are published at 0.x or 1.0.
- [ ] The API freeze audit is complete.
- [ ] The migration guide is published.
- [ ] 1.0.0 is tagged and announced.

### Risks

- Adoption is the hardest part; design partners must be recruited early, not at the end.
- 1.0 may reveal API mistakes that are expensive to fix; the audit is the safety net.
- Provider access for Google LSA, ADP, and Sage may not materialize; keep them `needs discovery`.

## Release sequence

| Order | Release | Contents |
|---|---|---|
| 1 | `dado-yaml` 0.1.0 | YAML, diag, graph, emit, glob |
| 2 | `wirekit` 0.1.0 | Core + Twilio, SendGrid, NOAA, Slack |
| 3 | `wirekit` 0.2.x | Critical-path providers + Google OAuth |
| 4 | `platform-core` 0.1.0 | Traits, registry, token store |
| 5 | `quota-ledger` 0.1.0 | Core + Postgres |
| 6 | `hookgate` 0.1.0 | Verify, dedupe, replay, outbox |
| 7 | `syncforge` 0.1.0 | Queue, idempotency, conflicts |
| 8 | Media and link crates 0.1.0 | ffscene, mixbus, replay-recorder, studio-protocol, rtmp-publish, signedlink |
| 9 | `wirekit` 0.2.x+ | Remaining provider modules |
| 10 | 1.0.0 | API freeze across the ecosystem |

`quota-ledger` Redis (BP-019) can ship after the Postgres backend without blocking the sequence.

## Decision gates

| Gate | Question | If no |
|---|---|---|
| G0 | Is `dado-yaml` publishable and is CI green? | Do not publish `wirekit`; fix the baseline |
| G1 | Is `wirekit` 0.1.0 usable by a developer who has never seen the source? | Do not start M2; improve docs and examples |
| G2 | Is provider access confirmed for each critical-path module? | Ship 0.2.x without the blocked provider; mark `needs discovery` |
| G3 | Do the substrate crates compose without a framework? | Do not start M4; fix the traits |
| G4 | Is the media family safe (redaction, permissions, no unapproved unsafe)? | Hold the affected crate |
| G5 | Are three external projects and GIA in production? | Do not freeze 1.0 |

## What would change the plan

| Event | Response |
|---|---|
| A design partner needs a specific provider | Promote that provider ahead of the M5 remaining-provider batch |
| A provider denies access | Mark the module `needs discovery`; do not fake it |
| A community crate covers a provider well | Recommend it and remove the module from scope |
| `wirekit` feature flags make builds painful | Propose a `wirekit-streaming` companion crate |
| Turborepo's Rust support stabilizes | Re-evaluate BP-037 against measured CI time |
| A JavaScript/TypeScript surface appears | Re-evaluate Turborepo and add a Node toolchain |
| MSRV 1.85 becomes a blocker | Amend the constitution and publish an MSRV policy |
| 1.0 demand arrives earlier | Run the freeze audit early; do not skip adoption evidence |

## Turborepo

**Decision:** not adopted now. The repository uses plain Cargo plus GitHub Actions for M0.

Turborepo 2.11 added experimental native Cargo workspace support. It requires a root `Cargo.toml`, `[workspace.metadata].name`, a root `Cargo.lock`, a `turbo.json` with `experimentalCargoWorkspaces`, and an npm-installed `turbo` binary. The current workspace is Rust-only, small, and ignores `Cargo.lock`, so the cost is not justified today.

Revisit criteria (BP-037):

1. CI time becomes a measured bottleneck.
2. The workspace grows enough that affected-task filtering matters.
3. A JavaScript/TypeScript docs site or dashboard is added.
4. Turborepo's Rust support leaves experimental status.

Sources: <https://turborepo.dev/docs/guides/tools/rust>, <https://turborepo.dev/blog/2-11>, <https://github.com/vercel/turborepo/discussions/13415>.

## Open questions

1. Is the M2 provider order correct, or should QuickBooks come before EagleView?
2. Should `syncforge` ship before `quota-ledger` and `hookgate` because the field app needs it first?
3. Should the docs site be part of M1 or wait until M5?
4. What is the minimum adoption evidence for 1.0: three projects, six months of use, or a specific production incident history?
5. Should the remaining providers be one milestone or split by access difficulty?
6. Who owns provider access applications, and by when?
7. Should the first publish include a 0.1.0 `wirekit` with only NOAA, Twilio, and SendGrid if Slack approval is slow?
