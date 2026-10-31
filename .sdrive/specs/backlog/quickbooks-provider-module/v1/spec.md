---
title: QuickBooks provider module
status: backlog
version: v1
created: 2026-10-31
updated: 2026-10-31
---

# QuickBooks provider module

## Overview

- **Problem Statement**: QuickBooks Online is the accounting system of record for most consumers of this ecosystem, but `wirekit` has no QuickBooks module, so every integration re-implements OAuth2 token handling, realm-scoped routing, Intuit `Fault` parsing, and webhook signature verification. Getting refresh or idempotency wrong produces duplicate invoices and broken token chains in a system of record.
- **Proposed Solution**: Add a `quickbooks` feature-flagged module to `wirekit` exposing `QuickBooksClient`, `Customer`, `Invoice`, `Payment`, `Account`, and `Item`, per `catalogue/13_quickbooks.md`. It covers create/update/read, invoice lines, void and send, payment recording, account and item listing, OAuth2 token refresh, and `intuit-signature` webhook verification, reusing the crate's client, retry, rate-limit, secret, and error types.
- **Target Audience**: Rust developers syncing accounting data, including the invoicing and accounting agent in a consuming application.
- **Assumptions**:
  - Intuit app approval, sandbox credentials, and the realm ID are `[to be confirmed]`; the module assumes the caller already holds approved OAuth2 credentials.
  - Base URL is `https://quickbooks.api.intuit.com/v3/company/{realmId}` with a `minorversion` query parameter.
  - Access tokens live about 60 minutes; refresh tokens rotate and must be persisted by the caller.
  - No external research was performed beyond the repository catalogue and constitution (CON-704).

## User Stories

### US-001: Connect and refresh OAuth2 tokens
As a developer, I want the module to manage the token lifecycle, so that sync jobs do not fail at expiry.

**Acceptance Criteria:**
- AC-001: Given a client ID, client secret, refresh token, and realm ID, when the client is built, then credentials are held in `Secret` and `Debug` renders no plaintext.
- AC-002: Given an access token that is expired or within the refresh window, when a request is made, then the module refreshes once, applies the new token, and retries the request exactly once.
- AC-003: Given a refresh that returns a new refresh token, when it succeeds, then the caller receives the rotated refresh token so it can persist it, and a 401 that persists after one refresh fails with an authentication error rather than looping.

### US-002: Sync customers
As a developer, I want typed customer sync, so that a customer created in the product exists in accounting.

**Acceptance Criteria:**
- AC-004: Given a customer payload, when `create` is called, then a POST to the realm's `customer` resource sends the display name and optional contact fields and returns the created entity with its QuickBooks id.
- AC-005: Given an existing customer id and changed fields, when `update` is called, then the request carries `Id` and `SyncToken` and returns the updated entity with an incremented sync token.

### US-003: Create, read, void, and send invoices
As a developer, I want invoices with lines, so that completed work becomes a billable document.

**Acceptance Criteria:**
- AC-006: Given a customer reference and at least one line, when `create_invoice` is called, then the request carries the customer reference and ordered lines with amount, description, and item reference.
- AC-007: Given an existing invoice, when `void_invoice` is called, then the request uses the documented void operation and the returned invoice reports a voided balance.
- AC-008: Given an existing invoice, when `send_invoice` is called, then the request targets the send operation and the response records the delivery.

### US-004: Record payments
As a developer, I want payments linked to invoices, so that the accounting record balances.

**Acceptance Criteria:**
- AC-009: Given an invoice id and an amount, when `record_payment` is called, then the payment references the invoice and the amount is expressed in the entity's linked transaction.
- AC-010: Given a payment over the invoice balance, when it is recorded, then the caller receives a typed validation error from the provider rather than a silent partial success.

### US-005: Read accounts and items
As a developer, I want reference data, so that invoice lines point at real accounts and items.

**Acceptance Criteria:**
- AC-011: Given a realm, when `list_accounts` or `list_items` is called, then the module returns typed entries carrying id, name, and the fields the catalogue names, using the query endpoint with a bounded result size.

### US-006: Verify webhooks
As a developer, I want signed change notifications, so that only Intuit can trigger a resync.

**Acceptance Criteria:**
- AC-012: Given raw bytes, the verifier token, and a correct `intuit-signature`, when verification runs, then the HMAC comparison is constant time and returns the parsed notification.
- AC-013: Given a wrong or missing signature, when verification runs, then it fails closed with an authentication error and nothing is parsed; a repeated event id is reported as a duplicate.

### US-007: Fail predictably
As a developer, I want Intuit faults, retries, and duplicate protection handled consistently.

**Acceptance Criteria:**
- AC-014: Given an Intuit `Fault` with `Error[]` entries, when a call fails, then the error is `WirekitError::Api` and exposes every Intuit error code and message.
- AC-015: Given a 429 with `Retry-After`, when the call is retried, then the retry policy honours the header and validation faults are never retried.
- AC-016: Given a caller-supplied idempotency key, when a create is retried, then the module checks for an existing entity by that key before creating a duplicate.

## Requirements

