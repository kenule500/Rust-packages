---
title: M5 Gated Integrations - Angi import, Google LSA, Sage Intacct, ADP
status: backlog
version: v1
created: 2026-10-29
updated: 2026-10-29
---

# M5 Gated Integrations - Angi import, Google LSA, Sage Intacct, ADP

## Overview

### Problem Statement

Three product items remain unspecced in the approved backlog because each is
gated by something outside the repository: `BP-033` (Angi) has no public API at
all, `BP-034` (Google LSA) needs Google Ads / LSA approval, and `BP-035` (ADP and
Sage Intacct) needs partner or enterprise access. Because the gate is unresolved,
none of them has a specification, so every downstream decision - what the typed
surface is, how failures are mapped, what must be redacted, what must never be
retried - is re-decided ad hoc by whoever writes the code first.

That is the expensive part, and it does not depend on the gate. A payroll client
that silently resends a submission it never confirmed, an accounting client that
treats HTTP 200 as success while the XML gateway reported a failure, a leads
parser that drops a malformed row without a trace, or a logging line that carries
an SSN, a pay rate, or a sender password is a defect whether or not a partner
approved the account. Leaving those behaviours unspecified until access arrives
guarantees they are decided by default rather than on purpose.

There is also a hard prerequisite the specification must state plainly:
`wirekit`, the crate these four modules live in, is **not on `main`**. It is
implemented on `feature/m1-first-publish` (`BP-005`) and has not been archived.
Nothing in this feature can compile on `main` until `m1-first-publish` lands, so
this specification defines the contract and blocks implementation on that
dependency rather than pretending it is available.

### Proposed Solution

Deliver `BP-033`, `BP-034`, and `BP-035` as one feature, `m5-gated-integrations`,
adding four feature-flagged modules to `wirekit` - not four new crates
(CON-104) - sequenced so that the part with no external dependency ships first:

1. **`wirekit::angi`** (`BP-033`) - the one module with no live API by design.
   `AngiLead`, `AngiLeadParser`, and `AngiImport` parse documented lead emails and
   CSV exports into a normalized record, emit a structured `ParseError` naming the
   offending field, and compute a deterministic dedupe key. Partner webhook
   verification exists only when a tenant has a provisioned feed. The module is
   honest about the absence of a public API and does not pretend to poll one.
2. **`wirekit::google_lsa`** (`BP-034`) - `GoogleLsaClient`, `Lead`, `Booking`,
   and `LeadStatus` over the LSA endpoints, reusing `wirekit::google_oauth` for
   token refresh (CON-212), with an explicit account-not-approved error for 403,
   dedupe by lead ID, and no assumed webhook.
3. **`wirekit::sage_intacct`** (`BP-035`) - `SageIntacctClient`, `Customer`,
   `Invoice`, `JournalEntry`, and `ReportRequest` over the XML gateway, scoped to
   create/read-by-key for three entities, parsing the gateway's own `status` and
   `errno` so that HTTP 200 is never mistaken for success (CON-213), with the
   sender password held in a redacting secret type (CON-214).
4. **`wirekit::adp`** (`BP-035`) - `AdpClient`, `Employee`, `TimeCard`,
   `PayrollRun`, and `PayrollReport` over OAuth2 client credentials, preserving
   ADP correlation IDs, reporting per-record batch failures so no hours are
   silently dropped, redacting SSN and pay rates, and reconciling instead of
   blindly retrying a timed-out payroll submission.

Every module is additive and default-off (CON-204), maps failures into
`WirekitError` (CON-207), carries a sandbox base-URL override so it is exercisable
without partner credentials, and adds no application domain type, product noun,
or global singleton (CON-106, CON-109).

### Scope

| Item | Product item | Milestone | Backlog status at proposal | This feature |
|---|---|---|---|---|
| Angi import helpers | BP-033 | deferred (P3) | `deferred` | Promoted into scope at the user's request; remains a parser-only module |
| Google LSA module | BP-034 | M5 (P2) | `needs discovery` | In scope; live access stays a gate, not a task |
| Sage Intacct module | BP-035 | M5 (P2) | `needs discovery` | In scope; XML surface limited to three entities |
| ADP module | BP-035 | M5 (P2) | `needs discovery` | In scope; payroll reconcile semantics specified |

### Target Audience

- **Primary:** external Rust developers integrating Angi lead exports, Google
  Local Services Ads, Sage Intacct, or ADP into their own products, who need
  typed models and one error type rather than four ad-hoc HTTP snippets.
