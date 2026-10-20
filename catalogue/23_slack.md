# 23 — Slack

**Provider:** Slack
**Package:** `wirekit` (module `wirekit::slack`)
**Status:** provider design, not a crate
**Covers:** Notifications, channels, users
**Base URL:** `https://slack.com/api`
**Auth:** Bot token + OAuth2

## What it does

Slack is GIA's internal notification channel: daily reports, escalations,
system health, and team alerts. It is not a homeowner channel. The contractor's
team can receive a storm alert, a failed invoice sync, or a safety escalation
in the Slack workspace they already use.

The module wraps the Web API with typed messages, blocks, channels, and file
uploads.

## GIA use

- Tool 14 (Escalation & Alert) for team notifications
- Agent 9 (Reporting) — daily/weekly reports
- Agent 11 (Error Detection) — system health
- Agent 15 (Team Management) — workload and burnout alerts

## Surface to build

| Type | Purpose |
|---|---|
| `SlackClient` | Bot token, team ID |
| `Message` | Text, blocks, thread |
| `Block` | Block Kit builder |
| `Channel` | List, join, post |
| `FileUpload` | Logs, reports, PDFs |

Methods: post message, post blocks, upload file, list channels, get user,
verify request signature.

## Wirekit contract

- **Auth:** bot token per workspace; OAuth2 install flow for multi-tenant.
- **Errors:** Slack returns HTTP 200 with `ok: false` and an `error` string;
  do not treat 200 as success. Map to `WirekitError::Api`.
- **Retry:** 429 with `Retry-After`; 5xx; do not retry `channel_not_found` or
  `not_in_channel`.
- **Rate limits:** tiered per method; respect `Retry-After`; cache channels.
- **Webhooks/events:** verify `X-Slack-Signature` and timestamp; respond to
  slash commands within 3 seconds, then update asynchronously.
- **Idempotency:** no general key; dedupe on a caller event ID in the message
  metadata where possible.

## GIA integration

Agent 9 posts the daily report to a configured channel. Agent 11 posts system
health alerts. Agent 10 posts safety escalations with a link to the incident.
The tenant connects Slack via OAuth; GIA never stores a user token.

## Test plan

- Slack test workspace and bot scopes.
- Message and Block Kit rendering.
- `ok: false` error handling.
- 429/`Retry-After` behavior.
- Signature verification and replay.
- File upload size/type limits.

## Open questions

1. **One app or many?** A single GIA Slack app with per-tenant installs is
   simpler; confirm scopes and review requirements.
2. **Channel mapping.** Which events go to which channel? Make it configurable,
   not hard-coded.
3. **PII.** Reports may contain customer data; confirm the tenant's Slack
   retention and access rules.
4. **Slash commands.** If GIA supports them, the 3-second response limit
   requires an async worker.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §21.
