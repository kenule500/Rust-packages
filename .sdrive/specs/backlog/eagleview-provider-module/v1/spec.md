---
title: EagleView provider module
status: backlog
version: v1
created: 2026-11-01
updated: 2026-11-01
---

# EagleView provider module

## Overview

- **Problem Statement**: Roof measurement is an asynchronous, per-report paid provider call, and `wirekit` has no EagleView module. Every integration re-implements the order lifecycle, polling with backoff, PDF download, and — most expensively — its own idempotency, so a retried order becomes a duplicate charge. Addresses that are not measurable and reports that fail need to be distinguishable from transport errors, or the caller either retries a rejection or silently gives up on a valid property.
- **Proposed Solution**: Add an `eagleview` feature-flagged module to `wirekit` exposing `EagleViewClient`, `MeasurementRequest`, `MeasurementOrder`, `MeasurementReport`, `RoofSegment`, `RoofPitch`, and `RoofArea`, per `catalogue/12_eagleview.md`. It covers OAuth2 client-credentials tokens, order creation with a caller idempotency key, status polling with bounded backoff, report fetch, PDF download with content-type and size validation, and listing reports for a property, reusing the crate's client, retry, rate-limit, secret, and error types.
- **Target Audience**: Rust developers producing preliminary roofing estimates, including the estimation agent in a consuming application.
- **Assumptions**:
  - Commercial contract, API credentials, and report pricing are `[to be confirmed]`; the module assumes the caller already holds approved credentials and a commercial agreement.
  - The order lifecycle is asynchronous: order, then status, then report. No status webhook is assumed to exist until the contract confirms one (RISK-002).
  - Each report is a real cost; the module never orders without an explicit caller acknowledgement of the cost.
  - Turnaround time is unknown; the module exposes status rather than blocking.
  - No external research was performed beyond the repository catalogue and constitution (CON-704).

## User Stories

### US-001: Authenticate with client credentials
As a developer, I want the module to obtain and cache a token, so that calls do not re-authenticate every time.

**Acceptance Criteria:**
- AC-001: Given a client ID, client secret, and API base, when the client is built, then both credentials are held in `Secret` and `Debug` renders no plaintext.
- AC-002: Given a cached token that is still valid, when a call is made, then no token request is sent, and an expired or soon-to-expire token is replaced before the call is attempted.

### US-002: Order a measurement
As a developer, I want to order a report, so that an address becomes measurements.

**Acceptance Criteria:**
- AC-003: Given an address, product, and options, when `order_measurement` is called, then a POST to the orders resource carries that request and returns an order carrying its provider id and initial status.
- AC-004: Given a caller idempotency key already used for the same request, when the order is submitted again, then the existing order is returned and no second order is created.
- AC-005: Given no explicit cost acknowledgement, when an order is attempted, then the module refuses locally without calling the provider.

### US-003: Poll the order lifecycle
As a developer, I want typed status, so that I know whether to wait, use the report, or stop.

**Acceptance Criteria:**
- AC-006: Given an order id, when `order_status` is called, then the response is mapped into a typed state distinguishing accepted, processing, complete, and failed.
- AC-007: Given a failed order, when its status is read, then the failure reason is surfaced and the module does not retry the order.

### US-004: Fetch and download the report
As a developer, I want the measurements, so that an estimate can be produced.

**Acceptance Criteria:**
- AC-008: Given a complete order, when `fetch_report` is called, then the report carries the total area, pitch, and the segment breakdown with each segment's type, area, and pitch.
- AC-009: Given a report with a document, when `download_pdf` is called, then the bytes are returned with the provider content type validated and the body bounded in size.

### US-005: List reports for a property
As a developer, I want existing reports, so that the same address is not ordered twice.

**Acceptance Criteria:**
- AC-010: Given an address, when `list_reports` is called, then the module returns the reports already produced for it, bounded by a caller-supplied limit.

### US-006: Distinguish provider failures
As a developer, I want typed errors, so that a rejection is never mistaken for a transport fault.

**Acceptance Criteria:**
- AC-011: Given a rejected order, an unmeasurable address, and a processing failure, when each is returned, then the caller can distinguish them by typed error code rather than by message text.
- AC-012: Given a 429 or 5xx, when the call is retried, then `Retry-After` is honoured, and a rejected order is never retried without being changed.

### US-007: Safe configuration
As a security reviewer, I want transport rules enforced, so that credentials never travel in cleartext.

**Acceptance Criteria:**
- AC-013: Given a non-loopback cleartext base URL, when the client is built, then it is rejected as invalid configuration, and the module reads no environment variables.

## Non-Functional Requirements

- NFR-001: Credentials and tokens MUST be held in `Secret` and MUST render redacted (`CON-511`, `CON-519`).
- NFR-002: Every request MUST be bounded in size and every response typed before use (`CON-515`).
- NFR-003: All traffic MUST use HTTPS; a cleartext base URL MUST be rejected at construction (`CON-520`).
- NFR-004: The module MUST add no runtime dependency: client-credentials OAuth2, JSON, and polling reuse what `wirekit` already compiles.
- NFR-005: No application-domain entity may be declared in `wirekit`; the module exposes provider schema types only (`AC-040`, `CON-106`).

## Edge Cases

- An address that resolves to multiple structures: the provider reports it unmeasurable rather than guessing.
- A report whose PDF is missing but whose measurements exist: measurements are usable, the download fails typed.
- An order polled past a caller-supplied deadline: the module returns the last known status rather than blocking forever.
- A repeated idempotency key with a different address: the module rejects the mismatch instead of returning the other order.

## Dependencies

- `BP-005` (wirekit core) is complete; this feature reuses its client, retry, rate limit, secret, and error types.
- **Commercial contract and API credentials `[to be confirmed]`** (RISK-001): tests run against a local mock, so the feature can be built and reviewed, but live verification needs the contract.

## Risks

- **RISK-001** EagleView commercial access unconfirmed → build against a mock and keep `[to be confirmed]` in the assumptions and the report.
- **RISK-002** no status webhook is assumed → poll with bounded backoff; if the contract confirms webhooks, add verification in a later version rather than guessing now.
- **RISK-003** cost per report unknown → the module requires an explicit cost acknowledgement, so an unpriced call cannot be made silently.
- **RISK-004** coverage gaps for rural or new-construction addresses → surfaced as a typed unmeasurable error, never as a generic failure.

