# Developer Experience Design System

> **Status:** Approved product plan
> **Date:** 2026-10-20
> **Scope:** This is a design system for a Rust library ecosystem, not a visual UI.
> **Visual brand:** `[to be confirmed]` - no colours, typography, logos, or design tokens are defined.

## Purpose

The product's interface is its API, documentation, examples, errors, and upgrade path. This document defines the patterns that must stay consistent across all 11 crates so a developer learns the model once.

## Principles

| Principle | What it means in practice |
|---|---|
| One model | One error enum, one retry policy, one client builder, one tracing story |
| Typed over stringly | Newtypes for IDs; typed requests and responses; no `serde_json::Value` in public signatures unless the provider is genuinely open-ended |
| Strict over permissive | Unknown input is an error with a position, not a silent guess |
| Secure by default | Secrets are redacted, signatures are verified on raw bytes, tenants are isolated |
| Observable | Every outbound call emits tracing events; retries and rate limits are visible |
| Copy-pasteable | Every provider has a runnable example that compiles |
| Honest | A provider with no public API is marked deferred, not "coming soon" |
| Composable | Stateful and media crates are opt-in; nothing forces a framework or database |

## API naming conventions

| Element | Convention | Example |
|---|---|---|
| Shared client | `Client` + `ClientBuilder` | `wirekit::Client::builder()` |
| Provider module | Lowercase provider name | `wirekit::twilio` |
| Request type | `<Verb><Resource>Request` | `SendMessageRequest` |
| Response type | Resource noun | `Message` |
| Identifier | Newtype | `MessageSid`, `TenantId`, `ActionId` |
| Error | Crate-prefixed enum | `WirekitError` |
| Trait | Capability noun | `Verifier`, `TokenStore`, `Storage`, `Transport` |
| Feature | Provider or module name, lowercase | `twilio`, `sendgrid`, `yaml` |
| Method | Verb, no `get_` prefix | `send`, `fetch`, `list`, `create`, `update`, `delete` |
| Conversion | `as_`, `to_`, `into_` | `as_str`, `to_owned`, `into_inner` |

Rules:

- Public enums that may grow SHOULD be `#[non_exhaustive]`.
- Public structs SHOULD have private fields and constructors or builders.
- Every public type implements `Debug`; `Debug` output is never empty.
- No public API returns a provider's raw JSON type unless the provider's surface is genuinely open-ended.

Source: `.sdrive/constitution.md` CON-216; Rust API Guidelines.

## Error taxonomy

### `wirekit`

| Variant | Meaning | Developer action |
|---|---|---|
| `Api { status, code, message }` | Provider returned an error response | Inspect `code`; fix the request or retry if transient |
| `Auth` | Credential missing, expired, or rejected | Refresh, re-consent, or check scopes |
| `RateLimit { retry_after }` | Provider throttled the call | Wait for the hint; the client already retried |
| `Timeout` | The request exceeded the configured timeout | Increase timeout or investigate latency |
| `Network` | DNS, connection, or TLS failure | Check connectivity and base URL |
| `InvalidConfig` | Missing or malformed configuration | Fix the builder input; no secret is echoed |

Rules:

- A provider MUST map its failures into this enum (CON-207).
- HTTP 200 with an error body MUST NOT be treated as success (CON-213).
- Error messages MUST NOT contain secrets (CON-214).
- Retry policy is shared (CON-208); a provider MUST NOT invent a second policy.

### `dado-yaml`

| Code | Meaning |
|---|---|
| `D0001` | YAML parser scaffold; parsing is not implemented yet |
| `D1001` | Parse or deserialization failure with line and column |
| `E1001` | Example diagnostic code for the diagnostics module |

Rules:

- Codes are stable public identifiers; a code MUST NOT be reused with a different meaning (CON-205).
- Every error carries a line and column when input position is known (CON-206).

### Other crates

Each crate defines one error enum and follows the same rules. `hookgate` uses `VerifyError`; `signedlink` uses `LinkError`; `platform-core` uses `TokenError`; `syncforge` uses its own error type. They MUST NOT leak provider-specific error shapes into the shared model.

## Feature-flag conventions

- `default = []` unless a feature is required for the crate to be useful.
- One feature per provider module.
- Feature names are lowercase, match the provider, and contain no placeholder words.
- Optional dependencies are enabled with `dep:` syntax.
- Features are additive: enabling one MUST NOT disable another (CON-204).
- The feature table is documented at the top of `lib.rs` and in the README.
- The default feature set is part of the public contract; removing a feature is a breaking change.

Example:

```toml
[features]
default = []
twilio = ["dep:reqwest", "dep:serde"]
sendgrid = ["dep:reqwest", "dep:serde"]
noaa = ["dep:reqwest", "dep:serde"]
slack = ["dep:reqwest", "dep:serde"]
```

## Module layout

```
crates/<crate>/
  Cargo.toml
  README.md
  src/
    lib.rs            # crate docs, feature table, re-exports
    client.rs         # shared client / builder
    error.rs          # error enum
    retry.rs          # shared retry policy
    ratelimit.rs      # token bucket
    auth.rs           # auth abstractions
    providers/
      twilio.rs
      sendgrid.rs
      noaa.rs
      slack.rs
  examples/
    twilio_send_sms.rs
    sendgrid_send_email.rs
    noaa_forecast.rs
    slack_post_message.rs
  tests/
    <feature>/
      <subfeature>.rs
```

Rules:

