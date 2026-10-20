# Product Backlog

> **Status:** Approved product plan
> **Date:** 2026-10-20
> **Scope:** Product-level candidates only. No feature specs, governance JSON, tasks, or workflow-state entries are created by this document.

## How to use this backlog

- Every item is a **candidate**, not started work.
- `backlog` means approved as a candidate and available for promotion.
- `deferred` means deliberately not planned now.
- `needs discovery` means access, feasibility, or a decision is unresolved.
- To start work on an item, promote it with `/sdrive:propose "<BP-ID> <title>"`. That command creates the branch, feature folder, spec, plan, traceability, and tasks.
- Do not treat a priority as a commitment. P0 items are the first-publish path; P1/P2/P3 are relative.

## Status legend

| Status | Meaning |
|---|---|
| `backlog` | Approved candidate, not started |
| `deferred` | Deliberately out of the current plan |
| `needs discovery` | Blocked on access, a decision, or feasibility |

## Priority legend

| Priority | Meaning |
|---|---|
| P0 | Required for the first publish or the 1.0 gate |
| P1 | Required for the critical path or ecosystem coherence |
| P2 | Important but not blocking the first publish |
| P3 | Optional or deferred |

## Summary

| Milestone | Items | Count |
|---|---|---|
| M0 - Foundation | BP-001 to BP-004 | 4 |
| M1 - First publish | BP-005 to BP-010 | 6 |
| M2 - Critical path | BP-011 to BP-016 | 6 |
| M3 - Integration substrate | BP-017 to BP-021 | 5 |
| M4 - Media and links | BP-022 to BP-027 | 6 |
| M5 - Harden and 1.0 | BP-028 to BP-032, BP-034, BP-035 | 7 |
| Deferred / discovery | BP-033, BP-036, BP-037 | 3 |
| **Total** | | **37** |

## M0 - Foundation

| ID | Feature | User value | Priority | Depends on | Status | Confidence / source |
|---|---|---|---|---|---|---|
| BP-001 | Finish `dado` YAML parser; publish `dado` 0.1.0 | Rust developers can parse strict YAML with positioned errors and no I/O | P0 | - | backlog | High - `crates/dado/README.md`; user-approved scope |
| BP-002 | CI: fmt, clippy, test, docs, MSRV | Contributors get fast, consistent feedback and the project ships with a quality baseline | P0 | - | backlog | Medium - agent recommendation; no CI exists today |
| BP-003 | `CONTRIBUTING.md`, `SECURITY.md`, issue/PR templates | Contributors know how to propose, test, and report security issues privately | P1 | - | backlog | Medium - agent recommendation |
| BP-004 | Release checklist and crates.io publishing runbook | Maintainers can publish repeatably without missing metadata or checks | P0 | - | backlog | Medium - agent recommendation; required before BP-010 |

### M0 notes

- BP-001 is the only item with existing code. The other three create the baseline the rest of the plan depends on.
- If the YAML parser slips, publish `dado` 0.1.0 with `diag`, `graph`, `emit`, and `glob`, and move YAML to 0.2.0. Record the split before publishing.
- BP-002 must include an MSRV job on Rust 1.85, not just the installed toolchain.

## M1 - First publish

| ID | Feature | User value | Priority | Depends on | Status | Confidence / source |
|---|---|---|---|---|---|---|
| BP-005 | `wirekit` core: error, retry, token bucket, pooled client, tracing, auth | Developers get one client, one error type, one retry policy, one auth layer, and tracing | P0 | BP-001 | backlog | High - `catalogue/01_wirekit.md` |
| BP-006 | NOAA provider module | Developers can fetch forecasts and alerts with no API key | P0 | BP-005 | backlog | High - `catalogue/07_noaa.md` |
| BP-007 | Twilio provider module | Developers can send SMS, voice, MFA, and lookup requests through one typed client | P0 | BP-005 | backlog | High - `catalogue/03_twilio.md` |
| BP-008 | SendGrid provider module | Developers can send transactional email with template and unsubscribe support | P0 | BP-005 | backlog | High - `catalogue/04_sendgrid.md` |
| BP-009 | Slack provider module | Developers can post notifications and handle Slack's body-level errors correctly | P0 | BP-005 | backlog | High - `catalogue/23_slack.md` |
| BP-010 | Publish `wirekit` 0.1.0 with four features, examples, docs | External developers can install `wirekit` 0.1.0 and run a provider example | P0 | BP-006, BP-007, BP-008, BP-009 | backlog | High - user-approved first-publish scope |

