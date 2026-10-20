# 27 — LeadPerfection

**Provider:** LeadPerfection
**Package:** `wirekit` (module `wirekit::leadperfection`)
**Status:** provider design, not a crate
**Covers:** Calendar booking
**Base URL:** `https://api.leadperfection.com`
**Auth:** API key

## What it does

LeadPerfection provides calendar availability and appointment booking. GIA
uses it when a contractor already runs scheduling in LeadPerfection and does
not want two calendars. Agent 1 checks availability and books the inspection;
the appointment syncs both ways.

The module wraps availability lookup, booking, and appointment updates.

## GIA use

- Agent 1 (Receptionist) — inspection booking
- Agent 3 (Lead Qualification) — availability-aware routing
- Agent 12 (Smart Dispatch) — appointment context
- Tenant settings — calendar connection

## Surface to build

| Type | Purpose |
|---|---|
| `LeadPerfectionClient` | API key and calendar ID |
| `Availability` | Slots by date range and service |
| `Appointment` | Book, reschedule, cancel |
| `AppointmentStatus` | Confirmed, completed, no-show |

Methods: check availability, book appointment, reschedule, cancel, get
appointment, handle webhook.

## Wirekit contract

- **Auth:** API key; per-tenant, not shared.
- **Errors:** map to `WirekitError::Api`; a slot conflict is a business error,
  not a retry.
- **Retry:** 429 and 5xx; do not retry a booking without checking whether it
  succeeded.
- **Rate limits:** plan-dependent; token bucket.
- **Webhooks:** verify the signature; dedupe by appointment ID.
- **Idempotency:** use the GIA job/lead ID as the booking reference; query
  before create.

## GIA integration

Agent 1 offers two slots from LeadPerfection availability, then books. If
LeadPerfection is down, GIA books into its native calendar and queues a sync;
it does not tell the homeowner there is no availability. Reschedules and
cancellations flow both ways.

## Test plan

- LeadPerfection sandbox/test calendar.
- Availability lookup across time zones and service types.
- Book/reschedule/cancel and conflict handling.
- Webhook signature and replay.
- Duplicate booking prevention.

## Open questions

1. **API access.** Confirm the API plan and whether booking write access is
   included.
2. **Two-way sync.** Which system is authoritative for a slot: GIA or
   LeadPerfection? Define the conflict rule.
3. **Time zones.** The homeowner, the technician, and the calendar may be in
   different zones; test the conversion.
4. **Fallback.** If LeadPerfection is unavailable, does GIA book natively and
   reconcile, or fail the booking?

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §25.
