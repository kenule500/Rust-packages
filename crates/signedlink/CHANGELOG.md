# Changelog

All notable changes to `signedlink` are documented here.

## 0.1.0 - unreleased

- HMAC-SHA256 claims signing with URL-safe base64 and constant-time verification.
- Expiry and clock-skew checks with a caller-supplied verification instant.
- Base62 codes from OS entropy with rejection sampling and bounded collision
  retry.
- Closed-set UTM normalisation with deterministic canonical ordering.
- Explicit attribution window with no default.
- Mandatory `LinkStore` contract for revocation and atomic single use.
- Redacted `SigningKey`, `SignedToken`, and `SignedUrl` formatting.
- Optional `axum` redirect handler with no-referrer and no-store headers.
