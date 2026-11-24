//! AC-002: the provider matrix and crate index are generated from the catalogue
//! and the crate manifests, so a status change in the source is reflected, and a
//! stale hand-written status fails the drift check.

mod support;

use support::{assert_node_ok, node, path_of, read, table_row, Mutation};

#[test]
fn ac_002_matrix_is_generated_from_source_and_drift_fails() {
    // 1. The committed pages are exactly what a fresh generation produces.
    assert_node_ok("scripts/docs-matrix.js", &["--check"]);

    // 2. A provider's status is read from the source, not asserted by hand: the
    //    matrix says what the manifest and the module files say.
    let matrix = read("docs/src/provider-matrix.md");

    let implemented = table_row(&matrix, "wirekit::twilio");
    assert!(
        implemented.ends_with("| implemented |"),
        "twilio must be reported implemented, got: {implemented}"
    );
    assert!(
        path_of("crates/wirekit/src/twilio.rs").is_file(),
        "twilio is reported implemented but its module source is missing"
    );

    let feature_only = table_row(&matrix, "wirekit::adp");
    assert!(
        feature_only.ends_with("| feature declared, no module |"),
        "adp must be reported as a feature with no module, got: {feature_only}"
    );
    assert!(
        !path_of("crates/wirekit/src/adp.rs").exists(),
        "adp is reported without a module but crates/wirekit/src/adp.rs exists"
    );

    let documented_only = table_row(&matrix, "wirekit::google_business");
    assert!(
        documented_only.ends_with("| documented only |"),
        "google_business must be reported as documented only, got: {documented_only}"
    );

    // 3. The crate index reports the workspace, including the version it declares.
    let crates = read("docs/src/crates.md");
    let wirekit = table_row(&crates, "[`wirekit`]");
    assert!(
        wirekit.contains("| 0.1.0 |"),
        "the crate index must report wirekit's declared version, got: {wirekit}"
    );
    assert!(
        crates.contains("## Catalogue packages with no crate yet"),
        "the crate index must separate catalogued packages from crates that exist"
    );

    // 4. A hand-written status is drift, and the check names the stale page. The
    //    mutation is undone by `Mutation::drop`, so a failing assertion below
    //    cannot leave the working tree dirty.
    let mutation = Mutation::append("docs/src/crates.md", "\n| hand-written status |\n");
    let drift = node("scripts/docs-matrix.js", &["--check"]);
    assert!(
        !drift.status.success(),
        "a hand-edited generated page must fail the drift check"
    );
    let stderr = String::from_utf8_lossy(&drift.stderr);
    assert!(
        stderr.contains("docs/src/crates.md") && stderr.contains("stale"),
        "the drift check must name the stale page, got:\n{stderr}"
    );
    drop(mutation);

    // 5. With the edit reverted the tree is current again.
    assert_node_ok("scripts/docs-matrix.js", &["--check"]);
}
