# signedlink

Signed, revocable short links. `signedlink` signs a compact claim payload with
HMAC-SHA256, mints base62 codes from OS entropy, normalises the five UTM
parameters, and enforces revocation and single use through a caller-supplied
store.

## Scope

- HMAC-SHA256 signing with URL-safe base64 and no padding.
- Constant-time MAC verification.
- Expiry and clock-skew checks with caller-supplied verification time.
- Base62 codes with bounded collision retry.
- Closed-set UTM normalisation and an explicit attribution window.
- Store-backed revocation and atomic single use.
- Optional `axum` redirect handler with `Referrer-Policy: no-referrer`.

## Non-goals

- A database, Redis client, HTTP client, or server.
- Analytics storage; the click hook is a callback.
- HTML, cookies, or a global header pipeline.
- Persisting keys. Keys are caller-supplied and live in memory only.

## Token format

```text
base64url(payload).base64url(HMAC-SHA256(key, "m4signedlink.v1\0" || payload))
```

Both parts use URL-safe base64 without padding. The payload is a fixed-order
`application/x-www-form-urlencoded` string:

```text
v=1&target=...&tenant=...&campaign_set=1&campaign=...&attribution_window=...
 &single_use=0&nonce=...&issued_at=...&expires_at=...
 &utm_campaign=...&utm_content=...&utm_medium=...&utm_source=...&utm_term=...
```

Unknown, duplicate, or missing fields fail closed. The MAC is verified before the
payload is parsed. `SigningKey`, `SignedToken`, and `SignedUrl` print `***` in
`Debug`/`Display` and implement no secret-bearing `Serialize`.

## Store requirement

Stateless `Signer::verify` checks the MAC, issue instant, and expiry. It cannot
revoke or consume. Revocation and single use require `verify_with_store` and a
`LinkStore`:

```text
put(code, claims)
get(code) -> Option<Claims>
revoke(code) -> bool
consume(code) -> bool
is_revoked(code) -> bool
is_consumed(code) -> bool
```

The last two keep missing, revoked, and consumed distinct. Consumption must be
atomic. Implementations must use parameterised SQL queries or equivalent
argument binding; never concatenate a code or token into a command string.

`MemoryLinkStore` is an offline example, not a production store: it never
evicts and does not survive a restart.

## Codes

A code is an identifier; the signature is the authorisation. Codes are base62,
4-32 characters, generated from OS entropy and rejection-sampled to avoid modulo
bias. Generation checks `get`, `is_revoked`, and `is_consumed`, retries collisions
up to the configured limit, and then returns `CodeExhausted`.

## UTM contract

The only accepted keys are `utm_source`, `utm_medium`, `utm_campaign`,
`utm_term`, and `utm_content`. Keys are matched case-insensitively and stored in
lower case. Values are trimmed and bounded to 256 characters; empty values and
control characters are rejected. Unknown keys are rejected, never dropped.
Canonical order is `utm_campaign`, `utm_content`, `utm_medium`, `utm_source`,
`utm_term`.

`Claims::new` requires an `attribution_window` argument (1 second to 1 year).
There is no default.

## Expiry and skew

`verify(token, now)` takes Unix seconds and a configured skew up to 300 seconds:

- valid from `issued_at - skew` (inclusive),
- valid through `expires_at + skew` (inclusive),
- before that becomes `NotYetValid`, after it becomes `Expired`.

## Revocation and single use

- `Revoked`: the store reports the code revoked.
- `Consumed`: the single-use nonce was already consumed.
- `NotFound`: no active link exists.
- `Tampered`: the stored claims differ from the signed claims.

For single use, the first successful verification calls `consume`; only the
first caller receives claims. Revocation is immediate and does not wait for
expiry.

## Redirect handler

With the `axum` feature, `redirect::redirect_handler` resolves `/{code}/{token}`,
verifies against the store, calls the optional click hook, and redirects to the
signed target. It sets:

```text
Referrer-Policy: no-referrer
Cache-Control: no-store
```

It refuses a target that is not HTTPS, so it cannot downgrade a scheme, and it
never echoes the token or the key. HSTS remains the deployment edge's
responsibility.

## Features

- `serde`: `Serialize`/`Deserialize` for `Claims`, `Utm`, and `TenantId`.
- `axum`: the redirect handler only.

## Example

```text
cargo run -p signedlink --example sign_and_verify
```