- **Secondary:** maintainers of this workspace, who must keep the modules
  consistent with `wirekit`'s shared retry, rate-limit, tracing, secret, and error
  contract without turning `wirekit` into a framework (CON-103).

### Assumptions

- `wirekit` core (`BP-005`) lands from `feature/m1-first-publish` before any task
  in this feature is implemented. Until then this feature is specified, not
  buildable. `[to be confirmed]` - the archive date of `m1-first-publish` is not
  known at proposal time.
- Partner access for Google LSA, ADP, and Sage Intacct is **not** confirmed.
  Every access-dependent behaviour is specified against documented contracts and
  sandbox overrides; no requirement here depends on a live credential being
  present on the build machine.
- Angi has no public API. This module is a parser and importer, permanently. It
  is not a placeholder for a future client (`catalogue/20_angi.md`).
- Rust 1.85 remains the MSRV and edition 2021 remains in force (CON-102).
- The workspace remains rooted at the repository root with `members = ["crates/*"]`
  and the four modules need no root manifest change (CON-101).
- `wirekit::google_oauth` exists in the `wirekit` core from `BP-005` and is reused
  for LSA token refresh rather than duplicated.
- Retry, rate-limit, tracing, secret, and error conventions are already fixed by
  `catalogue/01_wirekit.md`; this specification constrains them per provider, it
  does not restate the core design.
- **No external research available.** Context7 and web search were not reachable
  in this session. No provider documentation, pricing page, or third-party crate
  comparison was fetched. Every claim here comes from the repository: the
  catalogue files, the constitution, the backlog, and `crates/dado` as the
  in-house implementation precedent. Access-gated specifics are marked
  `[to be confirmed]` (CON-704).

### Constitution Alignment

| Rule | How this feature complies |
|---|---|
| CON-101 | Four modules inside the existing workspace; no root manifest change |
| CON-103 | `wirekit` stays one job - typed HTTP clients; no framework, no store |
| CON-104 | Providers ship as feature-flagged `wirekit` modules, never crates |
| CON-105 | Design files already exist: `catalogue/20`, `21`, `15`, `16` |
| CON-106 | No application domain types, billing rules, or product nouns |
| CON-107 | No quota or webhook state is added; `hookgate` and `quota-ledger` own it |
| CON-109 | No global mutable singleton; clients are constructed and injected |
| CON-110 | No dependency on a consuming application |
| CON-201 | `wirekit` keeps `#![forbid(unsafe_code)]`; no exception is requested |
| CON-202 | Every new public item is documented; `deny(missing_docs)` stays |
| CON-203 | Each module file opens with a `//!` comment stating scope and non-goals |
| CON-204 | New features are additive, default-off, and documented |
| CON-205 | Errors carry stable codes and implement `Display` + `Error` |
| CON-206 | Malformed input produces a positioned or field-named error, never a guess |
| CON-207 | One `WirekitError`; `ParseError` converts into it via `From` |
| CON-208 | Retry is exponential with jitter, 3 attempts, 5xx/429/network only |
| CON-209 | Per-provider token buckets with per-provider overrides |
| CON-211 | Request start, finish, retry, rate-limit, and error are traced |
| CON-212 | LSA tokens come from `wirekit::google_oauth` (Authorization Code + PKCE) |
| CON-213 | Intacct `status`/`errno` and ADP error arrays are parsed, not assumed |
| CON-214 | Sender password, SSN, and pay rates are redacted; no `Serialize` |
| CON-215 | Angi partner webhook verification uses the raw request bytes |
| CON-704 | Every unverified access claim is marked `[to be confirmed]` |
| CON-706 | No secret is generated, suggested, or committed; placeholders only |

### Anti-Redundancy Check

Searched before planning; nothing below is assumed to exist.

| Checked | Result | Decision |
|---|---|---|
| `crates/*` on `main` | Only `crates/dado` | `wirekit` modules must be created by `BP-005`, not here |
| `crates/wirekit` on `feature/m1-first-publish` | Present; `WirekitClient`, `WirekitError`, retry, rate limit, `google_oauth` | `reuse` - this feature consumes it, never re-implements it |
| `.sdrive/specs/**` across `main`, `m1`, `m3`, `m4` | `BP-033`, `BP-034`, `BP-035` appear in no spec or governance file | `create` - not a duplicate |
| `.sdrive/specs/**` for `BP-022`-`BP-027` | Already `m4-media-links` v1, phase `ongoing` | Not reproposed; out of scope here |
| `BP-036`, `BP-037` | Deferred; BP-036 conflicts with CON-201 | Excluded from this feature |
| Secret handling helper | None exists in `dado`; `wirekit` owns the contract | `reuse` the `wirekit` secret type |
| Existing Angi / LSA / Intacct / ADP code | None anywhere in the repository | `create` |

