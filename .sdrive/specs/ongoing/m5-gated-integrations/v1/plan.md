---
title: M5 Gated Integrations - technical plan
status: backlog
version: v1
created: 2026-10-29
updated: 2026-10-29
---

# M5 Gated Integrations - Technical Plan

## Approach

Four additive, default-off modules inside the existing `wirekit` crate. No new
crate, no new workspace member, no root manifest change (CON-101, CON-104,
CON-103). Each module is one file plus one feature flag, and every module depends
on `wirekit` core for the client, retry, rate limit, tracing, secret type, and
error type - none of which this feature re-implements.

The ordering is deliberate. `angi` ships first because it is the only module with
no network dependency: it parses bytes the caller already has. The three live
modules follow in descending certainty about their contract: LSA has a documented
base URL and OAuth2 reuse, Intacct has a documented gateway and an XML schema we
intentionally cut down to three entities, and ADP has the most consequential
retry semantics (payroll), so it goes last with the most explicit reconcile
design.

Nothing here is exercised against a live endpoint. Every live module takes a base
URL at construction, so a sandbox or a local stub can stand in until partner
access arrives, and no task depends on a credential existing.

### Preconditions

1. **`wirekit` core must exist on the base branch.** `BP-005` is implemented on
   `feature/m1-first-publish` and is not on `main`. Task `T0.1` verifies it; if it
   fails, this feature is blocked and `/sdrive:apply` must not start.
2. **Two library choices are made here and confirmed at apply.** `quick-xml` for
   the Intacct gateway and `csv` for Angi exports. Both are gated behind their own
   feature, so neither becomes a baseline dependency of `wirekit`. Versions are
   pinned at apply time from crates.io, and each licence is checked for
   `MIT OR Apache-2.0` compatibility before it is added.
3. **No provider path is invented.** The catalogue records base URLs only. Exact
   endpoint paths, required scopes, and payload shapes are confirmed against each
   provider's own documentation at implementation time and recorded in the module
   rustdoc. Until then the plan fixes the *contract* - types, errors, redaction,
   retry, idempotency - not the URL strings.

## Module Map

| Module | Feature | File | Product item | Network |
|---|---|---|---|---|
| `angi` | `angi` | `src/angi.rs` | BP-033 | none - parses caller bytes |
| `google_lsa` | `google-lsa` | `src/google_lsa.rs` | BP-034 | LSA API over OAuth2 |
| `sage_intacct` | `sage-intacct` | `src/sage_intacct.rs` | BP-035 | XML gateway |
| `adp` | `adp` | `src/adp.rs` | BP-035 | REST over OAuth2 client credentials |

Each module is `#[cfg(feature = "...")] pub mod ...;` in `src/lib.rs` and opens
with a `//!` comment naming its scope and its non-goals (CON-203).

### Dependency direction

```
consumer app -> wirekit::<module> -> wirekit core (client, retry, bucket,
                                      tracing, secret, WirekitError)
                                  -> quick-xml  (sage-intacct only)
                                  -> csv        (angi only)
```

No module depends on another. Enabling one feature never enables another
(CON-204), which is what `AC-023` checks.

### Error flow

```
provider failure -> module parse/mapping -> WirekitError -> caller
```

`angi::ParseError` is the one exception the catalogue requires, and it is not a
second top-level error: it carries the offending field and converts into
`WirekitError` through `From`, so a caller handles both uniformly (FR-026).

Two error classes must never be folded into a generic failure:

- **LSA 403** means the Ads account is not linked or not approved, not that the
  token is wrong (FR-009).
- **ADP payroll timeout** means the outcome is unknown, not that the submission
  failed (FR-022). A retry there is a double-payment risk, so the module reports
  the unknown state and lets the caller reconcile.

## Cross-Cutting Decisions

| Decision | Choice | Why |
|---|---|---|
| Client ownership | One client per tenant, constructed by the caller | No global singleton (CON-109); isolation stays structural (NFR-002) |
| Credentials | Passed at construction, held in the `wirekit` secret type | Never embedded, never logged (CON-511, CON-519, CON-214) |
| Retry | Reuse `wirekit` core policy; modules only classify errors retryable or not | One policy, one place (CON-208) |
| Rate limiting | Reuse the core token bucket with a per-provider key | One bucket implementation (CON-209, CON-514) |
| Tracing | Reuse core spans; modules add provider and operation fields, never values | CON-211 without leaking (NFR-005) |
| Base URL | Constructor argument, defaulted to the documented production URL, `https` enforced | Sandbox without access (FR-030, CON-520) |
| Idempotency | External key supplied by the caller; query before create | Safe replays (FR-011, FR-017, FR-022) |
| Unknown input | Typed error naming the field; never a default | CON-206, NFR-004 |

## Data Model

No persistence anywhere. These are the in-memory models each module returns.

