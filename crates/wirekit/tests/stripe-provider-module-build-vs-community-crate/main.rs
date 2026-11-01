//! Acceptance tests for `stripe-provider-module-build-vs-community-crate`
//! (AC-001 to AC-005).
//!
//! This feature is a decision record, so the tests assert that the decision is
//! recorded, evidenced, and published where a developer will look — and that no
//! Stripe code was added to `wirekit`. Every test name carries its AC id.

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the wirekit crate sits two levels below the repository root")
        .to_path_buf()
}

fn read(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

fn spec() -> String {
    read(".sdrive/specs/ongoing/stripe-provider-module-build-vs-community-crate/v1/spec.md")
}

fn wirekit_readme() -> String {
    read("crates/wirekit/README.md")
}

#[test]
fn ac_001_both_options_are_scored_against_the_criteria() {
    let spec = spec();

    for expected in [
        "Option A",
        "Option B",
        "C1",
        "C2",
        "C3",
        "C4",
        "C5",
        "C6",
        "async-stripe",
        "stripe-rust",
    ] {
        assert!(spec.contains(expected), "the record scores {expected}");
    }
}

#[test]
fn ac_002_the_chosen_option_and_approver_are_named() {
    let spec = spec();

    assert!(
        spec.contains("Chosen option") && spec.contains("Option B"),
        "the record names the chosen option"
    );
    assert!(
        spec.contains("Approver") && spec.contains("kenule500"),
        "the record names the approver"
    );
}

#[test]
fn ac_003_the_rejected_build_option_records_the_design_file_rule() {
    let spec = spec();

    assert!(
        spec.contains("rejected") || spec.contains("Rejected"),
        "the build option is recorded as rejected"
    );
    assert!(
        spec.contains("CON-105"),
        "the design-file rule that a build would trigger is recorded"
    );
}

#[test]
fn ac_004_the_community_crate_is_pinned_with_licence_and_msrv() {
    let spec = spec();
    let readme = wirekit_readme();

    for expected in [
        "async-stripe",
        "0.41.0",
        "MIT OR Apache-2.0",
        "1.82.0",
        "2025-04-08",
    ] {
        assert!(spec.contains(expected), "the record pins {expected}");
    }

    assert!(
        readme.contains("Payments (Stripe)") && readme.contains("async-stripe"),
        "the wirekit README points at the documented crate"
    );
    assert!(
        !repo_root().join("crates/wirekit/src/stripe.rs").exists(),
        "no stripe module may exist in wirekit"
    );
}

#[test]
fn ac_005_the_revisit_triggers_are_explicit() {
    let spec = spec();
    let readme = wirekit_readme();

    // AC-005 names revisit triggers. The spec states them in its AC text; the
    // wirekit README repeats them for consumers.
    assert!(
        spec.contains("async-stripe 1.0 stable") && spec.contains("unmaintained"),
        "the record names its revisit triggers"
    );
    assert!(
        readme.contains("Revisit"),
        "the consumer-facing guidance repeats the triggers"
    );
    assert!(
        spec.contains("unmaintained"),
        "the unmaintained trigger is stated"
    );
}