## User Stories

### US-001: Import Angi leads without silently dropping one

As a developer whose leads arrive from Angi as email or CSV, I want a parser that
normalizes every lead it can and names the field it could not read, so that an
import neither loses a lead quietly nor breaks on a template change.

**Acceptance Criteria:**

- AC-001: Given a documented Angi lead email fixture, when `AngiLeadParser::parse_email` runs, then it returns an `AngiLead` whose source, contact, address, and received-at are each populated or explicitly absent, and no field is guessed (CON-206).
- AC-002: Given an unknown email template or a record missing a phone number, when parsed, then a structured `ParseError` naming the offending field is returned, no lead is emitted, and `WirekitError::from(parse_error)` preserves the field name (CON-207).
- AC-003: Given a CSV export with missing columns, duplicate rows, and one malformed address, when `AngiImport` runs, then every row yields an outcome of imported, skipped, or failed with its 1-based row number, and the import completes instead of aborting.
- AC-004: Given the same lead delivered once with an Angi lead ID and once without, when the dedupe key is computed, then the ID form and the normalized phone + address + date form are stable across runs, platforms, and input whitespace.

### US-002: Consume Google LSA leads with honest failure

As a developer integrating Local Services Ads, I want typed lead and booking models with an explicit "account not approved" error, so that a missing Ads linkage is distinguishable from a bad credential.

**Acceptance Criteria:**

- AC-005: Given a `GoogleLsaClient` built with an injected token source, when `list_leads` is called, then exactly one HTTPS request is issued to the configured base URL with a bearer token and typed `Lead` values are returned; no raw JSON is exposed to the caller.
- AC-006: Given the API answers HTTP 403 because the Ads account is unlinked or unapproved, when the call fails, then the error is the explicit account-not-approved variant rather than a generic auth error, and the response body is redacted in the message.
- AC-007: Given HTTP 429 with a `Retry-After` header, when the request is retried, then the delay honours the header, the attempt count never exceeds the configured maximum, and request start, finish, retry, and error each emit a tracing event (CON-208, CON-211).
- AC-008: Given a lead status update replayed for the same lead ID, when applied twice, then the second call is idempotent: no duplicate state change and no second mutation request.
- AC-009: Given no Pub/Sub topic or webhook is confirmed for the account, when notification ingestion is invoked, then the module returns an explicit not-configured error instead of assuming a webhook exists.

### US-003: Sync Sage Intacct records without a false success

As a developer syncing accounting data, I want the XML gateway's own status to decide success and the sender password to stay out of every log, so that a failed post is never recorded as posted and a credential never leaks.

**Acceptance Criteria:**

- AC-010: Given valid sender ID, password, company ID, and endpoint, when a customer is created, then a typed request is built, the gateway's success status is parsed, and the returned key is exposed as a typed value rather than a string map.
- AC-011: Given the gateway returns HTTP 200 with a non-success `status` or a non-zero `errno`, when the call completes, then it returns an error carrying the `errno` and description, and no success value is produced (CON-213).
- AC-012: Given the client's `Debug`, `Display`, tracing output, and every error payload, when inspected, then the sender password never appears, and the password type does not implement `Serialize` (CON-214).
- AC-013: Given a response body exceeding the configured byte ceiling or the configured nesting depth, when parsed, then it is rejected with a typed bound error and no partially built model is returned (CON-515).
- AC-014: Given a create for an external key that already exists, when submitted, then the module queries first and returns the existing record instead of creating a duplicate.
- AC-015: Given a business-rule error, when the caller retries, then the retry policy does not resend the unchanged request and emits no retry event for that error class (CON-208).

### US-004: Export payroll and time to ADP without losing hours

As a developer exporting approved hours, I want per-record outcomes and a reconcile state for timeouts, so that a payroll run never silently drops hours and never double-submits.

**Acceptance Criteria:**

