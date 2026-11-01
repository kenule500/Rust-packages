---
title: EagleView provider module — tasks
status: backlog
version: v1
created: 2026-11-01
updated: 2026-11-01
---

# Tasks: EagleView provider module

## P1 — Foundation

- [ ] **T1.1** Add the `eagleview` feature and module wiring — additive, default-off
      `eagleview = []` in `Cargo.toml`, `pub mod eagleview` behind `cfg`, no other
      provider enabled. Covers FR-001, NFR-004.
- [ ] **T1.2** Define `EagleViewConfig` and the client-credentials token cache —
      `client_secret` in `Secret`, cached `AccessToken`, refresh when expired,
      redacted `Debug`, cleartext refusal. Covers FR-002, FR-013, NFR-001, NFR-003.

## P2 — Ordering

- [ ] **T2.1** Define `MeasurementRequest` and `MeasurementOrder` and implement
      `order_measurement` — POST to `/v1/orders`, return the provider id and initial
      status. Covers FR-003.
- [ ] **T2.2** Idempotent ordering — a repeated caller key returns the existing
      order, and a repeated key with a different address is refused. Covers FR-004.
- [ ] **T2.3** Cost acknowledgement guard — refuse locally, with no provider call,
      when the caller has not acknowledged the cost. Covers FR-005.

## P3 — Lifecycle

- [ ] **T3.1** Map order status into `MeasurementStatus` — accepted, processing,
      complete, failed. Covers FR-006.
- [ ] **T3.2** Failed orders — surface the reason and never retry the order.
      Covers FR-007.

## P4 — Report

- [ ] **T4.1** Fetch the report — `MeasurementReport` with `RoofArea`, `RoofPitch`,
      and ordered `RoofSegment` values. Covers FR-008, NFR-005.
- [ ] **T4.2** Download the PDF — validate the content type and bound the body
      before returning bytes; a missing document is a typed error. Covers FR-009,
      NFR-002.
- [ ] **T4.3** List reports for an address, bounded by a caller-supplied limit.
      Covers FR-010.

## P5 — Errors, docs, verification

- [ ] **T5.1** Distinguish rejected orders, unmeasurable addresses, and processing
      failures by typed code, and honour `Retry-After` without retrying a rejection.
      Covers FR-011, FR-012.
- [ ] **T5.2** Document the module (rustdoc, README row and example, CHANGELOG) and
      run the gates — `cargo fmt --all --check`, `clippy -D warnings`, `cargo doc`
      with `-D warnings`, and `validate-governance.js`. Covers NFR-001 … NFR-005.
