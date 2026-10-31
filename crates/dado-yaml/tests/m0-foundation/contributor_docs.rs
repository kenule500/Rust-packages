//! Acceptance tests for AC-022 through AC-027: contributor, security,
//! template, licence, and release-runbook surfaces.

use crate::common::{assert_contains, assert_contains_all, read_repo};

#[test]
fn ac_022_contributing_documents_the_full_contribution_path() {
    let doc = read_repo("CONTRIBUTING.md");

    assert_contains_all(
        &doc,
        &[
            "## Prerequisites",
            "Rust 1.85 or newer",
            "## Build and test",
            "cargo build --workspace --all-features",
            "cargo test --workspace --all-features",
            "cargo fmt --all --check",
            "cargo clippy --workspace --all-features -- -D warnings",
            "## Branches and commits",
            "feature/<name>",
            "specdrive(<command>): <summary>",
            "## SpecDrive workflow",
            "## Pull request checklist",
            "No Contributor License Agreement (CLA)",
            "Developer Certificate of Origin",
        ],
        "CONTRIBUTING.md",
    );
}

#[test]
fn ac_023_security_documents_private_reporting_and_response_targets() {
    let doc = read_repo("SECURITY.md");

    assert_contains_all(
        &doc,
        &[
            "## Supported versions",
            "0.1.x",
            "GitHub private vulnerability reporting",
            "security/advisories/new",
            "Do **not** open a public issue",
            "## Response targets",
            "Acknowledgement",
            "Initial triage and severity assessment",
            "keep the report private",
        ],
        "SECURITY.md",
    );
}

#[test]
fn ac_024_bug_and_feature_issue_forms_are_available() {
    let bug = read_repo(".github/ISSUE_TEMPLATE/bug_report.yml");
    let feature = read_repo(".github/ISSUE_TEMPLATE/feature_request.yml");
    let config = read_repo(".github/ISSUE_TEMPLATE/config.yml");

    assert_contains_all(
        &bug,
        &[
            "name: Bug report",
            "labels: [\"bug\"]",
            "Minimal reproduction",
            "validations:",
        ],
        "bug_report.yml",
    );
    assert_contains_all(
        &feature,
        &[
            "name: Feature request",
            "labels: [\"enhancement\"]",
            "Proposed solution",
            "validations:",
        ],
        "feature_request.yml",
    );
    assert_contains_all(
        &config,
        &[
            "blank_issues_enabled: false",
            "Report a security vulnerability",
            "security/advisories/new",
        ],
        "ISSUE_TEMPLATE/config.yml",
    );
}

#[test]
fn ac_025_pull_request_template_requests_summary_links_evidence_and_checklist() {
    let template = read_repo(".github/pull_request_template.md");

    assert_contains_all(
        &template,
        &[
            "## Summary",
            "## Linked work",
            "Backlog / spec:",
            "## Test evidence",
            "cargo test --workspace --all-features",
            "## Checklist",
            "No secrets, tokens, credentials, or generated build output",
        ],
        "pull_request_template.md",
    );
}

#[test]
fn ac_026_licence_files_exist_and_match_the_declared_dual_licence() {
    let mit_root = read_repo("LICENSE-MIT");
    let apache_root = read_repo("LICENSE-APACHE");
    let mit_crate = read_repo("crates/dado-yaml/LICENSE-MIT");
    let apache_crate = read_repo("crates/dado-yaml/LICENSE-APACHE");
    let workspace = read_repo("Cargo.toml");
    let crate_manifest = read_repo("crates/dado-yaml/Cargo.toml");

    assert_contains(&mit_root, "MIT License", "root LICENSE-MIT");
    assert_contains(
        &mit_root,
        "Permission is hereby granted",
        "root LICENSE-MIT",
    );
    assert_contains(&apache_root, "Apache License", "root LICENSE-APACHE");
    assert_contains(
        &apache_root,
        "Version 2.0, January 2004",
        "root LICENSE-APACHE",
    );
    assert_eq!(mit_root, mit_crate, "crate-local LICENSE-MIT drifted");
    assert_eq!(
        apache_root, apache_crate,
        "crate-local LICENSE-APACHE drifted"
    );
    assert_contains(
        &workspace,
        "license = \"MIT OR Apache-2.0\"",
        "workspace licence",
    );
    assert_contains(
        &crate_manifest,
        "license.workspace = true",
        "crate licence inheritance",
    );
}

#[test]
fn ac_027_release_runbook_covers_the_full_manual_release() {
    let doc = read_repo("RELEASING.md");

    assert_contains_all(
        &doc,
        &[
            "## 1. Pre-flight checks",
            "## 2. Version and changelog",
            "## 3. Package and dry-run",
            "cargo publish -p dado-yaml --dry-run",
            "## 4. Publish",
            "## 5. Tag the release",
            "dado-yaml-v0.1.0",
            "## 6. Post-publish verification",
            "cargo add dado-yaml@0.1.0 --features yaml",
            "## 7. Yank or rollback",
            "cargo yank --version 0.1.0 dado-yaml",
            "cargo yank --version 0.1.0 --undo dado-yaml",
        ],
        "RELEASING.md",
    );
}
