//! AC-046, AC-047, AC-048, AC-050, and AC-053: the workspace build matrix,
//! documentation lints, feature independence, and publishable package
//! metadata.
//!
//! Build-matrix ACs also have recorded evidence commands (see the test report):
//! `cargo build --workspace --all-features`, `cargo build -p <crate>
//! --no-default-features`, `cargo doc --workspace --all-features --no-deps`
//! with `RUSTDOCFLAGS=-D warnings`, and `cargo package --list` per crate.

use std::path::{Path, PathBuf};

const CRATES: [&str; 4] = ["platform-core", "quota-ledger", "hookgate", "syncforge"];

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("the crate is nested two levels below the repository root")
        .to_path_buf()
}

fn read_repo(relative: &str) -> String {
    let path = repo_root().join(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

fn manifest(crate_name: &str) -> String {
    read_repo(&format!("crates/{crate_name}/Cargo.toml"))
}

/// Parses single-line feature arrays from a manifest's `[features]` section.
fn features(manifest: &str) -> Vec<(String, Vec<String>)> {
    let mut in_features = false;
    let mut parsed = Vec::new();

    for line in manifest.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('[') {
            in_features = trimmed == "[features]";
            continue;
        }
        if !in_features || trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if let Some((name, value)) = trimmed.split_once('=') {
            let values = value
                .trim()
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|entry| entry.trim().trim_matches('"').to_owned())
                .filter(|entry| !entry.is_empty())
                .collect();
            parsed.push((name.trim().to_owned(), values));
        }
    }

    parsed
}

/// AC-046: the workspace builds all four crates and the example on Rust 1.85.
#[test]
fn ac_046_workspace_declares_the_four_crates_and_the_example() {
    let workspace = read_repo("Cargo.toml");
    assert!(
        workspace.contains("members = [\"crates/*\"]"),
        "AC-046: the workspace must include every crate under crates/"
    );
    assert!(
        workspace.contains("rust-version = \"1.85\""),
        "AC-046: the workspace must pin Rust 1.85"
    );
    assert!(
        workspace.contains("edition = \"2021\""),
        "AC-046: the workspace must use edition 2021"
    );

    for crate_name in CRATES {
        let manifest = manifest(crate_name);
        assert!(
            manifest.contains("version = \"0.1.0\""),
            "AC-046: {crate_name} must be version 0.1.0"
        );
        assert!(
            manifest.contains("rust-version.workspace = true"),
            "AC-046: {crate_name} must inherit the 1.85 MSRV"
        );
        assert!(
            manifest.contains("edition.workspace = true"),
            "AC-046: {crate_name} must inherit edition 2021"
        );
    }

    let example = read_repo("crates/hookgate/examples/substrate_integration.rs");
    for dependency in ["platform_core", "quota_ledger", "hookgate"] {
        assert!(
            example.contains(dependency),
            "AC-046: the integration example must compose {dependency}"
        );
    }
    for step in [
        "registry.register",
        "quota.reserve",
        "quota.commit",
        "verifier.verify",
        "insert_if_new",
    ] {
        assert!(
            example.contains(step),
            "AC-046: the example must exercise {step}"
        );
    }
}

/// AC-047: every crate forbids unsafe code, denies missing docs, and builds
/// rustdoc with warnings denied.
#[test]
fn ac_047_every_crate_enforces_documentation_lints() {
    for crate_name in CRATES {
        let lib = read_repo(&format!("crates/{crate_name}/src/lib.rs"));
        assert!(
            lib.contains("#![forbid(unsafe_code)]"),
            "AC-047: {crate_name} must forbid unsafe code at the crate root"
        );
        assert!(
            lib.contains("#![deny(missing_docs)]"),
            "AC-047: {crate_name} must deny missing docs at the crate root"
        );

        let manifest = manifest(crate_name);
        assert!(
            manifest.contains("[package.metadata.docs.rs]"),
            "AC-047: {crate_name} must configure docs.rs"
        );
        assert!(
            manifest.contains("all-features = true"),
            "AC-047: {crate_name} must document every feature"
        );
    }
}