| Entity | Module | Key fields |
|---|---|---|
| `AngiLead` | `angi` | source, lead id (optional), contact name, phone, email, address, service, received-at |
| `AngiImportOutcome` | `angi` | row number, outcome (imported / skipped / failed), reason |
| `ParseError` | `angi` | field, reason |
| `Lead` | `google_lsa` | lead id, lead type, contact, job details, attribution |
| `Booking` | `google_lsa` | booking id, lead id, status, attribution |
| `LeadStatus` | `google_lsa` | closed set of statuses from `catalogue/21_google_lsa.md` |
| `Customer` | `sage_intacct` | external key, name, provider id |
| `Invoice` | `sage_intacct` | external key, customer id, line items |
| `JournalEntry` | `sage_intacct` | external key, debits, credits |
| `ReportRequest` | `sage_intacct` | report name, parameters |
| `Employee` | `adp` | worker id, name, identifiers (redacted on display) |
| `TimeCard` | `adp` | worker id, hours, earnings, approval state |
| `PayrollRun` | `adp` | run id, cycle, status |
| `PayrollReport` | `adp` | run id, per-record outcomes |
| `SubmissionOutcome` | `adp` | sent / failed (record ids) / unknown outcome (client reference) |

Every model implements `Debug` (CON-216), and every model holding a secret or a
personal field implements it by hand with the value redacted (CON-214, AC-012,
AC-019).

## Security Baseline Expansion

`CON-511` to `CON-520` as they apply to four client modules. Every `REQUIRED`
control has a requirement and at least one task; the `NOT APPLICABLE` decision is
recorded rather than skipped.

| Control | Status | Requirement | Task |
|---|---|---|---|
| CON-511 Credentials | REQUIRED | NFR-001 | T6.3 |
| CON-512 Authorization / tenant isolation | REQUIRED | NFR-002 | T6.1 |
| CON-513 Security headers | NOT APPLICABLE - no HTTP server or response pipeline is owned here | - | - |
| CON-514 Rate limiting | REQUIRED | NFR-003 | T3.5 |
| CON-515 Input validation | REQUIRED | NFR-004 | T6.5 |
| CON-516 Injection protection | NOT APPLICABLE - no database, cache, or query surface (NFR-011) | NFR-011 | - |
| CON-517 XSS | NOT APPLICABLE - no markup is rendered | - | - |
| CON-518 CSRF | NOT APPLICABLE - no browser session or cookie state | - | - |
| CON-519 Environment security | REQUIRED | NFR-001 | T6.3 |
| CON-520 HTTPS / HSTS | REQUIRED | FR-030 | T6.4 |

## Performance

| Requirement | Strategy |
|---|---|
| NFR-007 (bounded memory) | Byte and depth ceilings enforced before or during parse; no unbounded read of a provider body |
| NFR-004 (no panic) | Typed errors on every untrusted input path; no `unwrap` on parsed data |
| NFR-005 (tracing cost) | Events per request, not per field; redacted values are never formatted |
| NFR-003 (rate limiting) | Shared core bucket; one timestamp update per request |

No latency target is claimed. These are I/O-bound clients and the provider sets
the floor; what the feature guarantees is a bound, not a speed.

## Dependencies

| Dependency | Type | Action | Notes |
|---|---|---|---|
| `wirekit` core (BP-005) | internal | no_change | Consumed as-is; not on `main` yet (precondition T0.1) |
| `wirekit::google_oauth` | internal | no_change | Reused for LSA refresh (CON-212) |
| `wirekit` secret type, `WirekitError`, retry, bucket, tracing | internal | no_change | Reused, never duplicated |
| `quick-xml` | library | add | `sage-intacct` feature only; version pinned at apply; licence checked; no skill expected |
| `csv` | library | add | `angi` feature only; version pinned at apply; licence checked; no skill expected |
| Google Ads / LSA access | external | no_change | Live-use gate; not implemented by any task |
| Sage Intacct partner agreement | external | no_change | Live-use gate |
| ADP partner / enterprise access | external | no_change | Live-use gate |
| Angi partner feed | external | no_change | Optional capability only |

## Risks

| Risk | Mitigation |
|---|---|
| `wirekit` never lands on the base branch | T0.1 blocks the feature before any code is written |
| A dependency brings an incompatible licence | Licence checked before adding; both are feature-gated, so removal is cheap |
| Provider paths invented from memory | Paths confirmed from provider documentation at apply and recorded in rustdoc; the plan fixes the contract only |
| Partner access denied mid-implementation | Every live module is sandbox-addressable; no task depends on a credential |
| A payroll retry double-pays | Unknown-outcome state plus a caller client reference (FR-022, T5.4) |
| Four modules drift into a framework | No shared state, no cross-module dependency, one crate (FR-027, FR-028) |

## Open Questions

1. `[to be confirmed]` Exact LSA endpoint paths, required scopes, and whether lead
   notifications arrive via Pub/Sub - confirm from provider documentation and the
   Ads account before T3.3.
2. `[to be confirmed]` Which Intacct entities and reports are stable enough to
   depend on in v1 - catalogue open question 4.
3. `[to be confirmed]` Which ADP product family (Workforce Now or Vantage) is in
   scope; the API surface differs - catalogue open question 2.
4. `[to be confirmed]` Whether a real Angi partner feed exists for any tenant; the
   module ships without assuming one.

## Verification

Formatter, linter, documentation, and feature-combination checks only. Test
authoring and execution belong to `/sdrive:test`, so no task here writes or runs
tests.

```sh
cargo fmt --all --check
cargo clippy -p wirekit --all-features -- -D warnings
cargo doc -p wirekit --all-features --no-deps          # RUSTDOCFLAGS="-D warnings"
cargo check -p wirekit --no-default-features
cargo check -p wirekit --no-default-features --features angi
cargo check -p wirekit --no-default-features --features google-lsa
cargo check -p wirekit --no-default-features --features sage-intacct
cargo check -p wirekit --no-default-features --features adp
node .sdrive/scripts/validate-governance.js
```