- AC-016: Given client-credentials configuration, when a token is requested, then it is cached until expiry, reused across calls, and refreshed exactly once after a 401 before the call is failed.
- AC-017: Given ADP returns its structured error array, when a call fails, then the mapped error preserves the correlation ID and every returned error code and message.
- AC-018: Given a payroll submission times out, when the call returns, then the outcome is an explicit unknown-outcome state requiring reconciliation, and no automatic resubmission occurs (CON-208).
- AC-019: Given worker records carrying an SSN and pay rates, when formatted with `Debug`, traced, or included in an error, then those values are redacted and their types do not implement `Serialize` (CON-214).
- AC-020: Given a batch time-card export where two of ten records fail, when the export completes, then the two failures are reported per record with their identifiers, the eight successes are reported as sent, and no hour is silently dropped.
- AC-021: Given two tenants using the same module concurrently, when either tenant calls, then no credential, cached token, or cached response belonging to the other tenant is observable.

### US-005: Enable one module without paying for the other three

As a developer who needs only one provider, I want each module behind its own additive feature and exercisable without live credentials, so that I do not compile four providers or wait for partner access to use one.

**Acceptance Criteria:**

- AC-022: Given `wirekit` built with no provider features, when it compiles, then none of the four modules is present and the dependency set is unchanged from the `BP-005` baseline (CON-204).
- AC-023: Given each feature alone, when `cargo check -p wirekit --no-default-features --features <module>` runs for `angi`, `google-lsa`, `sage-intacct`, and `adp`, then each compiles without pulling another module's dependencies.
- AC-024: Given the workspace, when `cargo fmt --all --check`, `cargo clippy -p wirekit --all-features -- -D warnings`, and `cargo doc -p wirekit --all-features --no-deps` run, then all pass while `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` remain in force (CON-201, CON-202).
- AC-025: Given a provider whose access is unconfirmed, when the client is constructed with a sandbox base-URL override, then every documented call path is reachable and returns typed values or typed errors without any live credential.

## Requirements

### Functional Requirements

#### Angi (BP-033)

- FR-001: `wirekit` MUST provide `angi::{AngiLead, AngiLeadParser, AngiImport}` behind an additive `angi` feature, and MUST NOT add an HTTP client for Angi because no public API exists.
- FR-002: `AngiLeadParser` MUST parse the documented Angi lead email templates into `AngiLead` and MUST NOT drop a lead silently; an unrecognised template or a missing required field MUST return a structured `ParseError` naming the offending field (CON-206).
- FR-003: `AngiImport` MUST import a CSV export through an explicit header mapping and MUST report one outcome per row - imported, skipped, or failed - with the 1-based row number, completing the import even when individual rows fail.
- FR-004: The module MUST compute a deterministic dedupe key: the Angi lead ID when present, otherwise the normalized phone, address, and received date, stable across whitespace and formatting differences.
- FR-005: Partner webhook verification MUST verify a shared-secret signature over the raw request bytes (CON-215) and MUST be exposed only as an optional capability; when no partner feed is provisioned the module MUST return an explicit not-supported error rather than pretend to poll.

#### Google LSA (BP-034)

- FR-006: `wirekit` MUST provide `google_lsa::GoogleLsaClient` behind an additive `google-lsa` feature, constructed from an account ID, a base URL, and an injected OAuth2 token source, reusing `wirekit::google_oauth` for refresh (CON-212).
- FR-007: The module MUST provide typed `Lead`, `Booking`, and `LeadStatus` models covering the catalogue's lead type, contact, job details, booking status, and attribution.
- FR-008: The client MUST expose list leads, get lead, update lead status, and report booking, and MUST NOT expose untyped JSON to the caller.
- FR-009: The module MUST map the Google error envelope into `WirekitError::Api` and MUST surface HTTP 403 as an explicit account-not-linked-or-unapproved variant, because that is the expected failure when access is unresolved.
- FR-010: The module MUST apply the shared retry and rate-limit contract: exponential backoff with jitter, three attempts by default, retry on 429 and 5xx only, honour `Retry-After`, and a per-provider token bucket (CON-208, CON-209).
- FR-011: The module MUST dedupe by lead ID and MUST make lead-status updates idempotent by lead, and MUST NOT assume a webhook: Pub/Sub or polling ingestion MUST be an explicit, account-specific configuration that returns a not-configured error when absent.

#### Sage Intacct (BP-035)

