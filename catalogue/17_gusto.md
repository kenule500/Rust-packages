# 17 — Gusto

**Provider:** Gusto
**Package:** `wirekit` (module `wirekit::gusto`)
**Status:** provider design, not a crate
**Covers:** Payroll — employees, payroll, time off
**Base URL:** `https://api.gusto.com`
**Auth:** OAuth2 Authorization Code

## What it does

Gusto is the payroll system for small and mid-sized contractors. It is simpler
than ADP and more common in the SMB segment GIA targets. The module syncs
employees, payroll runs, and time-off requests.

The module wraps Gusto's REST API with typed models and an OAuth2 token
manager. It is the default payroll integration for tenants who do not already
have ADP.

## GIA use

- Feature 4.6 (Time Tracking & Payroll Integration)
- Employee onboarding sync
- Payroll export from approved timesheets
- Time-off visibility for dispatch

## Surface to build

| Type | Purpose |
|---|---|
| `GustoClient` | OAuth2 token, company ID |
| `Employee` | Employee record and status |
| `Payroll` | Payroll run and totals |
| `TimeOffRequest` | Request and approval status |
| `Compensation` | Rate and pay type |

Methods: list/create employees, list payrolls, create a payroll (where
allowed), list time-off requests, refresh token.

## Wirekit contract

- **Auth:** OAuth2 authorization code; refresh token rotation; store
  encrypted.
- **Errors:** map Gusto's error envelope to `WirekitError::Api`; distinguish
  validation from auth.
- **Retry:** 429 and 5xx; do not retry a payroll submission without
  reconciliation.
- **Rate limits:** per-app; token bucket.
- **Webhooks:** verify the signature; dedupe by event ID.
- **Idempotency:** use a client key for payroll and employee creation; query
  before create.

## GIA integration

For a Gusto tenant, the office approves hours in GIA, then exports the pay
period to Gusto. Employee records sync both ways, with GIA as the source for
job assignments and Gusto as the source for pay rate. Time-off requests flow
into dispatch availability.

## Test plan

- Gusto demo company and OAuth consent.
- Employee create/update and termination.
- Payroll list and export; partial failure.
- Time-off request and approval.
- Webhook signature and replay.
- Sensitive-field redaction.

## Open questions

1. **Payroll creation.** Does the API allow creating a payroll, or only
   reading? Confirm before promising a one-click export.
2. **Contractor vs employee.** Gusto handles both; the GIA model must
   distinguish them for payroll and dispatch.
3. **State tax accounts.** Payroll exports can fail on missing registrations;
   the error must be actionable.
4. **Data retention.** Payroll data has legal retention requirements; confirm
   the GIA retention policy.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §15.
