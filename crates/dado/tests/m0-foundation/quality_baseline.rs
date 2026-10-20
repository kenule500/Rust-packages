//! Acceptance tests for AC-014 through AC-021: the CI quality baseline.
//!
//! These tests assert the workflow contract. The commands themselves are run
//! by CI and by the test-phase evidence commands; a workflow file cannot
//! execute GitHub Actions locally.

use crate::common::{assert_contains, assert_contains_all, assert_not_contains, read_repo};

fn ci_workflow() -> String {
    read_repo(".github/workflows/ci.yml")
}

#[test]
fn ac_014_ci_runs_every_required_job_on_push_and_pull_request() {
    let ci = ci_workflow();

    assert_contains_all(&ci, &["on:", "  push:", "  pull_request:"], "CI triggers");
    assert_contains_all(
        &ci,
        &[
            "  fmt:",
            "  clippy:",
            "  docs:",
            "  package:",
            "  test:",
            "  msrv:",
        ],
        "CI jobs",
    );
    assert_contains_all(
        &ci,
        &[
            "os: [ubuntu-latest, windows-latest, macos-latest]",
            "runs-on: ${{ matrix.os }}",
            "cargo test --workspace --all-features",
        ],
        "CI matrix",
    );
    assert_contains(&ci, "contents: read", "workflow permissions");
}

#[test]
fn ac_015_format_job_fails_on_a_formatting_violation() {
    let ci = ci_workflow();

    assert_contains(&ci, "  fmt:", "format job");
    assert_contains(&ci, "cargo fmt --all --check", "format command");
    assert_not_contains(&ci, "continue-on-error: true", "format job");
}

#[test]
fn ac_016_clippy_job_denies_warnings() {
    let ci = ci_workflow();

    assert_contains(&ci, "  clippy:", "clippy job");
    assert_contains(
        &ci,
        "cargo clippy --workspace --all-features -- -D warnings",
        "clippy command",
    );
    assert_not_contains(&ci, "continue-on-error: true", "clippy job");
}

#[test]
fn ac_017_test_jobs_run_the_workspace_suite_on_all_three_operating_systems() {
    let ci = ci_workflow();

    assert_contains(&ci, "  test:", "test job");
    assert_contains(
        &ci,
        "os: [ubuntu-latest, windows-latest, macos-latest]",
        "test matrix",
    );
    assert_contains(&ci, "cargo test --workspace --all-features", "test command");
}

#[test]
fn ac_018_msrv_job_uses_rust_1_85() {
    let ci = ci_workflow();

    assert_contains(&ci, "  msrv:", "MSRV job");
    assert_contains(&ci, "dtolnay/rust-toolchain@1.85.0", "MSRV toolchain");
    assert_contains(&ci, "cargo build --workspace --all-features", "MSRV build");
    assert_contains(
        &ci,
        "cargo test --workspace --all-features",
        "MSRV test command",
    );
}

#[test]
fn ac_019_docs_job_denies_rustdoc_warnings() {
    let ci = ci_workflow();

    assert_contains(&ci, "  docs:", "docs job");
    assert_contains(&ci, "RUSTDOCFLAGS: -D warnings", "rustdoc flags");
    assert_contains(
        &ci,
        "cargo doc --workspace --all-features --no-deps",
        "docs command",
    );
}

#[test]
fn ac_020_package_job_runs_before_any_publish() {
    let ci = ci_workflow();

    assert_contains(&ci, "  package:", "package job");
    assert_contains(&ci, "cargo package --workspace", "package command");
    assert_not_contains(&ci, "cargo publish", "CI publish step");
}

#[test]
fn ac_021_concurrency_cancels_older_runs_on_the_same_ref() {
    let ci = ci_workflow();

    assert_contains(&ci, "concurrency:", "concurrency block");
    assert_contains(
        &ci,
        "group: ${{ github.workflow }}-${{ github.ref }}",
        "concurrency group",
    );
    assert_contains(&ci, "cancel-in-progress: true", "concurrency cancellation");
}
