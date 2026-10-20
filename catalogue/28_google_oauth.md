# 28 — Google OAuth

**Provider:** Google OAuth 2.0
**Package:** `wirekit` (module `wirekit::google_oauth`)
**Status:** provider design, not a crate
**Covers:** Shared OAuth2 helper for Google APIs
**Base URL:** `https://oauth2.googleapis.com`
**Auth:** OAuth2 Authorization Code + PKCE

## What it does

Google OAuth is not a business integration; it is the shared auth helper for
Google Business Profile, Google LSA, Gmail, and any future Google API. It
generates authorization URLs, exchanges codes, refreshes tokens, and revokes
them.

The module wraps Google's OAuth2 endpoints with a typed authorization request,
token response, and token store interface. It is used by other wirekit modules
so each one does not re-implement OAuth.

## GIA use

- Tool 9/11-adjacent auth for Google Business Profile and Google LSA
- Tenant onboarding: connect a Google account
- Token refresh for review sync and LSA lead capture
- Revocation on offboarding

## Surface to build

| Type | Purpose |
|---|---|
| `GoogleOAuthClient` | Client ID/secret, redirect URI, scopes |
| `AuthorizationRequest` | URL, state, PKCE challenge |
| `TokenResponse` | Access, refresh, expiry, scope |
| `TokenStore` | Trait for encrypted persistence |
| `RevokeRequest` | Revoke a token |

Methods: build authorization URL, exchange code, refresh token, revoke token,
validate state/PKCE.

## Wirekit contract

- **Auth:** Authorization Code + PKCE; never use the implicit flow.
- **Errors:** Google's `error`/`error_description`; map to
  `WirekitError::Auth` or `Api` as appropriate.
- **Retry:** token refresh on 401 once; do not loop.
- **Rate limits:** token endpoint quotas; cache tokens until expiry minus a
  safety margin.
- **Webhooks:** none.
- **Idempotency:** state and PKCE prevent code replay; store the state
  server-side with a short TTL.

## GIA integration

A tenant connects Google from settings. GIA stores the refresh token encrypted,
keyed by workspace and provider. Google Business and LSA modules request
tokens through this helper. On offboarding, GIA revokes the token and deletes
the record.

## Test plan

- Mock token endpoint; authorization URL and state/PKCE.
- Code exchange, refresh, expiry, revocation.
- Invalid state, reused code, wrong redirect URI.
- Scope changes and incremental consent.
- Token encryption at rest.

## Open questions

1. **One Google app or per-tenant?** A single GIA Google Cloud project is
   simpler; confirm verification and branding.
2. **Scopes.** Request the minimum scopes per integration; do not ask for
   Gmail access unless it is used.
3. **Verification.** Google may require app verification for sensitive scopes;
   budget the review time.
4. **Token storage.** Where are refresh tokens encrypted, and who can decrypt
   them? This is a security-critical decision.

## Spec

[21_Packages_To_Build.md](<../../../GIA - Systems/overview/Catalog/21_Packages_To_Build.md>) §26.
