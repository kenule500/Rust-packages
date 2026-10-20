# 25 — Klarna

**Provider:** Klarna
**Package:** `wirekit` (module `wirekit::klarna`)
**Status:** provider design, not a crate
**Covers:** Consumer financing — checkout sessions, orders, captures, refunds
**Base URL:** `https://api.klarna.com`
**Auth:** Basic (merchant credentials)

## What it does

Klarna offers pay-over-time financing at the point of sale. GIA uses it on
estimates and invoices so a homeowner can say yes to a $12,000 roof without a
lump-sum payment. The contractor is paid upfront; Klarna carries the
installment risk.

The module wraps checkout sessions, orders, captures, and refunds.

## GIA use

- Feature 5.5 (Consumer Financing)
- Agent 18 (Invoicing) — payment options
- Agent 19 (Estimation) — financing awareness for out-of-budget replies
- Client Hub — payment choice

## Surface to build

| Type | Purpose |
|---|---|
| `KlarnaClient` | Merchant credentials, region |
| `CheckoutSession` | Create/read a session |
| `Order` | Order status and lines |
| `Capture` | Capture a payment |
| `Refund` | Full/partial refund |

Methods: create checkout session, get session, capture order, refund, handle
webhook, verify signature.

## Wirekit contract

- **Auth:** Basic merchant credentials; region-specific base URL.
- **Errors:** map Klarna's error envelope to `WirekitError::Api`; a declined
  application is a business outcome, not a retry.
- **Retry:** 429 and 5xx; do not retry a capture that may have succeeded —
  reconcile first.
- **Rate limits:** per-merchant; token bucket.
- **Webhooks:** verify the signature; dedupe by event ID; reconcile the order.
- **Idempotency:** use the GIA invoice ID as the order reference; query before
  create.

## GIA integration

Agent 18 offers Klarna alongside pay-in-full and Wisetack. The homeowner is
redirected to Klarna's hosted flow; GIA never sees card data. The order status
updates the invoice; a capture marks it paid. Declines route back to the
invoice with a clear message.

## Test plan

- Klarna sandbox credentials; create/capture/refund.
- Declined and cancelled flows.
- Webhook signature and replay.
- Currency and region handling.
- Reconciliation after a timeout.

## Open questions

1. **US availability.** Confirm Klarna's current US merchant offering and
   whether the product fits home improvement.
2. **Merchant onboarding.** Who owns the Klarna merchant account: GIA or each
   contractor? This changes the integration model.
3. **Disclosures.** Consumer financing has legal disclosure requirements;
   confirm the copy and APR display.
4. **Fees.** Who pays the merchant fee, and how is it reflected in the
   estimate?

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §23.
