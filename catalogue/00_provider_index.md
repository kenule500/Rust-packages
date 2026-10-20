# Provider Clients — the 26 wirekit modules

The 26 providers below are **modules inside `wirekit`**, not separate crates.
Each has its own design file in this folder so a provider can be reviewed,
owned, or scheduled on its own — without pretending it is a published crate.

The repository also holds standalone packages that are not provider clients.
All are public crates.io candidates; see the [README](README.md).

- **Provider client:** `wirekit` — one crate, 26 feature-flagged modules.
- **Sync:** `syncforge` — one crate for offline sync.
- **Quota:** [`quota-ledger`](29_quota.md) — per-tenant provider quota ledger.
- **Webhooks:** [`hookgate`](30_webhook_gateway.md) — inbound webhook edge.
- **Integration substrate:** [`platform-core`](31_platform_core.md) — generic provider traits, registry, token-store trait.
- **Composition:** [`ffscene`](32_ffscene.md) — scene graph → FFmpeg graph.
- **Audio:** [`mixbus`](33_mixbus.md) — real-time mixing graph.
- **Recording:** [`replay-recorder`](34_replay_recorder.md) — tracks and replay ring.
- **Production control:** [`studio-protocol`](35_studio_protocol.md) — preview/program protocol.
- **Egress:** [`rtmp-publish`](36_rtmp_publish.md) — destination model and health.
- **Links:** [`signedlink`](37_signedlink.md) — signed links and attribution.
- **Provider docs:** `03_twilio.md` … `28_google_oauth.md`.
- **Technical contract:** the provider docs in this folder; the GIA catalog copy is pending.

---

## The 26 provider docs

| File | Provider | Module | Third-party API | Access |
|---|---|---|---|---|
| [03_twilio.md](03_twilio.md) | Twilio | `wirekit::twilio` | SMS, Voice, MFA, Lookup | Standard account |
| [04_sendgrid.md](04_sendgrid.md) | SendGrid | `wirekit::sendgrid` | Transactional email | Standard account |
| [05_meta.md](05_meta.md) | Meta Graph | `wirekit::meta` | Messenger, Instagram, WhatsApp | App review / business verification |
| [06_google_business.md](06_google_business.md) | Google Business Profile | `wirekit::google_business` | Listings, reviews, posts | Google API approval |
| [07_noaa.md](07_noaa.md) | NOAA | `wirekit::noaa` | Forecasts, alerts, radar | No auth |
| [08_accuweather.md](08_accuweather.md) | AccuWeather | `wirekit::accuweather` | Forecasts, alerts, history | Commercial API key |
| [09_attom.md](09_attom.md) | ATTOM | `wirekit::attom` | Property records, valuations | Commercial contract |
| [10_corelogic.md](10_corelogic.md) | CoreLogic | `wirekit::corelogic` | Property records, valuations | Commercial contract |
| [11_buildzoom.md](11_buildzoom.md) | BuildZoom | `wirekit::buildzoom` | Permit data, contractors | Commercial/partner |
| [12_eagleview.md](12_eagleview.md) | EagleView | `wirekit::eagleview` | Satellite roof measurement | Commercial contract |
| [13_quickbooks.md](13_quickbooks.md) | QuickBooks | `wirekit::quickbooks` | Accounting | OAuth app approval |
| [14_jobber.md](14_jobber.md) | Jobber | `wirekit::jobber` | CRM/accounting | OAuth app approval |
| [15_sage_intacct.md](15_sage_intacct.md) | Sage Intacct | `wirekit::sage_intacct` | Accounting | Partner/company credentials |
| [16_adp.md](16_adp.md) | ADP | `wirekit::adp` | Payroll | Enterprise/partner |
| [17_gusto.md](17_gusto.md) | Gusto | `wirekit::gusto` | Payroll | OAuth app approval |
| [18_lob.md](18_lob.md) | Lob | `wirekit::lob` | Direct mail | Standard account |
| [19_yelp.md](19_yelp.md) | Yelp | `wirekit::yelp` | Business search, reviews, leads | Fusion API; leads may be partner-only |
| [20_angi.md](20_angi.md) | Angi | `wirekit::angi` | Lead import | **No public API** — manual/partner |
| [21_google_lsa.md](21_google_lsa.md) | Google LSA | `wirekit::google_lsa` | Local Services Ads leads | Google Ads API access |
| [22_pagerduty.md](22_pagerduty.md) | PagerDuty | `wirekit::pagerduty` | Incidents, alerts, on-call | Standard account |
| [23_slack.md](23_slack.md) | Slack | `wirekit::slack` | Notifications, channels | OAuth app |
| [24_deepgram.md](24_deepgram.md) | Deepgram | `wirekit::deepgram` | Speech-to-text | Standard account |
| [25_klarna.md](25_klarna.md) | Klarna | `wirekit::klarna` | Consumer financing | Merchant account |
| [26_wisetack.md](26_wisetack.md) | Wisetack | `wirekit::wisetack` | Consumer financing | Partner account |
| [27_leadperfection.md](27_leadperfection.md) | LeadPerfection | `wirekit::leadperfection` | Calendar booking | Partner/API key |
| [28_google_oauth.md](28_google_oauth.md) | Google OAuth | `wirekit::google_oauth` | Shared OAuth helper | Google Cloud project |

---

## “Are the other packages already online?”

**The third-party services: yes.** Twilio, SendGrid, NOAA, EagleView,
QuickBooks, and the rest are live APIs. GIA does not build them.

**The Rust clients: no, not published by GIA.** `wirekit` and `syncforge` are
blueprint packages — neither is on crates.io yet.

Some providers have community Rust crates — Twilio, SendGrid, Slack, and
PagerDuty have community work — but there is no unified, maintained client
with one error model, retry policy, rate limiter, and auth layer. Several
providers have no maintained Rust client at all; the blueprint names ATTOM,
EagleView, Jobber, Sage Intacct, BuildZoom, and Lob.

So “packages to build” means: **build one Rust client per provider inside
`wirekit`**, not 26 separate crates.

---

## What every provider doc contains

Each `NN_provider.md` follows the same shape:

1. **What it does** — the provider in one paragraph and the GIA use.
2. **Auth and base URL** — from the technical contract; no invented endpoints.
3. **Surface to build** — the structs and methods, copied from 21.
4. **Wirekit contract** — how it maps errors, retries, rate limits, webhooks,
   and idempotency into the shared layer.
5. **GIA integration** — which agent/tool uses it and what breaks if it fails.
6. **Test plan** — sandbox, fixtures, and the failure cases to cover.
7. **Open questions** — access, terms, and anything that must be verified
   before committing.

These docs are design notes, not API references. The provider’s own docs remain
the source of truth for field-level behavior.

---

## Open questions across providers

1. **Stripe is missing from the 26.** GIA uses Stripe for payments, and the
   wirekit examples reference it, but there is no `wirekit::stripe` module.
   Decide: wrap Stripe, or keep the community crate and remove Stripe from the
   examples.
2. **Angi has no public API.** The module is an import/partner helper, not a
   live client. Do not promise an integration that cannot be built.
3. **Google LSA requires Ads API access.** The client is straightforward; the
   approval and account structure are not.
4. **Provider rollout order.** Shipping all 26 at once is not required. The
   GIA critical path is Twilio, SendGrid, Meta, NOAA, EagleView, QuickBooks,
   Stripe, and Slack. The rest can follow.

---

## Status

Blueprint. No provider client code, no Cargo files, no generated SDKs. The
next step is to review the provider docs in priority order, starting with the
critical path above.
