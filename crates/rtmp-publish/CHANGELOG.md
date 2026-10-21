# Changelog

All notable changes to `rtmp-publish` are documented here.

## 0.1.0 - unreleased

- Destination model for RTMP, RTMPS, SRT, and WHIP with an optional backup chain.
- Hand-written redaction for `SecretString` and `Destination`; no `Serialize`
  for keys.
- Pre-connection validation of scheme, host, port, key, and credentials in URLs.
- TCP probes and an optional `reqwest` HTTP probe for WHIP.
- Bounded exponential backoff with injected jitter.
- Per-destination circuit breaker with open, half-open, and closed states.
- Backup promotion and cool-down demotion with self-reference rejection.
- Bitrate hints clamped to a configured floor and ceiling.
