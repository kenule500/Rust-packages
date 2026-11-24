//! Acceptance tests for `wirekit-self-serve-provider-modules` v1.
//!
//! One test per acceptance criterion (AC-001 to AC-007), with every test name
//! carrying its AC id (CON-405). Everything runs against a loopback `wiremock`
//! server, so no provider sandbox or live credential is needed; PagerDuty and
//! Klarna expose host overrides for exactly this transport.

mod accuweather;
mod common;
mod contract;
mod deepgram;
mod klarna;
mod lob;
mod pagerduty;
mod yelp;
