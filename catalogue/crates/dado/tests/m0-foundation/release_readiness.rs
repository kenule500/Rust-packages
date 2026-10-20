//! Acceptance tests for AC-028 through AC-034: release readiness, package
//! metadata, and the published-crate smoke path.

use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::{
    assert_contains, assert_contains_all, assert_not_contains, crate_path, read_crate, read_repo,
    repo_root,
};

#[test]
fn ac_028_publish_dry_run_is_documented_and_recorded_as_passing() {
    let doc = read_repo("RELEASING.md");

    assert_contains_all(
        &doc,
        &[
            "### Dry-run result",
            "Command: cargo publish -p dado --dry-run",
            "Result: Passed",
            "Date: 2026-10-20",
        ],
        "RELEASING.md dry-run record",
    );
}

#[test]
#[ignore = "requires dado 0.1.0 to be published on crates.io"]
fn ac_029_published_crate_resolves_and_the_yaml_example_compiles() {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock is before the Unix epoch")
        .as_nanos();
    let workspace = std::env::temp_dir().join(format!("dado-ac029-{}-{stamp}", std::process::id()));
    std::fs::create_dir_all(&workspace).expect("create smoke-test workspace");

    let new_status = Command::new("cargo")
        .args(["new", "--bin", "dado-smoke"])
        .current_dir(&workspace)
        .status()
        .expect("run cargo new");
    assert!(new_status.success(), "cargo new failed");

    let project = workspace.join("dado-smoke");
    let add_status = Command::new("cargo")
        .args(["add", "dado@0.1.0", "--features", "yaml"])
        .current_dir(&project)
        .status()
        .expect("run cargo add");
    assert!(add_status.success(), "cargo add dado@0.1.0 failed");

    std::fs::write(
        project.join("src/main.rs"),
        r#"use std::collections::BTreeMap;

fn main() -> Result<(), dado::yaml::Error> {
    let input = "\
name: gateway
region: us-east-1
";
    let config: BTreeMap<String, String> = dado::yaml::from_str(input)?;
    assert_eq!(config["name"], "gateway");
    Ok(())
}
"#,
    )
    .expect("write smoke-test main.rs");

    let check = Command::new("cargo")
        .arg("check")
        .current_dir(&project)
        .output()
        .expect("run cargo check");
    assert!(
        check.status.success(),
        "published crate did not compile:\n{}",
        String::from_utf8_lossy(&check.stderr)
    );

    let _ = std::fs::remove_dir_all(&workspace);
}

#[test]
fn ac_030_changelog_contains_the_0_1_0_release_and_tag_name() {
    let changelog = read_crate("CHANGELOG.md");
    let releasing = read_repo("RELEASING.md");

    assert_contains(&changelog, "## [0.1.0] - 2026-10-20", "CHANGELOG.md");
    assert_contains(&changelog, "### Added", "CHANGELOG.md");
    assert_contains(
        &changelog,
        "releases/tag/dado-v0.1.0",
        "CHANGELOG.md release link",
    );
    assert_contains(&releasing, "git tag -a dado-v0.1.0", "RELEASING.md tag");
}

#[test]
#[ignore = "requires the dado-v0.1.0 release tag to exist"]
fn ac_030_release_tag_exists() {
    let output = Command::new("git")
        .args(["tag", "--list", "dado-v0.1.0"])
        .current_dir(repo_root())
        .output()
        .expect("run git tag --list");
    assert!(output.status.success(), "git tag lookup failed");
    assert_eq!(
        String::from_utf8_lossy(&output.stdout).trim(),
        "dado-v0.1.0",
        "release tag is missing"
    );
}

#[test]
fn ac_031_token_is_documented_as_a_secret_and_not_committed() {
    let releasing = read_repo("RELEASING.md");
    let ci = read_repo(".github/workflows/ci.yml");

    assert_contains_all(
        &releasing,
        &[
            "CARGO_REGISTRY_TOKEN",
            "environment or a CI secret",
            "<token supplied by your secret store>",
        ],
        "RELEASING.md credential guidance",
    );
    assert_not_contains(&ci, "CARGO_REGISTRY_TOKEN", "ci.yml");
    assert_not_contains(&ci, "cargo publish", "ci.yml");

    for (label, document) in [("RELEASING.md", releasing), ("ci.yml", ci)] {
        for line in document.lines() {
            let Some(rest) = line.split("CARGO_REGISTRY_TOKEN").nth(1) else {
                continue;
            };
            let Some(equals) = rest.find('=') else {
                continue;
            };
            let value = rest[equals + 1..].trim().trim_matches('"').trim();
            assert!(
                value.starts_with('<') || value.starts_with("$env"),
                "{label} appears to contain a token value: {line}"
            );
        }
    }
}

#[test]
fn ac_032_package_metadata_resolves_to_real_values() {
    let manifest = read_crate("Cargo.toml");
    let workspace = read_repo("catalogue/Cargo.toml");

    assert_contains_all(
        &manifest,
        &[
            "version = \"0.1.0\"",
            "repository.workspace = true",
            "readme = \"README.md\"",
            "keywords = [",
            "categories = [",
            "license.workspace = true",
            "rust-version.workspace = true",
        ],
        "dado Cargo.toml",
    );
    assert_contains_all(
        &workspace,
        &[
            "rust-version = \"1.85\"",
            "license = \"MIT OR Apache-2.0\"",
            "repository = \"https://github.com/kenule500/Rust-packages\"",
        ],
        "workspace Cargo.toml",
    );
    assert!(crate_path("README.md").is_file(), "README.md is missing");
}

#[test]
fn ac_033_package_contents_include_code_readme_and_licences() {
    let manifest = read_crate("Cargo.toml");

    for relative in [
        "Cargo.toml",
        "README.md",
        "CHANGELOG.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "src/lib.rs",
        "src/yaml.rs",
    ] {
        assert!(
            crate_path(relative).is_file(),
            "packaged file is missing: {relative}"
        );
    }

    assert_contains_all(
        &manifest,
        &[
            "include = [",
            "\"Cargo.toml\"",
            "\"README.md\"",
            "\"CHANGELOG.md\"",
            "\"LICENSE-MIT\"",
            "\"LICENSE-APACHE\"",
            "\"src/**\"",
        ],
        "dado package include list",
    );
}

#[test]
fn ac_034_docs_rs_builds_with_all_features() {
    let manifest = read_crate("Cargo.toml");
    let ci = read_repo(".github/workflows/ci.yml");

    assert_contains(&manifest, "[package.metadata.docs.rs]", "docs.rs metadata");
    assert_contains(&manifest, "all-features = true", "docs.rs metadata");
    assert_contains(
        &ci,
        "cargo doc --workspace --all-features --no-deps",
        "docs job",
    );
}