### Functional Requirements
- FR-001: The crate MUST expose an additive, default-off `quickbooks` feature that enables no other provider (CON-104, AC-001).
- FR-002: `QuickBooksClient` MUST hold client credentials and refresh tokens in `Secret`, reject a non-HTTPS base URL, and read no environment variables (CON-519, CON-520, AC-001).
- FR-003: The module MUST provide `QuickBooksClient`, `Customer`, `Invoice`, `Payment`, `Account`, and `Item` as named in `catalogue/13_quickbooks.md` (AC-004, AC-006, AC-009, AC-011).
- FR-004: Token refresh MUST happen before expiry and at most once per request, surfacing the rotated refresh token (AC-002, AC-003).
- FR-005: Customer create and update MUST be supported with `Id` and `SyncToken` on update (AC-004, AC-005).
- FR-006: Invoice create with ordered lines, void, and send MUST be supported (AC-006, AC-007, AC-008).
- FR-007: Payments MUST reference an invoice and reject over-balance amounts with a typed error (AC-009, AC-010).
- FR-008: Account and item listing MUST use the query endpoint with a bounded result size (AC-011).
- FR-009: Webhook verification MUST use HMAC over raw bytes with constant-time comparison and MUST fail closed (AC-012, AC-013).
- FR-010: Duplicate webhook events MUST be detectable by event id (AC-013).
- FR-011: Intuit `Fault` bodies MUST map to `WirekitError::Api` preserving every code and message (AC-014).
- FR-012: Retries MUST honour `Retry-After` for 429 and 5xx and MUST NOT retry validation faults; creates MUST support an idempotency check (AC-015, AC-016).

### Non-Functional Requirements
- NFR-001: Client credentials, access tokens, refresh tokens, and the webhook verifier token MUST be held in `Secret` and MUST render redacted (CON-511, CON-519, AC-001).
- NFR-002: Every request MUST be realm-scoped by the configured realm id; the module MUST NOT accept a per-call realm that contradicts the configured one (CON-512, AC-004).
- NFR-003: Provider responses and webhook bodies MUST be size-bounded and typed before use (CON-515, AC-010, AC-012).
- NFR-004: All traffic MUST use HTTPS and a cleartext base URL MUST be rejected at construction (CON-520, AC-001).
- NFR-005: The rotated refresh token MUST be surfaced so the caller can persist it before the next request; losing it MUST NOT silently continue with a stale token (AC-003).
- NFR-006: The module MUST compile under `#![forbid(unsafe_code)]` and `#![deny(missing_docs)]` (CON-201, CON-202).

### Security Baseline
| Control | Status | Decision for this feature |
|---|---|---|
| CON-511 Authentication | REQUIRED | OAuth2 refresh-before-expiry, refresh-once-on-401, rotated refresh token surfaced (AC-002, AC-003). |
| CON-512 Authorization | REQUIRED | Realm id is fixed in the client, so a call cannot cross into another realm (AC-004). |
| CON-513 Security Headers | NOT APPLICABLE | The crate is an HTTP client and renders no responses. |
| CON-514 Rate Limiting | REQUIRED | Existing token bucket plus `Retry-After` handling per realm (AC-015). |
| CON-515 Input Validation | REQUIRED | Typed entities, bounded bodies, and provider validation faults surfaced as typed errors (AC-010, AC-014). |
| CON-516 Injection Protection | NOT APPLICABLE | No SQL or command backend in this module. |
| CON-517 XSS / CON-518 CSRF | NOT APPLICABLE | No markup and no browser session. |
| CON-519 Environment Security | REQUIRED | No environment reads; placeholders only in docs (AC-001). |
| CON-520 HTTPS / HSTS | REQUIRED | Non-HTTPS base URLs rejected (AC-001). |

## Edge Cases & Error States
- EC-001: Refresh succeeds but the rotated refresh token is not persisted by the caller -> the next run fails authentication rather than reusing a stale token.
- EC-002: Two creates with the same idempotency key race -> the second either finds the existing entity or surfaces the provider's duplicate error.
- EC-003: Webhook replays the same event id -> reported as a duplicate instead of resyncing twice.
- EC-004: Invoice line references an item that does not exist in the realm -> provider validation fault surfaced as a typed error naming the item.

## Out of Scope
- OAuth2 authorization-code browser flow and token persistence (the caller or `platform-core` owns storage).
- Payroll, tax filing, and other Intuit products.
- Bulk/batch endpoints and CDC polling.
- Accounting domain rules such as revenue recognition (CON-106).

## Dependencies & Risks
- **Dependencies**: `reqwest`/`rustls`, `sha2`, `hmac`, `serde`, `serde_json`, and `base64` are already compiled into `wirekit`. No new dependency.
- **RISK-001**: Intuit app approval, sandbox credentials, and a realm are unconfirmed. Mitigation: verify against a local mock of the documented shapes, as `m5-gated-integrations` did, and keep the assumption marked `[to be confirmed]`.
- **RISK-002**: Refresh-token rotation is lossy if the caller does not persist the new value. Mitigation: NFR-005 plus an explicit result type carrying the rotated token.
- **RISK-003**: QuickBooks entity schemas change with `minorversion`. Mitigation: one version constant, documented and asserted in tests.


