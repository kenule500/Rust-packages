# 22 — PagerDuty

**Provider:** PagerDuty
**Package:** `wirekit` (module `wirekit::pagerduty`)
**Status:** provider design, not a crate
**Covers:** Incident management, alerts, on-call
**Base URL:** `https://events.pagerduty.com/v2`
**Auth:** API key (header) for REST; routing key for Events API v2

## What it does

PagerDuty is the escalation path for things a human must see now: safety
hazards, guardrail violations, repeated agent failures, and payment outages.
Agent 10 (Supervisor) and Agent 11 (Error Detection) trigger incidents; the
on-call human acknowledges and resolves.

The module wraps the Events API and the REST API with typed incident, alert,
and escalation-policy models.

## GIA use

- Tool 14 (Escalation & Alert)
- Agents 10 (Supervisor), 11 (Error Detection), 15 (Team Management)
- Safety hazards and emergency escalations
- Platform health and webhook failures
- On-call schedules for the contractor's team

## Surface to build

| Type | Purpose |
|---|---|
| `PagerDutyClient` | API key, routing key, base URLs |
| `Incident` | Trigger, acknowledge, resolve |
| `Alert` | Alert payload and status |
| `EscalationPolicy` | Policy and schedule references |

Methods: trigger event, acknowledge incident, resolve incident, list
incidents, get on-call, verify webhook signature.

## Wirekit contract

- **Auth:** REST uses an API key header; Events API v2 uses a routing key in
  the payload. Keep the routing key per workspace.
- **Errors:** Events API returns 202 Accepted; a 400 means a malformed event,
  not a retry. REST errors map to `WirekitError::Api`.
- **Retry:** retry 5xx and network errors; do not retry a 400; dedupe events
  with `dedup_key`.
- **Rate limits:** per-account; token bucket.
- **Webhooks:** verify the signature; dedupe by event ID.
- **Idempotency:** `dedup_key` is the idempotency mechanism; reuse it for the
  same incident.

## GIA integration

A safety hazard from Agent 20 or a guardrail violation from any agent creates
a PagerDuty incident with a `dedup_key` derived from the escalation ID. Agent
11 uses PagerDuty for repeated execution failures. The contractor's on-call
schedule, not GIA's, decides who is paged.

## Test plan

- PagerDuty test routing key; trigger/ack/resolve lifecycle.
- 202 vs 400 vs 429 behavior.
- Dedup: same key twice does not create two incidents.
- Webhook signature and replay.
- PII redaction: incident payloads should not contain homeowner PII.

## Open questions

1. **Who is on call?** The contractor's team or GIA support? Define the
   escalation policy per tier.
2. **Event vs REST.** Use Events API v2 for triggers and REST for management;
   confirm the split.
3. **PII.** Incident payloads are visible to responders; do not include
   homeowner names, addresses, or payment data.
4. **Cost.** PagerDuty is per-user; confirm which tenant roles need seats.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §20.
