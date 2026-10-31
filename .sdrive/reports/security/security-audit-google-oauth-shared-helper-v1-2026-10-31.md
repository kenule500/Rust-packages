# Security Audit Report: Google OAuth shared helper (v1)

## 1. Executive Summary
- **Date:** 2026-10-31
- **Auditor:** AI Security Agent
- **Feature Version:** v1
- **Overall Risk Level:** Low
- **Total Vulnerabilities Found:** 1 (Critical: 0, High: 0, Med: 0, Low: 1)

One finding was raised and **fixed before this audit concluded**: the `Debug`
implementation of `AuthorizationRequest` printed the authorization URL verbatim,
and that URL carries `state`. The value is now masked. No open critical or high
finding remains.

## 2. Threat Model Summary
- **Key Assets:** the OAuth client secret, the PKCE `code_verifier`, the `state`
  value, the authorization code, access tokens, refresh tokens, and the records
  a caller keeps in its `TokenStore`.
- **Trust Boundaries:**
  1. caller application ↔ `wirekit::google_oauth` — the caller supplies every
     credential and the clock; the module never reads a file, an environment
     variable, or a database (CON-108, CON-519).
  2. `wirekit::google_oauth` ↔ Google endpoints — HTTPS only, with a loopback
     exception for tests (CON-520).
  3. caller ↔ `TokenStore` — persistence and encryption at rest are the
     implementor's contract; the crate ships only a non-durable memory store.
  4. browser ↔ authorization endpoint — outside the crate; the `state` value is
     the crate's contribution to that boundary.
- **Attacker capabilities considered:** read access to application logs and
  traces, control of provider responses and error bodies, a replayed or forged
  callback, and a hostile redirect URI.

## 3. Vulnerability Details

### VULN-001: OAuth `state` disclosed through `Debug`
- **CVSS Score:** 3.8 (Low) — `CVSS:3.1/AV:L/AC:H/PR:L/UI:N/S:U/C:L/I:L/A:N`
- **Location:** `crates/wirekit/src/google_oauth.rs`, `impl Debug for AuthorizationRequest`
- **Description:** `Debug` rendered the authorization URL with `url.as_str()`. The
  URL necessarily contains `?state=<value>`, so the CSRF token appeared in full
  next to a `state: ***` field that existed to redact it. Any `{:?}` of a request
  — a panic message, a trace, a support log — exposed the value.
- **Impact:** an attacker able to read logs could observe an in-flight `state`
  and combine it with a stolen authorization code to complete a callback the
  legitimate session did not initiate. Preconditions (log access, an interceptable
  code, the short single-use window) keep the severity Low.
- **Remediation:** the URL is now rendered through `redact_state`, which masks the
  `state` parameter and leaves every other parameter readable. Applied in
  `specdrive(apply): mask the OAuth state in AuthorizationRequest Debug` (`1f80712`).
- **Status:** Fixed, covered by TEST-008.

*No other vulnerability met the reporting bar. The items below are recorded
observations, not findings.*

| Observation | Assessment |
|---|---|
| `MemoryTokenStore` is non-durable and unencrypted | Documented in rustdoc; intended for tests and single-process callers. Durable stores must encrypt refresh tokens at rest and lock per record. Risk accepted by design (RISK-002). |
| `Secret<String>` does not zeroize on drop | Pre-existing crate-wide decision, documented as a non-goal of `secret.rs`; not introduced here. |
| Token-endpoint rate limiting depends on the caller's bucket configuration | The request is always charged to the `google-oauth` key; whether a bucket exists is the caller's `WirekitClient` configuration, consistent with every other module (NFR-003). |
| `with_endpoints` accepts plain-HTTP loopback | Intentional test affordance, rejected for any non-loopback host, and the shared client must additionally opt in via `allow_insecure_localhost` (CON-520, EC-007). |
| CI `MSRV (1.85)` job is red | Caused by `quick-xml@0.42.0` requiring rustc 1.86 (Sage Intacct dependency). It already fails on `main`; this feature adds no dependency. `cargo +1.85.0 check --features 'google-lsa google-oauth'` passes in isolation. |

## 4. Security Baseline

| Control | Status | Evidence |
|---|---|---|
| CON-511 Authentication | PASS | Credentials are constructor arguments only; `client_secret` and every token live in `Secret<String>` and render `***`. No default, embedded, or environment-sourced credential exists. |
| CON-512 Authorization / isolation | PASS | No global or static cache; every `GoogleOAuthClient` owns its own state, and `TokenStore` keys are caller-chosen. Nothing is shared between instances. |
| CON-513 Security headers | NOT APPLICABLE | No server or response pipeline. |
| CON-514 Rate limiting | PASS | `post_token` and `revoke` set `rate_limit_key("google-oauth")`, so the shared bucket and a caller override apply (NFR-003). |
| CON-515 Input validation | PASS | Provider error bodies decode into a typed `WireError`; bodies are size-bounded by the core client; `state` is compared in constant time; malformed JSON becomes `WirekitError::Decode`; no panic path. |
| CON-516 Injection | NOT APPLICABLE | No database, cache, or query surface. |
| CON-517 XSS | NOT APPLICABLE | No markup is produced. |
| CON-518 CSRF | NOT APPLICABLE | No browser session or cookie state; `state` is validated per callback, which is the crate's contribution to CSRF defence. |
| CON-519 Environment security | PASS | No `.env`, no `std::env`, no file or database access; verified by TEST-009's source scan and by inspection. |
| CON-520 HTTPS | PASS | Endpoints are parsed at construction; anything that is not HTTPS is rejected unless it is loopback (EC-007). |

Traceability cross-check: NFR-001, NFR-002 and NFR-003 carry the required
security links and are `complete` in `traceability.json`. No traceability gap.

## 5. Security Regression Verification
- [x] Secret scan passed — manual pattern scan over every changed file
  (`AKIA`, `BEGIN PRIVATE KEY`, `password =`, `api_key =`): no match.
- [x] Functional tests passed — `cargo test --workspace --all-features`, exit 0,
  156 passed / 0 failed / 2 ignored.
- [x] MSRV for this feature — `cargo +1.85.0 check -p wirekit --no-default-features
  --features 'google-lsa google-oauth'` passes.
- **Limitations:**
  - **Dependency audit NOT run** — `cargo audit` is not installed in this
    environment. No new dependency is introduced by this feature, so the graph is
    unchanged from `main`, but advisory scanning is unverified.
  - No dynamic analysis (fuzzing, live-token testing); no live Google account was
    used, by design.
  - No OWASP ZAP or similar tool applies: the crate has no server surface.

## 6. Recommendations
1. Add `cargo audit` to CI so dependency advisories are checked on every push.
2. Consider `zeroize` for `Secret<T>` crate-wide, including refresh tokens.
3. Document, with a worked example, the durable `TokenStore` contract: encryption
   at rest plus a per-record lock, so two processes cannot refresh concurrently.
4. Pin `quick-xml` to a 1.85-compatible version, or raise the workspace MSRV, so
   the MSRV job stops failing for every branch.
5. Keep the `state` redaction covered: TEST-008 is the regression guard.

