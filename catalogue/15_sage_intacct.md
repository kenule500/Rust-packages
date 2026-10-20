# 15 — Sage Intacct

**Provider:** Sage Intacct
**Package:** `wirekit` (module `wirekit::sage_intacct`)
**Status:** provider design, not a crate
**Covers:** Accounting — customers, invoices, journal entries
**Base URL:** `https://api.intacct.com/ia/xml/xmlgw.phtml`
**Auth:** Custom — sender ID + password + company ID

## What it does

Sage Intacct is the accounting system for larger, multi-entity contractors.
It is XML-based, older than QuickBooks, and common in mid-market finance
departments. GIA uses it to sync customers, invoices, and journal entries.

The module wraps the XML gateway with typed request/response models and a
signature/auth layer. XML is the main implementation risk.

## GIA use

- Tool 9 (Accounting Sync)
- Agent 18 (Invoicing) for mid-market tenants
- Journal entries for job costing
- Report generation for the accountant

## Surface to build

| Type | Purpose |
|---|---|
| `SageIntacctClient` | Sender ID, password, company ID, endpoint |
| `Customer` | Create/update/read |
| `Invoice` | Create/read, line items |
| `JournalEntry` | Create/read, debits/credits |
| `ReportRequest` | Standard report invocation |

Methods: create/update customer, create invoice, create journal entry, run a
report, read by key.

## Wirekit contract

- **Auth:** sender ID + password + company ID; the password is a secret and
  must never appear in logs. Support rotation.
- **Errors:** the XML gateway returns a result with `status` and `errno`;
  parse it and map to `WirekitError::Api`. HTTP 200 can still be a failure.
- **Retry:** 429 and 5xx; do not retry a business-rule error without changes.
- **Rate limits:** per-company; token bucket; batch where the API allows.
- **Webhooks:** none in the classic XML gateway; poll or reconcile.
- **Idempotency:** use the GIA invoice ID as the external key; query before
  create.

## GIA integration

For a mid-market tenant, Agent 18 creates the invoice in GIA, then the Intacct
invoice. Job costing writes a journal entry. If Intacct is unavailable, the
GIA invoice still goes to the homeowner and the sync is queued. The accountant
sees a clear sync status.

## Test plan

- Intacct sandbox company; sender ID/password auth.
- XML fixtures for success, business error, and auth failure.
- Customer/invoice/journal create and read-back.
- Duplicate prevention by external key.
- Secret redaction in logs and errors.

## Open questions

1. **Partner program.** Intacct access may require a partner agreement;
   confirm before committing.
2. **XML complexity.** The gateway's XML schema is large; scope v1 to
   customers, invoices, and journal entries only.
3. **Multi-entity.** A tenant with multiple Intacct entities needs a mapping
   per workspace.
4. **Report API.** Report invocation may be brittle; confirm which reports
   are stable enough to depend on.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §13.
