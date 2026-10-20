# 37 — signedlink

**Package:** `signedlink` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library, optional `axum` redirect feature
**Licence:** MIT OR Apache-2.0

`signedlink` creates short, signed, expiring links with UTM parameters and an
attribution window. It answers: **was this click ours, which campaign sent it,
and is it still valid?**

It is the link layer under affiliate, checkout, and share flows. It does not run
a marketing site.

---

## Why this is not a wirekit module

No provider API. It is also not `hookgate`: that verifies inbound webhooks; this
signs outbound URLs. They share a threat model (tampering) but not a direction.

---

## Scope

### Core types

```rust
pub struct Signer {
    pub key: SigningKey,      // HMAC-SHA256 or Ed25519
    pub ttl: Duration,
    pub clock_skew: Duration,
}

pub struct Claims {
    pub target: Url,
    pub campaign: Option<String>,
    pub tenant: TenantId,
    pub attribution_window: Duration,
    pub single_use: bool,
}

pub trait LinkStore {
    async fn put(&self, code: &Code, claims: &Claims) -> Result<(), LinkError>;
    async fn get(&self, code: &Code) -> Result<Option<Claims>, LinkError>;
    async fn revoke(&self, code: &Code) -> Result<(), LinkError>;
}
```

### Operations

- **Sign and verify.** `sign(claims) -> SignedUrl` appends a compact,
  URL-safe token. `verify(url, now) -> Result<Claims, LinkError>` rejects
  tampering, expiry, and clock skew outside the allowed window.
- **Short code.** Base62 code, collision-checked against the store. The code is
  an identifier; the signature is the authorisation. Do not rely on obscurity.
- **UTM builder.** Normalise `utm_source`, `utm_medium`, `utm_campaign`,
  `utm_term`, `utm_content`; reject unknown keys rather than dropping them
  silently.
- **Attribution window.** Claims carry the window; the analytics join uses it
  and never invents a default.
- **Single use.** Optional nonce consumed atomically in the store.

### Redirect path

```rust
pub async fn handle(path: Path<String>, state: State<App>) -> Response
```

An optional `axum` feature provides the redirect handler: resolve, verify,
record the click, redirect. Absent the feature, the app writes its own handler.

---

## Non-goals

- **Not an analytics system.** It records that a click happened; rollups belong
  elsewhere.
- **Not a URL shortener for arbitrary public links.** No anonymous abuse surface.
- **Not an auth system.** A signed link is a capability, not a session.
- **Not a QR generator.** Emit the URL; the app renders the code.

---

## Integration

1. The app mints a link when it needs a share, checkout, or affiliate target.
2. The store holds `code -> claims`; the signature is stateless and verifiable
   without a lookup, but the store is still needed for revocation and single use.
3. Incoming `/l/:code` verifies, records the click, and redirects.
4. Analytics stores `code`, `campaign`, `tenant`, and the attribution window.

---

## Test plan

- Tampered target, campaign, or tenant fails verification.
- Expired and single-use links fail closed.
- Clock skew beyond the allowed window is rejected; within it is accepted.
- UTM normalisation is stable and rejects unknown keys.
- Code collision is retried and bounded.
- Redirect never leaks the signature in a `Referer` to the target.
- Revocation takes effect immediately.

---

## Open questions

1. **Crate name.** `signedlink` is free. Confirm before publish.
2. **Signature algorithm.** HMAC-SHA256 is simple and sufficient; Ed25519 allows
   public verification. Ship HMAC only?
3. **Stateless vs stateful.** Fully stateless links cannot be revoked. Is the
   store mandatory in 1.0?
4. **Single use.** Atomic nonce consumption needs a store-specific primitive;
   is that in scope or an app responsibility?