### M1 notes

- BP-005 is the shared model. Do not start provider modules until the error, retry, rate-limit, and auth contracts are stable enough to test.
- Each provider needs a design file check before implementation: auth, base URL, retry, rate limit, webhooks, and test plan.
- BP-010 is a release, not a coding task. It needs docs, examples, a provider matrix, and a changelog.

## M2 - Critical path

| ID | Feature | User value | Priority | Depends on | Status | Confidence / source |
|---|---|---|---|---|---|---|
| BP-011 | Meta Graph provider module | Developers can integrate Messenger, Instagram, and WhatsApp under Meta's rules | P1 | BP-005; app review | backlog | Medium - `catalogue/05_meta.md`; access risk |
| BP-012 | EagleView provider module | Developers can order and poll roof measurements with explicit cost | P1 | BP-005; commercial contract | backlog | Medium - `catalogue/12_eagleview.md`; access risk |
| BP-013 | QuickBooks provider module | Developers can sync accounting data with refresh and visible failures | P1 | BP-005; OAuth approval | backlog | Medium - `catalogue/13_quickbooks.md`; access risk |
| BP-014 | Stripe provider module (build vs community crate) | Developers can take payments with a typed client, or use a documented community crate | P1 | BP-005; decision | backlog | Low - not in the 26; `catalogue/00_provider_index.md` open question |
| BP-015 | Google OAuth shared helper | Developers get one correct PKCE flow and token refresh path for Google APIs | P1 | BP-005 | backlog | High - `catalogue/28_google_oauth.md` |
| BP-016 | Publish `wirekit` 0.2.x with critical-path providers | External developers can use the critical-path providers in 0.2.x | P1 | BP-011 to BP-015 | backlog | High - user-approved critical path |

### M2 notes

- BP-011, BP-012, BP-013 each depend on an external approval. Start the applications during M1, not after M1.
- BP-014 is a decision item before it is a coding item. The catalogue docs say Stripe is missing from the 26 even though examples reference it.
- If an approval is blocked, ship 0.2.x without that provider and mark it `needs discovery`. Do not ship a stub that pretends to work.

## M3 - Integration substrate

| ID | Feature | User value | Priority | Depends on | Status | Confidence / source |
|---|---|---|---|---|---|---|
| BP-017 | `platform-core` 0.1: traits, registry, token store | App developers get tenant/provider identity, capabilities, and a token-store trait without product nouns | P1 | BP-005 | backlog | High - `catalogue/31_platform_core.md` |
| BP-018 | `quota-ledger` 0.1: core + Postgres backend | Services can reserve and commit provider quota across processes with Postgres | P1 | BP-017 | backlog | High - `catalogue/29_quota.md` |
| BP-019 | `quota-ledger` Redis backend | Services can use Redis for quota state when Postgres is too heavy | P2 | BP-018 | backlog | Medium - backend choice is open |
| BP-020 | `hookgate` 0.1: verifiers, dedupe, replay, outbox | Services can verify, dedupe, replay-protect, and forward webhooks without losing events | P1 | BP-017 | backlog | High - `catalogue/30_webhook_gateway.md` |
| BP-021 | `syncforge` 0.1: queue, idempotency, conflicts, traits | Field apps can queue actions offline and replay them idempotently | P1 | BP-017 | backlog | High - `catalogue/02_syncforge.md` |

### M3 notes

- These crates are independent of provider count. They can run in parallel with M2.
- `platform-core` must stay a trait crate: no OAuth, no quota client, no webhook client, no product nouns.
- `quota-ledger` and `hookgate` must not force a database on consumers that do not need one.
- `syncforge` is the highest-design-risk item in M3; do not rush the queue and conflict model.

## M4 - Media and links

