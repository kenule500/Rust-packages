# wirekit

**One HTTP client for many providers.** A shared error type, retry policy, rate
limiter, auth layer, tracing story, and request pipeline — so an application
that talks to four APIs contains one HTTP client, not four.

```toml
[dependencies]
wirekit = { version = "0.1.0", features = ["noaa", "twilio", "sendgrid", "slack"] }
```

The core is always compiled. Features add provider modules; enabling one never
enables or disables another.

| Feature | Module | Auth | Base URL |
|---|---|---|---|
| — (core) | `wirekit` | Bearer, Basic, API key (header/query), HMAC, OAuth2 client credentials | — |
| `noaa` | `wirekit::noaa` | None; a descriptive `User-Agent` is required | `https://api.weather.gov` |
| `twilio` | `wirekit::twilio` | Basic (auth token or API key) | `https://api.twilio.com/2010-04-01` |
| `sendgrid` | `wirekit::sendgrid` | Bearer API key | `https://api.sendgrid.com/v3` |
| `slack` | `wirekit::slack` | Bot token | `https://slack.com/api` |
| `native-tls` | — | Adds the platform TLS backend; rustls is always compiled in | — |

## Why one client

Each provider integration normally grows its own error enum, its own retry loop,
and its own rate limiter. That is four ways to learn that a request failed, four
backoff curves, and four connection pools. `wirekit` keeps the shared parts in
one place and lets a provider supply only what is genuinely provider-specific: a
URL, a method, a body, and a response type.

## Example

One client, one pool, two providers:

```rust,no_run
use wirekit::noaa::{ForecastRequest, NoaaClient};
use wirekit::WirekitClient;

# async fn run() -> Result<(), wirekit::WirekitError> {
let client = WirekitClient::builder(wirekit::noaa::BASE_URL)?.build()?;
let noaa = NoaaClient::builder(client)
    .user_agent("storm-service/1.0 (ops@example.com)")
    .build();

let forecast = noaa.forecast(&ForecastRequest::new(40.7128, -74.0060)?).await?;
println!("{} periods", forecast.periods.len());
# Ok(())
# }
```

Providers hold their own auth scheme and attach it to each request, so a single
`WirekitClient` can be cloned into several providers without a global singleton.
Clone it freely: every clone points at the same pool, retry policy, and token
buckets.

## Pipeline order

Every request runs through the same stages, in this order:

1. **Auth** — the provider's scheme prepares the request. Schemes fail closed.
2. **Rate limit** — the per-provider token bucket, if configured.
3. **Retry loop** — send, classify, back off, retry.
4. **Decode** — the body is decoded into the requested type.

A retryable failure is `429`, any `5xx`, a timeout, or a transport error. Every
other `4xx` is final. A `Retry-After` header (seconds or an HTTP date) wins over
computed backoff. Backoff is exponential with jitter and is capped by the
policy's `max_delay`.

Slack and NOAA report some failures inside a `200` body. The pipeline runs a
body-error hook before decoding, so `ok: false` and `problem+json` never become
silent successes.

## Errors

One enum, stable reason codes, and a `Debug` that never prints a credential, a
token, or a raw body:

| Code | Variant | Meaning |
|---|---|---|
| `W2001` | `Api` | A non-success status, or an error reported in a 200 body. Carries the status, the provider code, a short message, and `details` |
| `W2002` | `Auth` | A credential was rejected, or the auth flow failed |
| `W2003` | `RateLimit` | The bucket or the provider refused the request; carries `retry_after` |
| `W2004` | `Timeout` | The request timed out |
| `W2005` | `Network` | The transport failed before a complete response |
| `W2006` | `InvalidConfig` | The configuration cannot produce a valid request |
| `W2007` | `Decode` | The body did not match the expected type |

```rust
use wirekit::WirekitError;

match error {
    WirekitError::Api { status, code, .. } => eprintln!("HTTP {status}: {code}"),
    WirekitError::RateLimit { retry_after } => eprintln!("retry in {retry_after:?}"),
    WirekitError::Auth { .. } => eprintln!("check the credential"),
    other => eprintln!("{}: {other}", other.code()),
}
```

The enum is `#[non_exhaustive]`, so match with a wildcard arm.

## Providers

### NOAA

Point forecasts (latitude/longitude → grid → periods), active alerts by area or
zone, single-alert lookup, and radar-station metadata. A descriptive
`User-Agent` is mandatory; a request without one is `InvalidConfig`. Successful
GETs are cached in memory with a TTL.

### Twilio

SMS, WhatsApp, voice calls, call status, Verify v2 MFA challenges, and Lookups
v2 phone validation. Basic authentication carries either the account auth token
or an API key pair. `verify_webhook` checks the `X-Twilio-Signature` over the
exact received form bytes; the canonical string is the URL plus sorted
parameter names and values, HMAC-SHA1, base64.

### SendGrid

`/mail/send` with typed addresses, HTML and text content, dynamic templates,
attachments, and sandbox mode. The `X-Message-Id` header becomes
`SendEmailResponse::message_id`. An `errors[]` body becomes `WirekitError::Api`
with the first entry's field and message and the full array in `details`.
`verify_event_webhook` checks the ECDSA P-256 signature over
`timestamp || payload` and requires the exact received bytes.

### Slack

`chat.postMessage`, `conversations.list`, `conversations.info`, `users.info`,
and `files.upload`. `ok: false` at HTTP 200 becomes `WirekitError::Api`;
`channel_not_found` and `not_in_channel` are declared non-retryable.
`verify_request` checks `v0:{timestamp}:{body}` with HMAC-SHA256, hex-encoded,
and rejects a timestamp outside the allowed skew.

## Security notes

- All requests use HTTPS. `allow_insecure_localhost(true)` exists only for local
  mock servers and never applies to a non-loopback host.
- Credentials are wrapped in `Secret`, whose `Debug` and `Display` render `***`.
  Secrets are never serialized and never reach a log line.
- URLs are redacted before tracing: query values become `***`.
- Webhook verification always runs over the bytes received, never over a
  re-serialization of parsed parameters.
- The crate never reads environment variables, `.env` files, or credential
  files. The caller supplies credentials.

## Gated provider features

These features are additive and off by default; enabling one never enables another.

| Feature | Module | Network | What the caller supplies |
|---|---|---|---|
| `angi` | `wirekit::angi` | none | the email text or CSV bytes to parse |
| `google-lsa` | `wirekit::google_lsa` | Local Services Ads API | an OAuth2 token carrying the `adwords` scope |
| `sage-intacct` | `wirekit::sage_intacct` | Intacct XML gateway | sender ID, sender password, company ID, user ID |

`angi` parses Angi lead emails and CSV exports into a normalized lead and a
deterministic dedupe key. Angi publishes no API, so this module has no client and
does no polling; a partner webhook is verified only when a tenant has one.

`google-lsa` implements the two search methods the published `localservices:v1`
API actually defines - `v1/accountReports:search` and
`v1/detailedLeadReports:search`. That API is reporting-only, so this module
deliberately offers no lead-status update, no booking report, and no notification
ingestion.

`sage-intacct` posts the documented XML envelope to the gateway, derives success
from the gateway's own `status` and `errorno` rather than the HTTP status, bounds
the response before parsing it, and sends the caller's key as `<uniqueid>` so a
retried create is idempotent.

ADP is not implemented. Its documentation could not be obtained, so no token
endpoint or resource path could be verified, and a guessed route has no place in a
published crate.

## Minimum supported Rust version

Rust 1.85, edition 2021.

## License

MIT OR Apache-2.0.
