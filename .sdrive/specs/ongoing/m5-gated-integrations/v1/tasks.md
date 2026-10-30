---
title: M5 Gated Integrations - execution tasks
status: backlog
version: v1
created: 2026-10-29
updated: 2026-10-29
---

# M5 Gated Integrations - Execution Tasks

Production code only. No task here writes, adds, updates, or runs a test; test
authoring and execution belong to `/sdrive:test`. Verification tasks are limited
to formatter, linter, documentation, and feature-combination checks.

## P0 - Preconditions

Depends on: none.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T0.1 | Confirm `wirekit` core exists on the base branch | FR-028, NFR-010 | `crates/wirekit/Cargo.toml` exists with `WirekitClient`, `WirekitError`, retry, rate limit, tracing, `google_oauth`; if absent, stop and report the feature as blocked |
| T0.2 | Confirm the XML and CSV dependency choices | FR-025 | `quick-xml` and `csv` are recorded with a pinned version and a confirmed `MIT OR Apache-2.0`-compatible licence in the apply notes |

## P1 - Feature scaffolding

Depends on: P0.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T1.1 | Add `angi`, `google-lsa`, `sage-intacct`, `adp` features to `crates/wirekit/Cargo.toml` | FR-025 | Four additive, default-off features exist; none enables another; optional deps are attached with `dep:` |
| T1.2 | Create the four module files and register them in `lib.rs` | FR-029, FR-028 | `src/angi.rs`, `src/google_lsa.rs`, `src/sage_intacct.rs`, `src/adp.rs` exist behind `#[cfg(feature = ...)]`, each opening with a `//!` scope-and-non-goals comment |
| T1.3 | Document the four features in the crate README | FR-025 | The README lists each feature, what it adds, and whether it needs network access |

## P2 - `angi` module (BP-033)

Depends on: P1. No network access by design.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T2.1 | Define `AngiLead` and the dedupe key | FR-004 | The model carries the catalogue fields and `dedupe_key()` returns the lead ID when present, otherwise a normalized phone + address + date key |
| T2.2 | Define `ParseError` and its conversion | FR-002, FR-026 | `ParseError` exposes the offending field and reason; `From<ParseError> for WirekitError` preserves the field |
| T2.3 | Implement the email template parser | FR-002 | Documented templates produce an `AngiLead`; an unknown template or missing required field returns `ParseError`, never a partial lead |
| T2.4 | Implement CSV import with per-row outcomes | FR-003 | `AngiImport` takes an explicit header mapping and yields one `AngiImportOutcome` per row with a 1-based row number; a missing column fails before any row is processed |
| T2.5 | Implement optional partner webhook verification | FR-005 | Signature verification reads the raw request bytes only (CON-215); without a provisioned feed the call returns an explicit not-supported error |
| T2.6 | Confirm no HTTP client was added for Angi | FR-001 | The module compiles with `--no-default-features --features angi` and references no provider URL or HTTP client |

## P3 - `google_lsa` module (BP-034)

Depends on: P1.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T3.1 | Implement `GoogleLsaClient` construction | FR-006 | The client takes an account ID, a base URL, and an injected token source, and reuses `wirekit::google_oauth` for refresh (CON-212) |
| T3.2 | Define the LSA models | FR-007 | `Lead`, `Booking`, and `LeadStatus` are typed and documented; no raw JSON reaches the caller |
| T3.3 | Implement list, get, update status, and report booking | FR-008 | All four calls return typed values; provider paths confirmed from documentation, not assumed |
| T3.4 | Map the Google error envelope | FR-009, FR-026 | Errors map into `WirekitError::Api`; HTTP 403 becomes the explicit account-not-linked-or-unapproved variant with a redacted body |
| T3.5 | Wire retry and the rate-limit bucket | FR-010, NFR-003 | The core retry policy applies (3 attempts, 5xx/429/network, `Retry-After` honoured) and a per-provider bucket is used |
| T3.6 | Make status updates idempotent and notifications explicit | FR-011 | A replayed status update produces no second mutation; a missing Pub/Sub or webhook configuration returns a not-configured error |

