//! Acceptance tests for the `m1-first-publish` feature.
//!
//! Every test name carries its acceptance-criterion id (`ac_001_..`), and the
//! modules follow the layout required by CON-406. Run the suite with
//! `cargo test --workspace --all-features` (CON-402).

mod common;

mod auth;
mod client;
mod crate_hygiene;
mod errors;
mod noaa;
mod observability;
mod pipeline;
mod rate_limit;
mod release;
mod retry_policy;
mod sendgrid;
mod slack;
mod twilio;
