//! Acceptance tests for the `m0-foundation` feature.
//!
//! Test names carry the acceptance-criterion ID (`ac_001`, `ac_002`, ...), and
//! the files follow the feature layout required by CON-406.

mod common;

#[cfg(feature = "yaml")]
mod yaml_errors;

#[cfg(feature = "yaml")]
mod yaml_values;

mod contributor_docs;
mod purity;
mod quality_baseline;
mod release_readiness;