## P4 - `sage_intacct` module (BP-035)

Depends on: P1.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T4.1 | Implement `SageIntacctClient` and its configuration | FR-012, FR-015 | Sender ID, company ID, endpoint, and a password held in the `wirekit` secret type; the password is redacted in `Debug` and the type does not implement `Serialize` |
| T4.2 | Define the Intacct request models | FR-013 | `Customer`, `Invoice`, `JournalEntry`, and `ReportRequest` exist and the v1 surface is limited to create/update customer, create invoice, create journal entry, read by key, and report invocation |
| T4.3 | Parse the gateway result instead of the HTTP status | FR-014 | Success is derived from `status` and `errno`; HTTP 200 with a failure status returns an error carrying both |
| T4.4 | Enforce XML input bounds | FR-016, NFR-007 | Byte ceiling and nesting depth are configuration-driven and a document exceeding either returns a typed bound error with no partial model |
| T4.5 | Make creates idempotent by external key | FR-017 | A create whose external key already exists returns the existing record instead of creating a duplicate |
| T4.6 | Classify business errors as non-retryable | FR-018, NFR-006 | A business-rule error is not resent unchanged and emits no retry event; polling or reconciliation is documented as the caller's responsibility |

## P5 - `adp` module (BP-035)

Depends on: P1.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T5.1 | Implement `AdpClient` with a cached client-credentials token | FR-019 | The token is cached until expiry and refreshed exactly once after a 401 before the call fails |
| T5.2 | Define the ADP models | FR-020 | `Employee`, `TimeCard`, `PayrollRun`, and `PayrollReport` exist and the v1 surface is limited to list workers, create or update time cards, list payroll runs, and export a payroll report |
| T5.3 | Map the ADP error array | FR-021, FR-026 | Every returned code and message survives mapping and the correlation ID is preserved |
| T5.4 | Return an unknown outcome for a payroll timeout | FR-022 | A timed-out submission yields `SubmissionOutcome::Unknown` with the caller's client reference; no automatic resubmission is performed |
| T5.5 | Redact sensitive payroll fields | FR-023 | SSN and pay rates are redacted in `Debug`, `Display`, tracing, and errors; their types do not implement `Serialize` |
| T5.6 | Report batch outcomes per record | FR-024 | A partial export lists failed records by identifier and reports the successes; no hour is dropped silently |

## P6 - Cross-cutting contract

Depends on: P2, P3, P4, P5.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T6.1 | Keep tenant state on the client instance | NFR-002, FR-027 | No module-level cache, static, or singleton exists; two clients for two tenants share no credential, token, or response |
| T6.2 | Trace requests without leaking values | NFR-005 | Request start, finish with duration, retry, rate-limit hit, and error are traced; no secret or personal field is recorded |
| T6.3 | Take every credential from the caller | NFR-001 | No key, token, password, or company ID is embedded, defaulted, or logged; nothing is read from a `.env` file (CON-519, CON-706) |
| T6.4 | Enforce HTTPS and the sandbox override | FR-030 | Every live module accepts a base URL, defaults to the documented production URL, and rejects a non-HTTPS override at construction |
| T6.5 | Remove panics from untrusted input paths | NFR-004 | Email, CSV, JSON, XML, and ADP error payloads each produce a typed error; no `unwrap`, `expect`, or slice indexing on parsed data |

## P7 - Verification and documentation

Depends on: P6.

| ID | Task | Requirements | Done when |
|---|---|---|---|
| T7.1 | Document each module's public surface | FR-029 | Every public item has rustdoc and each module has a runnable documentation example |
| T7.2 | Run the formatter check | NFR-008 | `cargo fmt --all --check` passes |
| T7.3 | Run the linter | NFR-008 | `cargo clippy -p wirekit --all-features -- -D warnings` passes |
| T7.4 | Build the documentation | FR-029 | `cargo doc -p wirekit --all-features --no-deps` passes with `RUSTDOCFLAGS="-D warnings"` |
| T7.5 | Compile every feature combination | FR-025, NFR-009 | `--no-default-features` plus one `--features <module>` per module all compile on Rust 1.85, and enabling one module never enables another |
| T7.6 | Run the governance validator | NFR-008 | `node .sdrive/scripts/validate-governance.js` reports every artifact valid |