- FR-012: `wirekit` MUST provide `sage_intacct::SageIntacctClient` behind an additive `sage-intacct` feature, constructed from a sender ID, a password, a company ID, and an endpoint.
- FR-013: The module MUST provide typed `Customer`, `Invoice`, `JournalEntry`, and `ReportRequest` models and MUST scope v1 to create/update customer, create invoice, create journal entry, read by key, and standard report invocation - it MUST NOT attempt the full XML schema.
- FR-014: The module MUST parse the XML gateway result and MUST derive success from its `status` and `errno`, never from the HTTP status alone (CON-213).
- FR-015: The sender password MUST be held in the `wirekit` secret type, MUST be redacted in `Debug`, `Display`, tracing, and error payloads, and its type MUST NOT implement `Serialize` (CON-214).
- FR-016: The module MUST bound XML response size and nesting depth by configuration and MUST reject documents exceeding either bound with a typed error (CON-515).
- FR-017: Create operations MUST be idempotent by caller-supplied external key: the module queries before creating and returns the existing record when the key is already present.
- FR-018: The module MUST NOT retry business-rule errors, and because the classic gateway has no webhooks it MUST document polling or reconciliation as the caller's responsibility instead of inventing an event stream.

#### ADP (BP-035)

- FR-019: `wirekit` MUST provide `adp::AdpClient` behind an additive `adp` feature, authenticating with OAuth2 client credentials, caching the token until expiry, and refreshing it exactly once after a 401.
- FR-020: The module MUST provide typed `Employee`, `TimeCard`, `PayrollRun`, and `PayrollReport` models, and MUST scope v1 to list workers, create or update time cards, list payroll runs, and export a payroll report.
- FR-021: The module MUST map ADP's structured error array into `WirekitError::Api` and MUST preserve the correlation ID plus every returned code and message (CON-213).
- FR-022: The module MUST NOT blindly retry a payroll submission: after a timeout it MUST return an unknown-outcome state that requires reconciliation, and it MUST support a caller-supplied client reference so a resubmission is identifiable (CON-208).
- FR-023: The module MUST redact SSNs and pay rates in `Debug`, `Display`, tracing, and error payloads, and their types MUST NOT implement `Serialize` (CON-214).
- FR-024: Batch export MUST report a per-record outcome so that a partial failure lists the failed records by identifier and never drops hours silently.

#### Shared module contract

- FR-025: Every module MUST be an additive, default-off Cargo feature named `angi`, `google-lsa`, `sage-intacct`, and `adp`, MUST NOT disable another feature when enabled, and MUST be documented in the crate README (CON-204).
- FR-026: Every failure MUST map into `WirekitError`; the Angi `ParseError` MUST convert into it through `From`, preserving the offending field, and no module may introduce a second top-level error type (CON-207).
- FR-027: No module may add an application domain type, product noun, billing rule, or global mutable singleton; clients MUST be constructed and injected (CON-106, CON-109).
- FR-028: The four modules MUST live inside `wirekit`; no new crate may be added to the workspace for any of them (CON-103, CON-104).
- FR-029: Each module file MUST open with a `//!` comment stating its scope and non-goals, and every new public item MUST carry rustdoc (CON-202, CON-203).
- FR-030: Every module that talks to a live endpoint MUST accept a configurable base URL so it can be pointed at a sandbox; the default MUST be the documented production endpoint, and a non-HTTPS override MUST be rejected (CON-520).

### Non-Functional Requirements

