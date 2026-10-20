# 26 — Wisetack

**Provider:** Wisetack
**Package:** `wirekit` (module `wirekit::wisetack`)
**Status:** provider design, not a crate
**Covers:** Consumer financing — pre-qualification, loans, payments
**Base URL:** `https://api.wisetack.com`
**Auth:** Bearer token

## What it does

Wisetack provides long-term consumer loans for home improvement, often up to
120 months. It complements Klarna's shorter installments. GIA uses it when the
job is large and the homeowner needs a monthly payment rather than a
point-of-sale plan.

The module wraps pre-qualification, loan applications, status, and payment
events.

## GIA use

- Feature 5.5 (Consumer Financing)
- Agent 18 (Invoicing) — financing option
- Agent 19 (Estimation) — out-of-budget reply path
- Client Hub — apply and view status

## Surface to build

| Type | Purpose |
|---|---|
| `WisetackClient` | Bearer token, partner ID |
| `LoanApplication` | Application and applicant data |
| `Loan` | Status, terms, amount |
| `Payment` | Payment events and schedule |

Methods: pre-qualify, create application, get loan status, list payments,
handle webhook, verify signature.

## Wirekit contract

- **Auth:** Bearer token per partner; keep separate from Klarna credentials.
- **Errors:** map to `WirekitError::Api`; a declined application is not a
  retry.
- **Retry:** 429 and 5xx; do not retry an application submission without
  reconciliation.
- **Rate limits:** partner-dependent; token bucket.
- **Webhooks:** verify the signature; dedupe by event ID; reconcile the loan.
- **Idempotency:** use the GIA invoice/estimate ID as the external reference.

## GIA integration

Agent 19 offers Wisetack when the homeowner says the price is out of budget.
The homeowner completes the application in Wisetack's hosted flow; GIA stores
only the loan status and reference. An approved loan marks the estimate as
financing-ready; a funded loan updates the invoice.

## Test plan

- Wisetack sandbox/partner environment.
- Pre-qualification approved, declined, and pending.
- Application submission and status.
- Webhook signature and replay.
- Timeout reconciliation.

## Open questions

1. **Partner onboarding.** Wisetack requires a partner agreement and possibly
   state licensing; confirm the timeline.
2. **Who is the merchant?** GIA or the contractor? This changes disclosures,
   fees, and support.
3. **Disclosures.** Financing offers require APR and term disclosures; confirm
   the copy with counsel.
4. **Coverage.** Confirm which states and loan sizes are supported.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §24.