| ID | Feature | User value | Priority | Depends on | Status | Confidence / source |
|---|---|---|---|---|---|---|
| BP-022 | `ffscene` 0.1: scene to filter graph, ladder, GPU probe | Developers can compile a scene into a deterministic FFmpeg graph | P2 | - | backlog | High - `catalogue/32_ffscene.md` |
| BP-023 | `mixbus` 0.1: gain, pan, gate, compressor, ducker, loudness | Developers can mix live audio with gain, ducking, gating, and loudness | P2 | - | backlog | High - `catalogue/33_mixbus.md` |
| BP-024 | `replay-recorder` 0.1: tracks, ring, segment/tee, disk budget | Developers can record isolated tracks and keep a replay ring within a disk budget | P2 | - | backlog | High - `catalogue/34_replay_recorder.md` |
| BP-025 | `studio-protocol` 0.1: envelope, permissions, state machine | Developers can implement preview/program control with server-side permissions | P2 | - | backlog | High - `catalogue/35_studio_protocol.md` |
| BP-026 | `rtmp-publish` 0.1: destination, redaction, health, failover | Developers can model RTMP/SRT/WHIP destinations without leaking stream keys | P2 | - | backlog | High - `catalogue/36_rtmp_publish.md` |
| BP-027 | `signedlink` 0.1: sign/verify, short code, UTM, attribution | Developers can issue signed, expiring links with UTM and attribution | P2 | - | backlog | High - `catalogue/37_signedlink.md` |

### M4 notes

- These crates share IDs and conventions with `platform-core` but do not require `wirekit`.
- `rtmp-publish` redaction is the single most important behavior in the crate; it needs property tests.
- `mixbus` must not require `unsafe` in the first release. Plugin hosting is BP-036.
- `studio-protocol` transitions must be pure functions of `(state, message, actor)`.

## M5 - Harden and 1.0

| ID | Feature | User value | Priority | Depends on | Status | Confidence / source |
|---|---|---|---|---|---|---|
| BP-028 | Remaining 17 provider modules | Developers can use the remaining providers through the same model | P2 | BP-016 | backlog | Medium - `catalogue/00_provider_index.md`; access varies |
| BP-029 | Public docs site and provider matrix | Developers can discover crates, providers, and examples from one place | P1 | BP-010 | backlog | Medium - agent recommendation |
| BP-030 | External design partners and case studies | The project has real production evidence and feedback | P1 | BP-010 | backlog | Medium - user-approved success measure |
| BP-031 | API freeze audit and 1.0 release | External users get a stable 1.0 API with a documented freeze | P0 | BP-028, BP-029, BP-030 | backlog | High - user-approved stability model |
| BP-032 | Migration guide 0.x to 1.0 | 0.x users can upgrade without guessing | P1 | BP-031 | backlog | High - required by the 1.0 freeze |
| BP-034 | Google LSA module | Google LSA users can ingest leads if Ads API access is available | P2 | Ads API access | needs discovery | Low - `catalogue/21_google_lsa.md`; access unresolved |
| BP-035 | ADP / Sage Intacct modules | ADP and Sage users can integrate payroll/accounting through partner access | P2 | partner access | needs discovery | Low - `catalogue/15_sage_intacct.md`, `catalogue/16_adp.md` |

### M5 notes

- BP-028 is a batch, not a single provider. Each module still needs its own design file and approval.
- BP-029 should be planned before the 1.0 announcement, not after.
- BP-030 is the evidence gate for 1.0. Recruit design partners during M1/M2, not at M5.
- BP-031 is an audit, not a version bump. It should remove accidental public surface, not just tag a release.

## Deferred and needs discovery

| ID | Feature | User value | Priority | Depends on | Status | Confidence / source |
|---|---|---|---|---|---|---|
| BP-033 | Angi import helper | Angi users get an import helper that is honest about the lack of a public API | P3 | - | deferred | Low - `catalogue/20_angi.md`; no public API |
| BP-036 | VST3/CLAP hosting in `mixbus` | `mixbus` users can host plugins if the unsafe exception is approved | P3 | constitution amendment | deferred | Low - `catalogue/33_mixbus.md`; conflicts with CON-201 |
| BP-037 | Evaluate Turborepo experimental Cargo support | Maintainers can evaluate Turborepo without committing to it | P3 | committed `Cargo.lock`, workspace metadata, `turbo.json`, npm `turbo` | deferred | Medium - Turborepo 2.11 experimental Rust support; not adopted now |