- NFR-001: Credentials MUST be supplied by the caller at construction time; no key, token, password, or company ID may be embedded, defaulted, or logged (CON-511, CON-519).
- NFR-002: Tenant isolation MUST be structural: a client instance holds one tenant's credentials and MUST NOT share cached tokens, cached responses, or connection state across tenants (CON-512).
- NFR-003: Each module MUST use a per-provider token bucket with caller override and MUST emit a tracing metric on a rate-limit hit (CON-209, CON-514).
- NFR-004: Every untrusted input - email body, CSV row, JSON lead, XML result, ADP error array - MUST be validated and MUST produce a typed error rather than a panic, an unwrap, or a silent default (CON-515).
- NFR-005: Every outbound request MUST emit tracing events for start, finish with duration, retry, rate-limit hit, and error, and MUST NOT record secret or personal fields (CON-211, CON-214).
- NFR-006: Retries MUST be exponential with jitter and bounded to three attempts by default, retrying only 5xx, 429, and network errors, and honouring `Retry-After` (CON-208).
- NFR-007: Memory per call MUST stay bounded by configuration: the Intacct XML body ceiling, the Angi CSV row and file ceiling, and the LSA and ADP response ceiling MUST each be enforced before allocation grows without limit.
- NFR-008: The crate MUST keep `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]`, and no module may request an unsafe exception (CON-201, CON-202).
- NFR-009: The feature MUST build on Rust 1.85 with edition 2021, and each feature MUST compile independently under `--no-default-features` (CON-102, CON-204).
- NFR-010: The feature MUST add no runtime, no async runtime choice, and no new global state; blocking versus async MUST follow whatever `wirekit` core already exposes from `BP-005`.
- NFR-011: Security Baseline `CON-516` (NoSQL / injection protection) is **NOT APPLICABLE** with rationale: none of these four modules owns a database, cache, or query surface. If a caller persists the returned models, parameterised storage is the caller's implementation and out of scope here. Every other REQUIRED control in the baseline (CON-511, CON-512, CON-514, CON-515, CON-519, CON-520) is expanded above as NFR-001 through NFR-008.

## UI/UX & Data Requirements

- **UI/UX:** None. This repository ships no application UI, view code, or presentation component (CON-301). The modules return typed data; the consuming application renders it. No accessibility or interaction requirement applies.
- **Data captured and returned:**
  - `AngiLead`: source, Angi lead ID (optional), contact name, phone, email, address, service or job type, received-at, and the dedupe key.
  - `Lead` (LSA): lead ID, lead type, contact block, job details, and attribution fields.
  - `Booking` / `LeadStatus` (LSA): booking status, attribution, and the closed set of statuses documented in `catalogue/21_google_lsa.md`.
  - `Customer` / `Invoice` / `JournalEntry` (Intacct): the keys and line items needed to create and read back, with the external key supplied by the caller.
  - `Employee` / `TimeCard` / `PayrollRun` / `PayrollReport` (ADP): worker identifier, approved hours and earnings, payroll cycle status, and export outcome per record.
- **Data deliberately not stored:** no module persists anything. There is no database, no file write, and no local state beyond what the caller supplies; `CON-107` keeps quota and webhook state out of `wirekit`.

## Edge Cases & Error States

- EC-001: Angi email uses an unknown template -> structured `ParseError` naming the field; no lead emitted, no partial record.
- EC-002: Angi CSV has a missing required column -> import fails with a header error before any row is processed; no partial import is reported as complete.
- EC-003: Angi CSV row is malformed -> that row fails with its row number, remaining rows continue, and the summary counts the failure.
- EC-004: Angi webhook is called with no partner feed provisioned -> explicit not-supported error; no signature is "verified" optimistically.
- EC-005: Angi duplicate lead with no lead ID -> dedupe by normalized phone + address + date, not by insertion order.
- EC-006: LSA returns 403 for an unlinked account -> explicit account-not-approved error; the body is redacted.
- EC-007: LSA returns 429 with `Retry-After` longer than the configured timeout -> the call fails with a typed rate-limit error instead of sleeping past its deadline.
- EC-008: LSA lead status update replayed -> idempotent no-op; no second mutation.
- EC-009: Intacct returns HTTP 200 with `status: failure` -> error carrying `errno` and description; no success value.
- EC-010: Intacct response exceeds the byte ceiling or the depth ceiling -> typed bound error, no partial model.
- EC-011: Intacct create for an external key that already exists -> returns the existing record; no duplicate.
- EC-012: Intacct sender password appears in a configuration struct -> redacted in `Debug`; the type is not `Serialize`.
- EC-013: ADP token expires mid-batch -> one refresh, then the in-flight call retries once; a second 401 fails the call.
- EC-014: ADP payroll submission times out -> unknown-outcome state returned for reconciliation; no automatic resubmission.
- EC-015: ADP error array carries three errors -> all three survive mapping with their codes; none is collapsed into one message.
- EC-016: ADP batch export partially fails -> per-record outcomes; failed records are listed, successful ones are not resent.
- EC-017: Base URL override is `http://` -> rejected at construction with a typed error (CON-520).
- EC-018: Feature enabled twice through a dependency graph -> additive features compose; no compile error and no behaviour change.

## Out of Scope

