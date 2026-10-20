# 31 — platform-core

**Package:** `platform-core` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library, optional `sqlx`/`axum` features
**Licence:** MIT OR Apache-2.0

`platform-core` is the generic, tenant-scoped substrate for a Rust product that
integrates many third-party providers. It defines the traits and the registry
that every provider family shares: who the tenant is, which provider is being
called, what that provider can actually do, and where its token lives.

It is a **trait crate**. It deliberately ships no product vocabulary, no OAuth
implementation, no quota client, and no webhook client. Those belong to the
consuming product's own adapter crate.

---

## Why this is not a wirekit module

| Concern | `wirekit` | `platform-core` |
|---|---|---|
| Unit | One provider client | A fleet of providers |
| Identity | API credentials | Tenant + provider pair |
| State | None | A `TokenStore` trait the app implements |
| Knowledge | Endpoints and payloads | Capability shape and provisioning mode |
| Consumer | Any single integration | A multi-tenant product |

`wirekit` answers “how do I call Twilio?”. `platform-core` answers “which
providers does this tenant have, what can each one actually do, and where does
the token live?”.

---

## Scope

### Core types

```rust
pub struct TenantId(pub Uuid);
pub struct ProviderId(pub &'static str);

pub enum ProvisioningMode {
    Api,          // fully self-serve
    Partner,      // requires an approved partner program
    Manual,       // operator handoff, e.g. a stream key
    Unsupported,  // no public creation path
}

pub struct Capability {
    pub key: &'static str, // opaque to this crate
    pub mode: ProvisioningMode,
}

pub trait Provider: Send + Sync {
    fn id(&self) -> ProviderId;
    fn capabilities(&self) -> &'static [Capability];
}

pub trait TokenStore {
    async fn get(&self, t: &TenantId, p: &ProviderId) -> Result<Option<Secret>, TokenError>;
    async fn put(&self, t: &TenantId, p: &ProviderId, token: Secret) -> Result<(), TokenError>;
    async fn revoke(&self, t: &TenantId, p: &ProviderId) -> Result<(), TokenError>;
}
```

### Operations

- **Registry.** `register`, `get`, `iter`, `capabilities`. Registration is
  explicit; there is no global mutable singleton.
- **Capability query.** Callers read the map. They must never infer “supported”
  from the presence of a crate.
- **Token lifecycle.** The trait is storage-agnostic. Encryption, KMS, and
  rotation are the implementor's responsibility.
- **Error taxonomy.** One `IntegrationError` with `Auth`, `Scope`, `RateLimit`,
  `Quota`, `Provider`, `Transport`, `Unsupported`.

### Feature flags

| Feature | What it adds |
|---|---|
| `registry` | Provider trait, registry, capability types; no storage |
| `sqlx` | Optional Postgres-backed `TokenStore` |
| `memory` | In-memory `TokenStore` for tests and single-binary tools |
| `axum` | Optional extractor for tenant-scoped handlers |

---

## What this crate deliberately does not contain

This is the part that keeps the crate publishable and stable:

| Excluded | Why | Where it belongs |
|---|---|---|
| Capability key vocabulary | Product-specific and changes often | The app's adapter crate |
| OAuth flows | Provider-specific, already in `wirekit` | `wirekit` modules |
| Quota ledger client | Owns state | `quota-ledger` |
| Webhook ingress client | Owns delivery state | `hookgate` |
| KMS / envelope encryption | Deployment-specific | The app's adapter crate |
| Product domain types | Publishing them is a permanent mistake | The app |

A trait crate can stay at 1.0 for years. A crate that also owns the product's
capability strings, storage, and policy cannot.

---

## Integration

1. Each provider family implements `Provider` and returns its capability slice.
2. The app's **adapter crate** implements `TokenStore` against its KMS and
   registry vocabulary.
3. The API builds a `Registry` at startup from the configured families.
4. Handlers resolve the tenant, then ask the registry for capabilities before
   rendering an action.

The public crate never learns a product's nouns. The adapter never re-implements
transport.

---

## Test plan

- Registry returns `None` for an unregistered provider and never panics.
- Capability lookup is stable across process restarts.
- `TokenStore` contract suite passes for `memory` and any external backend.
- A `TokenStore` implementation is tenant-isolated: no key can read another
  tenant's token.
- `IntegrationError` mapping covers every `wirekit` error variant.
- The crate compiles with `--no-default-features` and no storage backend.

---

## Open questions

1. **Crate name.** `platform-core` is free on crates.io as of October 2026, but
   it is generic. Confirm before publish; `provider-core` is an alternative.
2. **Capability keys.** `&'static str` is simple but invites drift. Ship an
   opaque newtype instead?
3. **Tenant model.** `Uuid` is an assumption; should `TenantId` be generic?
4. **Secrets type.** Depend on `secrecy` for `Secret`, or define the trait over
   an associated type?