## Notes

- **Blocked start:** if `T0.1` fails, `/sdrive:apply` must not begin. This feature
  is specified against `wirekit` core, which lives on `feature/m1-first-publish`
  and is not on `main`.
- **No credential tasks:** no task requires partner access, a sandbox account, or
  a live endpoint. `T3.3`, `T4.2`, and `T5.2` confirm provider paths from
  documentation; they do not call the provider.
- **No test tasks:** tests are authored and executed by `/sdrive:test`; the
  acceptance criteria are linked through `traceability.json` and the Test Agent
  fills the test IDs.

## Progress

| Task | State | Evidence |
|---|---|---|
| T0.1 | done | `crates/wirekit` present on the branch after `main` was merged in (`dda3cd8`) |
| T0.2 | done | `quick-xml` 0.42.0 (MIT) and `csv` 1.4.0 (Unlicense/MIT) verified on crates.io and pinned |
| T1.1 | done | `angi`, `google-lsa`, `sage-intacct`, `adp` features added, additive and default-off |
| T2.1 | done | `AngiLead` plus `dedupe_key()`: lead ID wins, else normalized phone + address + date |
| T2.2 | done | `ParseError` names the field and converts into `WirekitError` through `From` |
| T2.3 | done | `parse_email` recognizes the documented labels and fails on an unknown template |
| T2.4 | done | `import_csv` takes a `ColumnMap`, reports one outcome per row with its line number |
| T2.5 | done | `AngiWebhook::verify` checks an HMAC-SHA256 over the raw bytes in constant time |
| T2.6 | done | `cargo check -p wirekit --no-default-features --features angi` passes; no HTTP client is referenced |

Verified at this point: `cargo fmt --all --check`, `cargo clippy -p wirekit
--all-features -- -D warnings`, and `cargo doc -p wirekit --all-features
--no-deps` with `RUSTDOCFLAGS="-D warnings"` all pass, as does
`cargo check --no-default-features` for the baseline and for each of the four
features.

Still open: T1.2 (the `adp` module), T1.3, P6, and P7.

### Update after T3 and T4

| Task | State | Evidence |
|---|---|---|
| T3.1 | done | `GoogleLsaClient` built from a pooled client plus an OAuth2 token in `Secret` |
| T3.2 | done | `AccountReport`, `DetailedLeadReport`, `MessageLead`, `PhoneLead`, `SearchPage` |
| T3.3 | **not done — superseded** | the published `localservices:v1` document has no list/get/update-lead or report-booking method; only `GET v1/accountReports:search` and `GET v1/detailedLeadReports:search` exist, and both are implemented |
| T4.1 | done | `SageIntacctConfig` with the sender password in `Secret`, redacted in `Debug` |
| T4.3 | done | `parse_response` reads `status`, `errorno`, `description`, `key` and ignores the HTTP status |
| T4.4 | done | response bytes bounded by `max_response_bytes`, nesting bounded by `DEFAULT_MAX_DEPTH` |
| T4.5 | done | `create` sends the caller's key as `<uniqueid>`, the documented idempotence control |
| T4.2 | **partial** | Intacct exposes a documented `call` for any function body; the named `Customer`/`Invoice`/`JournalEntry` models are not built yet |
| T4.6 | **partial** | business errors surface as `WirekitError::Api` and are not auto-retried by the shared policy, but no explicit non-retryable classification is sent |

### ADP (P5) deferred

BP-035's ADP half is out of m5 v1. `developers.adp.com` serves a JavaScript
shell to every URL, so no token endpoint or resource path could be verified, and
inventing `/core/v1/workers` would put a fabricated route into a published crate.
The module returns to `needs discovery` and lands when a real OpenAPI or
API-Explorer definition can be supplied.