- **BP-022 to BP-027** - already specified and in implementation as `m4-media-links` v1; not reproposed here.
- **BP-036** (`mixbus` VST3/CLAP plugin hosting) - conflicts with CON-201; requires a constitution amendment through `/sdrive:constitution`, not a feature spec.
- **BP-037** (Turborepo evaluation) - tooling, not a crate; needs a committed `Cargo.lock`, `[workspace.metadata].name`, and an npm `turbo`, and the repository has no JavaScript surface.
- `wirekit` core itself (`BP-005`): client, retry, rate limiter, tracing, and `google_oauth` are consumed from `feature/m1-first-publish`.
- All other provider modules (`catalogue/03`-`14`, `17`-`28`).
- Live provider certification, sandbox accounts, partner agreements, and any task that requires a real credential.
- Webhook ingestion, dedupe, replay protection, or forwarding for these providers - that belongs to `hookgate` (`BP-020`), not to a client module (CON-107).
- Quota reservation or ledger state (`BP-018`, `BP-019`).
- Persistence, caching to disk, reporting rollups, analytics, or dashboards.
- Application UI, product nouns, billing rules, and vendor workflow code (CON-106, CON-301).
- Publishing `wirekit` to crates.io from this feature branch.
- Test authoring and test execution (owned by `/sdrive:test`).

## Dependencies & Risks

### Dependencies

| Dependency | Type | Status | Notes |
|---|---|---|---|
| `wirekit` core (`BP-005`) | internal | **Not on `main`** | Present on `feature/m1-first-publish`; required before any task here can compile |
| `wirekit::google_oauth` | internal | Not on `main` | Reused for LSA token refresh (CON-212); not re-implemented |
| `wirekit` secret type + `WirekitError` | internal | Not on `main` | Reused by Intacct, ADP, and the Angi `From` conversion |
| Provider HTTP/JSON stack from `BP-005` | library | Not on `main` | Whatever `wirekit` core exposes; this feature adds none of it |
| XML parsing for Intacct | library | `[to be confirmed]` | No library is chosen here; the Gate 2 plan records the choice and its licence |
| CSV parsing for Angi imports | library | `[to be confirmed]` | Same rule: decided at Gate 2, not in the specification |
| Google Ads / LSA access | external | `needs discovery` | Gate on live use only; no requirement here depends on it |
| Sage Intacct partner agreement | external | `needs discovery` | Gate on live use only |
| ADP partner / enterprise access | external | `needs discovery` | Gate on live use only; product family (Workforce Now / Vantage) `[to be confirmed]` |
| Angi partner feed or webhook | external | `needs discovery` | Absent by default; the module must stay honest without it |

### Risks

- RISK-001: `m1-first-publish` never archives, so this feature is specified but unbuildable. Mitigation: implementation is explicitly blocked on `BP-005`; the spec states the dependency instead of duplicating `wirekit` core.
- RISK-002: Partner access is denied for LSA, ADP, or Intacct. Mitigation: keep the contract and sandbox path specified; mark the live gate in the backlog rather than faking an integration (`roadmap.md` G2).
- RISK-003: Angi changes its email templates and the parser silently degrades. Mitigation: FR-002 and EC-001 require a field-named failure and forbid silent drops.
- RISK-004: The Intacct XML schema is large enough to absorb unlimited effort. Mitigation: FR-013 fixes the v1 surface at three entities plus read-by-key and report invocation.
- RISK-005: XML parsing becomes a memory or CPU denial-of-service surface. Mitigation: FR-016 and NFR-007 bound bytes and depth before parsing completes.
- RISK-006: A payroll timeout is retried and double-pays. Mitigation: FR-022 and EC-014 require an unknown-outcome state plus a caller-supplied client reference.
- RISK-007: Personal or secret data reaches logs through a `Debug` derive. Mitigation: FR-015, FR-023, NFR-005, and AC-012/AC-019 require redaction and forbid `Serialize` on those types.
- RISK-008: Four modules turn `wirekit` into a framework. Mitigation: FR-025 through FR-028 keep them feature-flagged, error-consistent, and inside one crate with no store and no singleton (CON-103, CON-107).

## Product Plan Impact

- `BP-033` moves from `deferred` to in-scope for this feature. The catalogue states Angi has no public API, so "in scope" means a parser and importer, not a client. The backlog status entry stays as written until `/sdrive:archive` records the outcome.
- `BP-034` and `BP-035` keep their `needs discovery` status. This feature specifies the contract; access remains a live-use gate, and no task may depend on a real credential.
- `BP-036` and `BP-037` are untouched and remain deferred. `BP-036` needs a constitution amendment first (CON-201).
- `BP-022` to `BP-027` are untouched; they belong to `m4-media-links` v1.

