# 03 — Twilio

**Provider:** Twilio
**Package:** `wirekit` (module `wirekit::twilio`)
**Status:** provider design, not a crate
**Covers:** SMS, Voice, MFA, Lookup
**Base URL:** `https://api.twilio.com/2010-04-01`
**Auth:** Basic (Account SID + Auth Token) or API Key + Secret

## What it does

Twilio is the primary telephony and messaging transport for GIA. It carries
inbound calls, outbound SMS, WhatsApp, missed-call recovery, and MFA. It is
the front door for Tier 3: if Twilio fails, the homeowner cannot reach the
contractor.

This module wraps Twilio's REST API in typed requests and responses and maps
its errors into the shared `WirekitError` model. It does not re-implement
Twilio's messaging logic; it makes the calls consistent with every other
provider.

## GIA use

- Tool 1 (Telephony & Voice), Tool 2 (Omnichannel Messaging)
- Agents 1 (Receptionist), 2 (Missed Call), 4 (Follow-Up), 6 (Inbox),
  8 (Review & Referral), 13 (Marketing Automation), 14 (Social Media)
- Compliance: every outbound SMS passes Tool 13 before Twilio is called.

## Surface to build

| Type | Purpose |
|---|---|
| `TwilioClient` | Authenticated client |
| `TwilioConfig` | Account SID, token/API key, messaging service SID |
| `SendSmsRequest` / `SendSmsResponse` | Outbound SMS |
| `SendWhatsAppRequest` | WhatsApp via Twilio |
| `InitiateCallRequest` / `CallStatus` | Voice calls and status |
| `MfaChallenge` / `MfaVerification` | SMS/voice MFA |
| `LookupRequest` / `LookupResult` | Number validation |

Methods: send SMS, send WhatsApp, initiate a call with TwiML, fetch call
status, look up a number, start/verify MFA, verify webhook signatures.

## Wirekit contract

- **Errors:** map Twilio's `code`, `message`, and HTTP status into
  `WirekitError::Api { status, code, message }`; auth failures into
  `WirekitError::Auth`.
- **Retry:** retry on 429, 5xx, and network errors; respect `Retry-After`;
  never retry other 4xx.
- **Rate limits:** per-account token bucket, configurable; surface remaining
  quota via tracing.
- **Webhooks:** verify `X-Twilio-Signature` with constant-time comparison;
  reject stale timestamps; support replayed delivery callbacks.
- **Idempotency:** Twilio does not provide a general idempotency key for SMS.
  Dedupe at the action layer using recipient + body hash + a short window, and
  store the returned Message SID.

## GIA integration

Agent 2 sends a recovery SMS within 60 seconds of a missed call. Agent 1
answers and books; Agent 8 sends review requests after payment. Each send
must carry the workspace's messaging service SID and the compliance decision
from Tool 13. Voice calls need a TwiML endpoint and a recording policy.

## Test plan

- Test credentials and magic test numbers.
- Inbound webhook fixtures for SMS, voice, and status callbacks.
- Signature validation: valid, tampered, stale.
- Delivery failure: undelivered, failed, opt-out.
- Rate-limit and 5xx retry with a mock transport.

## Open questions

1. **A2P 10DLC / toll-free verification.** Which sender type per workspace?
   Registration is a launch blocker for SMS at scale.
2. **Messaging Service SID vs phone number.** Use a messaging service for
   pooled sending and compliance features; confirm per-tenant isolation.
3. **Voice hosting.** Where does the TwiML endpoint live, and how is it
   signed? This is a service concern, not a wirekit concern, but it must be
   decided before Agent 1 can answer calls.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §1.
