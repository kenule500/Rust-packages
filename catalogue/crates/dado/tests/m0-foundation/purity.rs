//! Acceptance tests for AC-035 through AC-037: I/O purity, feature
//! independence, and crate-level lints.

use crate::common::{
    assert_contains, assert_contains_all, assert_not_contains, read_crate, read_repo,
};

#[test]
fn ac_035_dependency_tree_has_no_network_filesystem_or_async_runtime() {
    let manifest = read_crate("Cargo.toml");
    let dependencies = manifest
        .split("[dependencies]")
        .nth(1)
        .expect("Cargo.toml must declare dependencies")
        .split("\n[")
        .next()
        .expect("dependency section");

    for line in dependencies.lines().map(str::trim) {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let name = line
            .split('=')
            .next()
            .expect("dependency line")
            .trim()
            .trim_matches('"');
        assert!(
            matches!(name, "serde" | "winnow" | "indexmap"),
            "unexpected dependency: {name}"
        );
    }

    for forbidden in [
        "tokio",
        "reqwest",
        "hyper",
        "async-std",
        "async_std",
        "ureq",
        "curl",
    ] {
        assert_not_contains(&manifest, forbidden, "Cargo.toml");
    }

    for source in [
        "src/lib.rs",
        "src/yaml.rs",
        "src/diag.rs",
        "src/graph.rs",
        "src/emit.rs",
        "src/glob.rs",
    ] {
        let code = read_crate(source);
        for forbidden in [
            "std::fs",
            "std::net",
            "std::process",
            "reqwest",
            "tokio",
            "async_std",
            "hyper",
            "ureq",
            "TcpStream",
            "TcpListener",
            "File::",
        ] {
            assert_not_contains(&code, forbidden, source);
        }
    }
}

#[test]
fn ac_036_features_are_additive_and_yaml_is_independent_of_diag() {
    let manifest = read_crate("Cargo.toml");
    let lib = read_crate("src/lib.rs");

    assert_contains(&manifest, "default = []", "feature defaults");
    assert_contains(
        &manifest,
        "yaml = [\"dep:serde\", \"dep:winnow\", \"dep:indexmap\"]",
        "yaml feature",
    );
    assert_contains(&manifest, "diag = []", "diag feature");
    assert_contains(&manifest, "graph = []", "graph feature");
    assert_contains(&manifest, "emit = []", "emit feature");
    assert_contains(&manifest, "glob = []", "glob feature");

    let yaml_feature_line = manifest
        .lines()
        .find(|line| line.trim_start().starts_with("yaml = "))
        .expect("yaml feature line");
    assert_not_contains(yaml_feature_line, "diag", "yaml feature");

    assert_contains_all(
        &lib,
        &[
            "#[cfg(feature = \"yaml\")]",
            "#[cfg(feature = \"diag\")]",
            "#[cfg(feature = \"graph\")]",
            "#[cfg(feature = \"emit\")]",
            "#[cfg(feature = \"glob\")]",
        ],
        "src/lib.rs module gates",
    );
}

#[test]
fn ac_037_crate_root_forbids_unsafe_code_and_denies_missing_docs() {
    let lib = read_crate("src/lib.rs");

    assert_contains(&lib, "#![forbid(unsafe_code)]", "src/lib.rs");
    assert_contains(&lib, "#![deny(missing_docs)]", "src/lib.rs");
    assert_contains(
        &read_repo("catalogue/Cargo.toml"),
        "edition = \"2021\"",
        "workspace edition",
    );
}