## Sources

Repository only - no external research was available in this session.

- `catalogue/20_angi.md` - Angi has no public API; parser and import helper surface, `ParseError`, dedupe rule.
- `catalogue/21_google_lsa.md` - LSA base URL, OAuth2, `Lead`/`Booking`/`LeadStatus`, 403 meaning, notification uncertainty.
- `catalogue/15_sage_intacct.md` - XML gateway, sender ID + password + company ID, `status`/`errno`, entity scope, secret handling.
- `catalogue/16_adp.md` - OAuth2 client credentials, worker/time/payroll surface, correlation IDs, timeout reconciliation, sensitive-field redaction.
- `catalogue/01_wirekit.md` - shared error, retry, rate-limit, tracing, and feature-flag contract.
- `catalogue/00_provider_index.md` - provider inventory and status.
- `.sdrive/constitution.md` - CON-101 to CON-216, CON-301, CON-511 to CON-520, CON-601 to CON-614, CON-701 to CON-707.
- `.sdrive/product/backlog.md` - BP-033, BP-034, BP-035 entries and the deferred notes.
- `.sdrive/product/roadmap.md` - M5 goal, G2 decision gate, remaining-provider batch.
- `crates/dado/README.md` and `crates/dado/Cargo.toml` - in-house precedent for feature flags, MSRV, lints, and package metadata.

## Spec Update 2026-10-29 - Local Services Ads surface corrected

The published `localservices:v1` discovery document
(`https://localservices.googleapis.com/$discovery/rest?version=v1`) defines exactly
two resources - `accountReports` and `detailedLeadReports` - each with a single
`:search` method, and exactly one OAuth scope,
`https://www.googleapis.com/auth/adwords`. It defines **no** method to list or get a
lead by id, to update a lead's status, or to report a booking.

`FR-007`, `FR-008`, and `FR-011` described that non-existent surface. Under the
ID-preservation rule they are left in place and superseded by `FR-031` to `FR-033`
below. `US-002` is superseded by `US-006`.

### US-006: Read Local Services Ads performance without pretending to mutate it

As a developer integrating Local Services Ads, I want typed account and lead reports
over the documented search methods, so that I can build reporting on contracts that
actually exist.

**Acceptance Criteria:**

- AC-026: Given a manager customer id and an inclusive date range, when `account_reports` runs, then it issues `GET v1/accountReports:search` with `query=manager_customer_id:<id>` and the documented date parameters, and returns a typed `AccountReportsPage` carrying any next-page token.
- AC-027: Given the same inputs, when `detailed_lead_reports` runs, then it issues `GET v1/detailedLeadReports:search` and returns typed `DetailedLeadReport` rows with their message or phone detail intact.
- AC-028: Given the API answers HTTP 403, when either search is called, then the error carries the stable code `LOCALSERVICES_ACCOUNT_NOT_APPROVED` rather than a generic authentication failure.
- AC-029: Given neither a manager customer id nor an explicit query, when a search is attempted, then it fails with a typed configuration error before any request is sent.

### Functional Requirements

- FR-031: `google_lsa` MUST implement `accountReports:search` and `detailedLeadReports:search` with the documented query and date parameters, and MUST expose pagination through the response's next-page token.
- FR-032: `google_lsa` MUST NOT expose a lead-status update, a booking report, or a notification-ingestion surface, because the published API defines none; adding one MUST wait for Google to publish it.
- FR-033: `google_lsa` MUST send the OAuth2 bearer token from a caller-supplied `Secret` and MUST use the single documented scope `https://www.googleapis.com/auth/adwords`.

### Non-Functional Requirements

- NFR-012: `google_lsa` MUST never render the bearer token in `Debug`, tracing, or an error payload, and MUST NOT implement `Serialize` for it (CON-214).

### Edge Cases & Error States

- EC-019: HTTP 403 from localservices → `LOCALSERVICES_ACCOUNT_NOT_APPROVED`; the shared policy does not retry it.
- EC-020: A search response body that does not match the typed page → `WirekitError::Decode` naming the resource, never a partial page.
- EC-021: A caller-supplied sandbox base URL → used verbatim, and a non-HTTPS override is rejected at construction (CON-520).
