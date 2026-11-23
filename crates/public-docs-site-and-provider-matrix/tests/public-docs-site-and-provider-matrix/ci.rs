//! AC-004: on a pull request the workflow builds the book and runs the drift and
//! link checks, and the job fails if either does.

mod support;

use support::{read, yaml_block};

#[test]
fn ac_004_ci_builds_the_book_and_runs_checks_on_pull_requests() {
    let workflow = read(".github/workflows/ci.yml");

    assert!(
        workflow.contains("  pull_request:"),
        "the workflow must trigger on pull_request, not only on push"
    );

    // The docs job is the one that runs the site build. Finding it by name keeps
    // this test honest: another job adding `mdbook build` elsewhere would not
    // satisfy it.
    let book = yaml_block(&workflow, "book").expect("ci.yml must define a `book` docs job");

    assert!(
        book.contains("node scripts/docs-check.js"),
        "the docs job must run the drift and link checks, got:\n{book}"
    );
    assert!(
        book.contains("mdbook build docs"),
        "the docs job must build the book, got:\n{book}"
    );
    assert!(
        !book.contains("continue-on-error"),
        "the docs job must be able to fail the pull request, got:\n{book}"
    );
    assert!(
        !book.contains("if: false") && !book.contains("if: ${{ false }}"),
        "the docs job must not be disabled, got:\n{book}"
    );

    // RISK-001 and NFR-003: the mdBook version is pinned rather than installed
    // from source, so the job stays inside the time budget and stays reproducible.
    assert!(
        workflow.contains("MDBOOK_VERSION"),
        "the mdBook version must be pinned, not resolved at run time"
    );
    assert!(
        !workflow.contains("cargo install mdbook"),
        "the docs job must use a prebuilt mdBook; `cargo install mdbook` is too slow for CI"
    );
}
