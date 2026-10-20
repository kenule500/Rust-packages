# 14 — Jobber

**Provider:** Jobber
**Package:** `wirekit` (module `wirekit::jobber`)
**Status:** provider design, not a crate
**Covers:** CRM/accounting — clients, jobs, invoices, quotes
**Base URL:** `https://api.getjobber.com/api/graphql`
**Auth:** OAuth2 Authorization Code

## What it does

Jobber is both a competitor and an integration target. Many small contractors
already run their business on it; GIA should not force a rip-and-replace on
day one. The Jobber module lets GIA sync clients, jobs, quotes, and invoices
with an existing Jobber account.

The module wraps Jobber's GraphQL API with typed queries and mutations. GraphQL
means the client must own query documents, pagination, and error shapes.

## GIA use

- Tool 9 (Accounting Sync)
- Agent 18 (Invoicing) — invoice sync
- Migration path for tenants leaving Jobber
- Coexistence for tenants who keep Jobber for accounting

## Surface to build

| Type | Purpose |
|---|---|
| `JobberClient` | OAuth2 token, GraphQL endpoint |
| `Client` | Customer record |
| `Job` | Job record and status |
| `Invoice` | Invoice and line items |
| `Quote` | Quote and conversion |

Methods: query clients/jobs/invoices/quotes, create/update, paginate,
subscribe to webhooks, refresh token.

## Wirekit contract

- **Auth:** OAuth2 authorization code; refresh token rotation; store encrypted.
- **Errors:** GraphQL returns HTTP 200 with an `errors[]` array. Do not treat
  200 as success; map GraphQL errors to `WirekitError::Api`.
- **Retry:** 429 and 5xx; GraphQL partial errors are not retried blindly.
- **Rate limits:** Jobber uses a cost-based GraphQL limit; track the returned
  cost/throttle status and back off.
- **Webhooks:** verify the signature; dedupe by event ID; fetch the entity to
  reconcile.
- **Idempotency:** use a GIA ID in a custom field; query before create.

## GIA integration

For a tenant that keeps Jobber, Agent 18 creates the invoice in GIA and pushes
it to Jobber; payments sync back. For a tenant migrating off Jobber, the
module is an import path: clients, jobs, and history. Do not write GIA data
into a Jobber account the tenant does not own.

## Test plan

- Jobber developer account and OAuth consent.
- GraphQL query/mutation fixtures, including partial errors.
- Pagination and cost-based throttling.
- Webhook signature and replay.
- Duplicate prevention and migration import.

## Open questions

1. **Competitive conflict.** Jobber is a direct competitor. Confirm the API
   terms allow integration and that GIA does not depend on it for core
   workflows.
2. **GraphQL schema changes.** Jobber can change its schema; pin a version if
   available and add contract tests.
3. **Data ownership.** The tenant owns the Jobber account; GIA must not hold
   credentials after offboarding.
4. **Feature parity.** Which Jobber objects are in scope for v1: clients,
   jobs, invoices, quotes — or all four?

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §12.