- `lib.rs` starts with `//!` docs that state the promise, feature table, and quickstart.
- Each module starts with `//!` docs that state scope and non-goals (CON-203).
- Integration tests follow `crates/<crate>/tests/<feature>/<subfeature>.rs` (CON-406).
- Examples are runnable and read configuration from environment variables, never hard-coded secrets.

## Documentation structure

Every crate README follows this order:

1. One-sentence promise.
2. Install command with the relevant features.
3. Minimal working example.
4. Feature table.
5. Error handling.
6. Provider or module table.
7. Observability.
8. Security notes.
9. Status and version.
10. Licence.

Every provider module doc follows this order:

1. What it does.
2. Auth and base URL.
3. Required scopes or account access.
4. Surface: structs and methods.
5. Retry and rate-limit behavior.
6. Webhook behavior.
7. Example.
8. Failure cases.
9. Open questions.

## Example template

```rust
//! Send an SMS with Twilio.
//!
//! Run with:
//! `TWILIO_ACCOUNT_SID=... TWILIO_AUTH_TOKEN=... cargo run --example twilio_send_sms --features twilio`

use wirekit::twilio::{SendMessageRequest, Twilio};

#[tokio::main]
async fn main() -> Result<(), wirekit::WirekitError> {
    let client = Twilio::from_env()?;
    let message = client
        .messages()
        .send(SendMessageRequest {
            to: "+15555550100".into(),
            from: "+15555550101".into(),
            body: "Hello from wirekit".into(),
        })
        .await?;

    println!("sent {}", message.sid);
    Ok(())
}
```

Rules:

- No `unwrap()` or `expect()` in examples; use `?`.
- No real phone numbers, keys, or tokens.
- The example name is `<provider>_<action>.rs`.
- The doc comment includes the exact run command.
- The example is compiled in CI.

## README template

```markdown
# <crate>

<One-sentence promise.>

```toml
<crate> = { version = "0.1", features = ["<feature>"] }
```

## Quickstart

<Minimal working example.>

## Features

| Feature | Module | What it does |
|---|---|---|

## Errors

<One table of variants and actions.>

## Status

0.x. APIs may change. See the changelog.

## Licence

MIT OR Apache-2.0
```

## Provider matrix template

| Provider | Feature | Auth | Status | Docs |
|---|---|---|---|---|
| Twilio | `twilio` | Account SID + auth token | first publish | link |
| SendGrid | `sendgrid` | API key | first publish | link |
| NOAA | `noaa` | None | first publish | link |
| Slack | `slack` | OAuth / bot token | first publish | link |
| Meta | `meta` | App review / business verification | critical path | link |
| ... | ... | ... | ... | ... |

Status values:

- `first publish`
- `critical path`
- `backlog`
- `needs discovery`
- `deferred`
- `not supported` (no public API)

## Secret handling pattern

- Credentials come from a caller-supplied config or a secret store.
- Examples read from environment variables; `.env.example` documents the names.
- `Debug` and `Display` redact secrets as `***`.
- No `Serialize` implementation for a secret field.
- Error payloads and tracing spans never include credentials, tokens, or stream keys.
- Property tests assert that redaction holds.

Source: `.sdrive/constitution.md` CON-214; `catalogue/36_rtmp_publish.md`.

## Deprecation policy

1. Mark the item with `#[deprecated(since = "<version>", note = "<replacement>")]`.
2. Add a changelog entry under `Deprecated`.
3. Keep the item for at least one minor release before removal.
4. Provide a migration example in the changelog.
5. In 0.x, a removal still requires a changelog entry and a migration note.

## Versioning and release pattern

| Concern | Rule |
|---|---|
| Version scheme | Semver per crate, starting at 0.1.0 |
| Stability | 0.x: APIs may change; 1.0 freezes after external usage |
| Changelog | Per-crate `CHANGELOG.md`, Keep a Changelog format |
| Release notes | Summarize breaking changes, deprecations, and new providers |
| MSRV | 1.85 until a constitution amendment changes it |
| Licence | `MIT OR Apache-2.0` on every crate |

## Visual and brand guidance

- There is no application UI, so there are no colours, typography, spacing, or component tokens to define.
- No Figma file, screenshot, or brand guide was provided.
- If a docs site is built, it SHOULD use a readable, accessible default rather than a custom design system.
- Brand name, logo, and colour palette are `[to be confirmed]`.
- Do not invent a brand identity for the plan.

## Accessibility

- The library itself has no user interface.
- If a documentation site is built, it MUST target WCAG 2.2 AA.
- Code examples MUST be plain text, keyboard-selectable, and readable by screen readers.
- Status labels MUST not rely on colour alone.

## Testing patterns

| Pattern | Use |
|---|---|
| Inline `#[cfg(test)]` | Unit tests for parsing, retry math, redaction |
| `tests/<feature>/<subfeature>.rs` | Integration tests against a mock transport |
| `wiremock` / `httpmock` | Deterministic provider responses, including 200-with-error bodies |
| `proptest` | Secret redaction, glob matching, graph acyclicity, signature tamper cases |
| Doc tests | Every public example that can run without credentials |
| Failure fixtures | 401, 403, 429 with `Retry-After`, 5xx, timeout, malformed JSON |

## Open questions

1. Should the shared client be a concrete `Client` or a trait so consumers can inject their own transport?
2. Should `WirekitError` be `#[non_exhaustive]` from 0.1?
3. Should error codes be numeric, string, or both?
4. Should the docs site be mdBook, Docusaurus, Astro, or docs.rs only?
5. Should examples live in each crate or in a dedicated `examples` workspace member?
6. Should the provider matrix be generated from Cargo features?
7. What is the public brand name and logo?
8. Should the project adopt a CLA, DCO, or neither?
