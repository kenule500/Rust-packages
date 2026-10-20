# 13 — QuickBooks

**Provider:** QuickBooks Online
**Package:** `wirekit` (module `wirekit::quickbooks`)
**Status:** provider design, not a crate
**Covers:** Accounting — customers, invoices, payments, accounts, items
**Base URL:** `https://quickbooks.api.intuit.com/v3`
**Auth:** OAuth2 Authorization Code

## What it does

QuickBooks is the accounting system of record for most GIA tenants. GIA does
not replace it; it pushes invoices, payments, customers, and items into it and
reads back the chart of accounts. Agent 18 creates the invoice in GIA and syncs
it; the accountant keeps working in QuickBooks.

The module wraps the QuickBooks Online API with typed entities, an OAuth2
token manager, and webhook verification.

## GIA use

- Tool 9 (Accounting Sync)
- Agent 18 (Invoicing), Accounting AI
- Two-way customer sync
- Invoice and payment sync
- Account and item mapping for job costing

## Surface to build

| Type | Purpose |
|---|---|
| `QuickBooksClient` | Realm ID, OAuth2 tokens, minor version |
| `Customer` | Create/update/read |
| `Invoice` | Create/update/read, line items |
| `Payment` | Record against an invoice |
| `Account` | Chart of accounts |
| `Item` | Products/services for line items |

Methods: query, create, update, void, send invoice, record payment, list
accounts/items, refresh token, verify webhook signature.

## Wirekit contract

- **Auth:** OAuth2 authorization code; store the refresh token encrypted;
  refresh before expiry; on 401, refresh once and retry the request.
- **Errors:** Intuit returns a `Fault` object with `Error[]`; map to
  `WirekitError::Api` and preserve the Intuit error codes.
- **Retry:** 429 and 5xx; respect `Retry-After`; do not retry validation
  faults.
- **Rate limits:** per-realm; token bucket; back off on 429.
- **Webhooks:** verify the `intuit-signature` HMAC; dedupe by event ID;
  reconcile the changed entity by ID.
- **Idempotency:** use the GIA invoice ID in a custom field or memo; check for
  an existing invoice before creating a duplicate.

## GIA integration

When a job is marked complete, Agent 18 creates the GIA invoice, then the
QuickBooks invoice. If QuickBooks is down, the GIA invoice is still sent to the
homeowner and the sync is queued. Payment webhooks update both sides. A failed
sync must surface in the dashboard, not disappear.

## Test plan

- QuickBooks sandbox company; OAuth consent and token refresh.
- Create/update/void invoice; partial payment; refund.
- Duplicate prevention: same GIA invoice syncs twice.
- Webhook signature: valid, tampered, replayed.
- 429/5xx retry and 401 refresh-once.
- Field mapping: tax, class, department, item.

## Open questions

1. **Sync direction.** Which fields are GIA-authoritative vs QuickBooks-
   authoritative? Customer names and addresses are a common conflict.
2. **Minor version.** Pin the QuickBooks minor version and review it on a
   schedule; silent changes break mappings.
3. **Multi-entity.** A tenant with several QuickBooks companies needs a
   mapping per workspace.
4. **Accountant access.** The accountant may need read-only access; confirm
   the role model and audit trail.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §11.