### Deferred notes

- **BP-033:** Angi has no public API. The catalogue explicitly says not to promise an integration that cannot be built. An import helper is honest only if the user understands it is manual/partner-mediated.
- **BP-036:** `mixbus` plugin hosting may require `unsafe`. CON-201 allows an amendment path; do not take it silently.
- **BP-037:** Turborepo 2.11 added experimental native Cargo support, but it requires a root `Cargo.toml`, `[workspace.metadata].name`, a root `Cargo.lock`, and an npm-installed `turbo`. The current workspace is Rust-only, small, and ignores `Cargo.lock`. Revisit when CI time or a JS/TS surface justifies it.

## Provenance and confidence

### Confirmed by the user

- Product scope: public Rust package ecosystem.
- 6-12 month outcome: publish `dado` + `wirekit` critical path.
- Primary users: external Rust developers.
- Catalogue docs: reference, not the plan.
- Product promise: one dependency, one model.
- v0.1 scope: all 11 crates, sequenced.
- First publish: Twilio, SendGrid, NOAA, Slack.
- Stability: 0.x, freeze at 1.0 later.
- Success: three external projects + GIA in production.
- Turborepo: not adopted now; BP-037 deferred.

### Derived from the repository

- Crate boundaries, provider list, auth/retry/security rules: `README.md`, `catalogue/00_provider_index.md`, and `catalogue/01_wirekit.md` through `catalogue/37_signedlink.md`.
- Workspace, MSRV, licence: `Cargo.toml`.
- `dado` status: `crates/dado/README.md`.
- Governance constraints: `.sdrive/constitution.md`.

### Agent recommendations

- Milestone sequence M0-M5.
- CI, contributor docs, release runbook in M0.
- Docs site and design partners in M5.
- Design-system patterns and example templates.
- Turborepo deferral criteria.

### Assumptions to revisit

- Provider access can be obtained within the roadmap window.
- External developers accept 0.x.
- MSRV 1.85 remains acceptable.
- A single maintainer is sufficient until contributors join.
- No JavaScript/TypeScript surface is planned.

## Open questions by item

| Item | Open question |
|---|---|
| BP-001 | Split `dado` if YAML slips, or hold the release? |
| BP-005 | Is `WirekitError` `#[non_exhaustive]` from 0.1? |
| BP-007 | Which Twilio products are in 0.1: SMS only, or SMS + voice + MFA + lookup? |
| BP-010 | Is the first publish 0.1.0 or 0.0.x? The plan says 0.1.0. |
| BP-014 | Build Stripe or recommend a community crate? |
| BP-015 | Which Google APIs share the OAuth helper in 0.2? |
| BP-017 | Is `platform-core` a dependency of `wirekit` or independent? |
| BP-018 | Postgres only, or Postgres + Redis at 0.1? |
| BP-021 | Does `syncforge` ship before `quota-ledger` and `hookgate`? |
| BP-028 | Which of the remaining 17 providers are must-have vs optional? |
| BP-031 | What adoption evidence is required for 1.0? |
| BP-033 | Is an Angi import helper wanted, or is Angi out of scope entirely? |
| BP-036 | Does plugin hosting justify an `unsafe` amendment? |
| BP-037 | What measured CI threshold would trigger re-evaluation? |

## Promotion to `/sdrive:propose`

Run one of these when you are ready to start:

```
/sdrive:propose "BP-001 finish dado YAML parser and publish dado 0.1.0"
/sdrive:propose "BP-005 wirekit core error retry rate limit tracing auth"
/sdrive:propose "BP-007 Twilio provider module"
```

`/sdrive:propose` will:

1. resolve the base branch and create the feature branch;
2. create the feature folder and version folder;
3. write the specification, traceability, plan, and tasks;
4. stop before implementation.

This backlog deliberately does **not** create `.sdrive/specs/backlog/<feature>/`, governance JSON, tasks, or workflow-state entries. Promotion is the only path from candidate to implementation.