/// AC-048: `--no-default-features` compiles without a database, HTTP
/// framework, or native dependency.
#[test]
fn ac_048_no_crate_forces_a_database_http_framework_or_native_library() {
    for crate_name in CRATES {
        let manifest = manifest(crate_name);
        for forbidden in [
            "axum",
            "reqwest",
            "hyper",
            "openssl",
            "native-tls",
            "tokio-postgres",
            "libsqlite3-sys",
        ] {
            assert!(
                !manifest.contains(forbidden),
                "AC-048: {crate_name} must not depend on {forbidden}"
            );
        }

        if let Some(line) = manifest
            .lines()
            .find(|line| line.trim_start().starts_with("sqlx ="))
        {
            assert!(
                line.contains("optional = true"),
                "AC-048: {crate_name} must keep sqlx optional so --no-default-features builds, got {line:?}"
            );
            assert!(
                manifest.contains("postgres = [\"dep:sqlx\"]"),
                "AC-048: {crate_name} must gate sqlx behind the postgres feature"
            );
        }

        let features = features(&manifest);
        assert!(
            features.iter().any(|(name, _)| name == "default"),
            "AC-048: {crate_name} must declare an explicit default feature set"
        );
    }
}

/// AC-050: features are additive and never disable one another.
#[test]
fn ac_050_features_are_additive_across_the_feature_set() {
    let platform = manifest("platform-core");
    assert!(
        platform.contains("default = [\"registry\", \"memory\"]"),
        "AC-050: platform-core must default to registry and memory together"
    );
    assert!(platform.contains("registry = []"), "AC-050: registry must be additive");
    assert!(platform.contains("memory = []"), "AC-050: memory must be additive");

    let quota = manifest("quota-ledger");
    assert!(quota.contains("default = []"), "AC-050: quota-ledger must default to no features");
    assert!(
        quota.contains("postgres = [\"dep:sqlx\"]"),
        "AC-050: the postgres feature must only add its dependency"
    );

    let hook = manifest("hookgate");
    assert!(
        hook.contains("default = [\"presets\"]"),
        "AC-050: hookgate must default to its presets"
    );
    assert!(hook.contains("presets = []"), "AC-050: presets must be additive");

    let sync = manifest("syncforge");
    assert!(sync.contains("default = []"), "AC-050: syncforge must have no default features");

    for crate_name in CRATES {
        let manifest = manifest(crate_name);
        let features = features(&manifest);
        let declared: Vec<&str> = features.iter().map(|(name, _)| name.as_str()).collect();
        let defaults = features
            .iter()
            .find(|(name, _)| name == "default")
            .map(|(_, values)| values.clone())
            .unwrap_or_default();

        for default in defaults {
            assert!(
                declared.contains(&default.as_str()),
                "AC-050: {crate_name}'s default feature {default} must be declared"
            );
        }
        for (name, values) in &features {
            assert!(
                !values.iter().any(|value| value.starts_with('!') || value.starts_with('-')),
                "AC-050: {crate_name}'s feature {name} must not negate another feature"
            );
        }
    }
}

/// AC-053: `cargo package --list` includes READMEs, licenses, and 0.1.0
/// metadata for every crate.
#[test]
fn ac_053_every_crate_packages_readme_licenses_and_metadata() {
    for crate_name in CRATES {
        let manifest = manifest(crate_name);
        assert!(
            manifest.contains("version = \"0.1.0\""),
            "AC-053: {crate_name} must package version 0.1.0"
        );
        assert!(
            manifest.contains("license.workspace = true"),
            "AC-053: {crate_name} must inherit the workspace license"
        );
        assert!(
            manifest.contains("readme = \"README.md\""),
            "AC-053: {crate_name} must declare its README"
        );
        assert!(
            manifest.contains("repository.workspace = true"),
            "AC-053: {crate_name} must declare its repository"
        );
        assert!(
            manifest.contains("include = ["),
            "AC-053: {crate_name} must declare an explicit package include list"
        );
        for included in [
            "\"Cargo.toml\"",
            "\"README.md\"",
            "\"LICENSE-MIT\"",
            "\"LICENSE-APACHE\"",
            "\"src/**\"",
        ] {
            assert!(
                manifest.contains(included),
                "AC-053: {crate_name}'s package must include {included}"
            );
        }

        for file in ["Cargo.toml", "README.md", "LICENSE-MIT", "LICENSE-APACHE"] {
            let path = repo_root().join("crates").join(crate_name).join(file);
            assert!(
                path.is_file(),
                "AC-053: {crate_name} must ship {file} for packaging"
            );
        }
    }

    assert!(
        manifest("hookgate").contains("\"examples/**\""),
        "AC-053: hookgate must package its integration example"
    );
}
