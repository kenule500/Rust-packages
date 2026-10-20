# 16 — ADP

**Provider:** ADP
**Package:** `wirekit` (module `wirekit::adp`)
**Status:** provider design, not a crate
**Covers:** Payroll — employees, payroll runs, time cards
**Base URL:** `https://api.adp.com`
**Auth:** OAuth2 Client Credentials

## What it does

ADP is the payroll system for larger contractors. GIA's time tracking produces
approved hours; the ADP module exports employees, time cards, and payroll runs
so the office does not re-key them.

The module wraps ADP's worker, time, and payroll endpoints with typed models
and an OAuth2 client-credentials token manager. Payroll data is sensitive; the
client must be strict about scopes and logging.

## GIA use

- Feature 4.6 (Time Tracking & Payroll Integration)
- Payroll export for Enterprise tenants
- Employee sync from ADP to GIA
- Time-card export from GIA to ADP

## Surface to build

| Type | Purpose |
|---|---|
| `AdpClient` | OAuth2 client credentials, API endpoints |
| `Employee` | Worker record |
| `TimeCard` | Hours, earnings, approvals |
| `PayrollRun` | Payroll cycle and status |
| `PayrollReport` | Export/report |

Methods: list workers, create/update time cards, list payroll runs, export a
payroll report, refresh token.

## Wirekit contract

- **Auth:** OAuth2 client credentials; cache token until expiry; refresh on
  401 once.
- **Errors:** ADP returns structured error arrays; map to
  `WirekitError::Api`, preserve correlation IDs.
- **Retry:** 429 and 5xx; do not retry a payroll submission that may have
  succeeded — reconcile first.
- **Rate limits:** contract-dependent; token bucket.
- **Webhooks:** ADP may offer event subscriptions; verify before relying on
  them. Otherwise poll/reconcile.
- **Idempotency:** payroll submissions need a client reference; reconcile
  after a timeout rather than retrying blindly.

## GIA integration

After a pay period closes, the office approves timesheets in GIA and exports
them to ADP. The export is a batch, not a per-hour stream. A failed export
must be visible and retryable; it must never silently drop hours.

## Test plan

- ADP sandbox/partner environment; OAuth token.
- Worker and time-card fixtures.
- Batch export with partial failures.
- Timeout reconciliation: submission may have succeeded.
- Sensitive-field redaction (SSN, pay rates) in logs.

## Open questions

1. **Access.** ADP API access is often partner/enterprise-gated; confirm the
   program and certification requirements.
2. **Scope.** Which ADP products (Workforce Now, Vantage, etc.) are in scope?
   The API surface differs.
3. **Compliance.** Payroll data has retention and access rules; confirm the
   data model and audit requirements.
4. **Employee identity.** Match GIA users to ADP workers by email or employee
   ID; decide the source of truth.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §14.
