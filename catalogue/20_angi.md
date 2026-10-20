# 20 — Angi

**Provider:** Angi
**Package:** `wirekit` (module `wirekit::angi`)
**Status:** provider design, not a crate
**Covers:** Lead integration — limited public API
**Base URL:** none public
**Auth:** Custom, partner-specific

## What it does

Angi does not offer a full public API. Contractors receive leads by email,
dashboard, or a partner feed. The Angi module is therefore not a live client;
it is a set of parsing and import helpers.

This is an honest limitation, not a temporary gap. Do not promise a
real-time Angi integration until Angi provides one.

## GIA use

- Agent 13 (Marketing Automation) — lead import
- Agent 6 (Inbox) — create a conversation from an imported lead
- Lead source attribution

## Surface to build

| Type | Purpose |
|---|---|
| `AngiLead` | Normalized lead record |
| `AngiLeadParser` | Parse email/CSV into `AngiLead` |
| `AngiImport` | Manual/CSV import helper |

Methods: parse an email body, parse a CSV export, normalize a lead, verify a
partner webhook if one is provisioned.

## Wirekit contract

- **Auth:** partner-specific; there is no standard public auth.
- **Errors:** parser errors are validation errors, not API errors; return a
  structured `ParseError` with the offending field.
- **Retry:** not applicable to parsing; import jobs retry as a batch.
- **Rate limits:** not applicable; respect the partner feed's terms.
- **Webhooks:** only if the tenant has a partner-provisioned webhook; verify
  with the shared secret.
- **Idempotency:** dedupe by Angi lead ID, or by normalized phone + address +
  date if no ID exists.

## GIA integration

An imported Angi lead becomes a contact and a lead with `source = ANGI`. Agent
6 opens a conversation; Agent 3 scores it. The parser must handle the common
email templates and fail loudly on a format change, not silently drop leads.

## Test plan

- Email fixtures from several Angi templates.
- CSV import with missing columns and duplicate rows.
- Parser failure: unknown format, missing phone, malformed address.
- Dedupe and source attribution.
- No live API tests — there is no public API.

## Open questions

1. **Partner access.** Does the tenant have a partner feed or webhook? If not,
   this is a manual import feature.
2. **Terms.** Confirm Angi's terms allow parsing emails and storing leads in a
   third-party CRM.
3. **Parser maintenance.** Email templates change; decide who owns parser
   updates and how failures are alerted.
4. **ROI.** Angi leads are shared/sold; measure close rate before recommending
   the channel.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §18.
