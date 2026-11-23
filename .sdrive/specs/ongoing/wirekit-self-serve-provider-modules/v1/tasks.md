---
title: wirekit self-serve provider modules tasks
status: backlog
version: v1
created: 2026-11-08
updated: 2026-11-08
---

# wirekit self-serve provider modules tasks

> Production code only. Test authoring and execution belong to `/sdrive:test`.

## P1 - Feature wiring

- [ ] **T1.1 - Add six additive features and module gates** (`FR-001`, `FR-007` / `AC-001`, `AC-006`)
  Add `accuweather`, `lob`, `pagerduty`, `deepgram`, `yelp`, and `klarna` to the manifest, each enabling only its own module. Declare each `mod` behind its feature in `lib.rs` and document the feature table. Confirm the default build compiles none of them and each feature compiles exactly its module.

## P2 - Provider modules

- [ ] **T2.1 - Implement the AccuWeather module** (`FR-002`, `FR-003`, `FR-004` / `AC-001`, `AC-002`, `AC-004`)
  Implement `accuweather.rs` from `catalogue/08_accuweather.md`: location search, current conditions, and forecast, using the shared API-key auth, retry, and rate limiter.
- [ ] **T2.2 - Implement the Lob module** (`FR-002`, `FR-003`, `FR-005` / `AC-001`, `AC-002`, `AC-003`)
  Implement `lob.rs` from `catalogue/18_lob.md`: create and retrieve letters and postcards, verification, and the `error.message` envelope mapped into `WirekitError`.
- [ ] **T2.3 - Implement the PagerDuty module** (`FR-002`, `FR-003`, `FR-005` / `AC-001`, `AC-002`, `AC-003`)
  Implement `pagerduty.rs` from `catalogue/22_pagerduty.md`: incidents, alerts, and on-calls, with the body-level `error.message` envelope treated as an error on HTTP 200.
- [ ] **T2.4 - Implement the Deepgram module** (`FR-002`, `FR-003`, `FR-004` / `AC-001`, `AC-002`, `AC-004`)
  Implement `deepgram.rs` from `catalogue/24_deepgram.md`: transcription and project listing, using bearer auth and the shared retry policy.
- [ ] **T2.5 - Implement the Yelp module** (`FR-002`, `FR-003`, `FR-005` / `AC-001`, `AC-002`, `AC-003`)
  Implement `yelp.rs` from `catalogue/19_yelp.md`: business search and reviews. If the catalogue marks the leads endpoint partner-only, withdraw that surface with a citation instead of faking it.
- [ ] **T2.6 - Implement the Klarna module** (`FR-002`, `FR-003`, `FR-004` / `AC-001`, `AC-002`, `AC-004`)
  Implement `klarna.rs` from `catalogue/25_klarna.md`: session and order calls with the shared auth and rate limiter.

## P3 - Contract, docs, verification

- [ ] **T3.1 - Verify the shared contract across the six modules** (`FR-002`, `FR-004`, `FR-006`, `NFR-001`, `NFR-003`, `NFR-004` / `AC-002`, `AC-004`, `AC-005`, `AC-007`)
  Confirm each module maps into `WirekitError`, honours `Retry-After`, never retries other 4xx, redacts its secret, validates provider JSON into typed values, and emits the shared tracing events. Update the crate docs and README provider table, and state that partner-gated providers are out of scope.
- [ ] **T3.2 - Run the verification checks** (`NFR-001`, `NFR-002`, `FR-007` / `AC-006`, `AC-007`)
  Run `cargo fmt --all --check`, `cargo clippy --workspace --all-features -- -D warnings`, `cargo doc --workspace --all-features --no-deps` under `RUSTDOCFLAGS=-D warnings`, `cargo check -p wirekit` with default features and with all features, and `node .sdrive/scripts/validate-governance.js`.
