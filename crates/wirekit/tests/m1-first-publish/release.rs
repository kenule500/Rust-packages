//! Acceptance tests for the release-preparation criteria (AC-075 to AC-082).
//!
//! These criteria are cargo- and file-level: `cargo package --list`,
//! `cargo publish --dry-run`, `cargo build --examples --all-features`, and
//! documentation content. Running cargo from inside a test would contend with
//! the test harness for the target-directory lock, so the assertions here
//! inspect the manifest, the files, and the documentation the commands act on.
//! The commands themselves are release-runbook and CI steps, which the test
//! report records as a coverage note.

use crate::common;

/// The trimmed text of one documentation table row.
fn row<'a>(text: &'a str, prefix: &str) -> &'a str {
    text.lines()
        .find(|line| line.trim_start().starts_with(prefix))
        .unwrap_or_else(|| panic!("no documentation row starts with {prefix:?}"))
}

/// The `//!` module documentation of a source file, with the markers removed.
fn module_docs(source: &str) -> String {
    source
        .lines()
        .take_while(|line| line.starts_with("//!") || line.trim().is_empty())
        .filter(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The string entries of a `keywords = [...]` or `categories = [...]` line.
fn list_entries<'a>(line: &'a str, key: &str) -> Vec<&'a str> {
    let prefix = format!("{key} = [");
    let line = line
        .trim_start()
        .strip_prefix(&prefix)
        .unwrap_or_else(|| panic!("expected {prefix:?} in {line:?}"));
    line.trim_end_matches(']')
        .split(',')
        .map(|entry| entry.trim().trim_matches('"'))
        .filter(|entry| !entry.is_empty())
        .collect()
}

#[test]
fn ac_075_the_package_includes_the_documented_file_set() {
    let manifest = common::repo_file("Cargo.toml");

    let include = manifest
        .split_once("include = [")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once(']'))
        .map(|(entries, _)| entries)
        .expect("the package declares an include list");

    for entry in [
        "Cargo.toml",
        "README.md",
        "CHANGELOG.md",
        "LICENSE-MIT",
        "LICENSE-APACHE",
        "src/**",
        "examples/**",
    ] {
        assert!(
            include.contains(&format!("\"{entry}\"")),
            "the include list must carry {entry}: {include}"
        );
    }

    // Every file the include list names actually exists and is not empty.
    for file in ["Cargo.toml", "README.md", "CHANGELOG.md"] {
        assert!(
            common::repo_file(file).len() > 200,
            "{file} is present and substantive"
        );
    }
    assert!(common::repo_file("LICENSE-MIT").contains("MIT License"));
    assert!(common::repo_file("LICENSE-APACHE").contains("Apache License"));

    let sources = common::crate_sources();
    assert!(
        sources.iter().any(|(name, _)| name == "lib.rs"),
        "src/** covers the library root"
    );
    assert!(
        sources.iter().any(|(name, _)| name == "noaa.rs"),
        "src/** covers the provider modules"
    );

    for example in ["noaa.rs", "twilio.rs", "sendgrid.rs", "slack.rs"] {
        assert!(
            common::repo_file(&format!("examples/{example}")).len() > 200,
            "examples/{example} is packaged"
        );
    }

    // Nothing outside the include list needs to ship, and the test harness
    // itself is deliberately not part of the package.
    assert!(
        !include.contains("tests/**"),
        "the acceptance tests are not published"
    );
}

#[test]
fn ac_076_the_crate_is_publishable_without_a_real_publish() {
    let manifest = common::repo_file("Cargo.toml");

    assert!(
        !manifest.contains("publish = false"),
        "the crate must be publishable"
    );
    assert!(
        !manifest.contains("path ="),
        "published dependencies cannot use paths"
    );
    assert!(
        !manifest.contains("[patch."),
        "a patch table would not be part of the published crate"
    );
    assert!(manifest.contains("version = \"0.1.0\""));
    assert!(manifest.contains("name = \"wirekit\""));

    // Every dependency entry carries a version requirement, so the packaged
    // manifest resolves from the registry.
    let dependencies = manifest
        .split_once("[dependencies]")
        .map(|(_, rest)| rest)
        .and_then(|rest| rest.split_once("\n[").map(|(block, _)| block))
        .expect("the manifest declares dependencies");
    let mut declared = 0;
    for line in dependencies.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('"')
            || line.starts_with(']')
            || line.starts_with('}')
            || line.ends_with("] }")
        {
            continue;
        }
        assert!(
            line.contains("version =") || (line.contains(" = \"") && !line.contains('{')),
            "the dependency {line:?} must pin a registry version"
        );
        assert!(
            !line.contains("path ="),
            "the dependency {line:?} must not use a path"
        );
        declared += 1;
    }
    assert!(declared >= 10, "the crate declares its dependencies");

    // The dry-run itself is a release step: it needs a clean tree and the
    // crates.io index, so it runs from the runbook, not from this test.
    let changelog = common::repo_file("CHANGELOG.md");
    assert!(changelog.contains("## [0.1.0]"));
}

#[test]
fn ac_077_published_metadata_resolves() {
    let workspace = common::repo_file("../../Cargo.toml");
    let manifest = common::repo_file("Cargo.toml");

    assert!(
        workspace.contains("repository = \"https://github.com/kenule500/Rust-packages\""),
        "the workspace declares the repository"
    );
    assert!(workspace.contains("license = \"MIT OR Apache-2.0\""));
    assert!(workspace.contains("rust-version = \"1.85\""));

    assert!(manifest.contains("repository.workspace = true"));
    assert!(manifest.contains("license.workspace = true"));
    assert!(manifest.contains("rust-version.workspace = true"));
    assert!(manifest.contains("readme = \"README.md\""));

    let description = row(&manifest, "description =");
    assert!(
        description.len() > 60,
        "the description is specific: {description}"
    );

    let keywords = list_entries(row(&manifest, "keywords ="), "keywords");
    assert!(
        !keywords.is_empty() && keywords.len() <= 5,
        "crates.io allows at most five keywords"
    );
    for keyword in &keywords {
        assert!(
            keyword.len() <= 20,
            "the keyword {keyword:?} must fit crates.io's 20-character limit"
        );
        assert!(
            keyword
                .chars()
                .all(|character| character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '-'
                    || character == '_'),
            "the keyword {keyword:?} is crates.io-shaped"
        );
    }

    let categories = list_entries(row(&manifest, "categories ="), "categories");
    assert!(!categories.is_empty());
    assert!(categories.len() <= 5, "crates.io allows at most five categories");
    for category in &categories {
        assert!(
            category
                .chars()
                .next()
                .is_some_and(|character| character.is_ascii_lowercase()),
            "the category {category:?} is a lowercase crates.io slug"
        );
        assert!(
            category
                .chars()
                .all(|character| character.is_ascii_lowercase()
                    || character.is_ascii_digit()
                    || character == '-'
                    || character == ':'),
            "the category {category:?} is crates.io-shaped"
        );
    }

    assert!(manifest.contains("[package.metadata.docs.rs]"));
    let docs_rs = manifest
        .split_once("[package.metadata.docs.rs]")
        .map(|(_, rest)| rest)
        .expect("the docs.rs metadata exists");
    assert!(
        docs_rs.contains("all-features = true"),
        "docs.rs builds every feature"
    );
}

#[test]
fn ac_078_the_changelog_describes_the_first_release() {
    let changelog = common::repo_file("CHANGELOG.md");
    let manifest = common::repo_file("Cargo.toml");

    assert!(changelog.contains("## [0.1.0]"));
    assert!(
        manifest.contains("version = \"0.1.0\""),
        "the changelog and the manifest agree on the version"
    );

    let entry = changelog
        .split_once("## [0.1.0]")
        .map(|(_, rest)| rest)
        .expect("the 0.1.0 entry exists");
    let entry = entry
        .split_once("\n## [")
        .map_or(entry, |(section, _)| section);

    for required in ["WirekitClient", "WirekitError", "RetryPolicy", "RateLimitConfig"] {
        assert!(
            entry.contains(required),
            "the 0.1.0 entry describes the core type {required}"
        );
    }
    for provider in ["noaa", "twilio", "sendgrid", "slack"] {
        assert!(
            entry.contains(&format!("`{provider}` feature")),
            "the 0.1.0 entry describes the {provider} provider"
        );
    }
    assert!(
        entry.contains("MSRV is Rust 1.85"),
        "the entry states the minimum supported Rust version"
    );
    assert!(entry.contains("### Added"));
    assert!(entry.contains("### Notes"));
}

#[test]
fn ac_079_the_provider_matrix_lists_auth_base_url_and_feature() {
    let readme = common::repo_file("README.md");
    let lib = module_docs(&common::crate_source("lib"));

    let providers = [
        ("| `noaa` |", "None", "https://api.weather.gov"),
        ("| `twilio` |", "Basic", "https://api.twilio.com/2010-04-01"),
        ("| `sendgrid` |", "Bearer", "https://api.sendgrid.com/v3"),
        ("| `slack` |", "Bot token", "https://slack.com/api"),
    ];

    for (prefix, auth, base_url) in providers {
        let readme_row = row(&readme, prefix);
        assert!(
            readme_row.contains(auth),
            "the README row carries the auth scheme: {readme_row}"
        );
        assert!(
            readme_row.contains(base_url),
            "the README row carries the base URL: {readme_row}"
        );

        let doc_row = row(&lib, prefix);
        assert!(
            doc_row.to_lowercase().contains(&auth.to_lowercase())
                && doc_row.contains(base_url),
            "the crate docs carry the same matrix entry: {doc_row}"
        );
    }

    assert!(readme.contains("| Feature | Module | Auth | Base URL |"));
    assert!(lib.contains("| Feature | Module | Auth | Base URL |"));

    // The native-tls alternative is documented too, so a reader is not
    // surprised by a second TLS backend.
    assert!(readme.contains("| `native-tls` |"));
    assert!(readme.contains("rustls"));
}

#[test]
fn ac_080_the_documentation_carries_the_feature_and_error_tables() {
    let lib = module_docs(&common::crate_source("lib"));
    let readme = common::repo_file("README.md");

    // Feature table: the crate docs list every feature-gated module, and the
    // README states which features exist and that the core is always present.
    for feature in ["noaa", "twilio", "sendgrid", "slack"] {
        assert!(
            lib.contains(&format!("| `{feature}` |")),
            "the crate docs list the {feature} feature"
        );
        assert!(
            readme.contains(&format!("\"{feature}\"")),
            "the README shows how to enable {feature}"
        );
    }
    assert!(lib.contains("`default = []`"));
    assert!(readme.contains("| Feature | Module | Auth | Base URL |"));

    // Error reference: one row per stable code, with the variant it comes from.
    let codes = [
        ("W2001", "Api"),
        ("W2002", "Auth"),
        ("W2003", "RateLimit"),
        ("W2004", "Timeout"),
        ("W2005", "Network"),
        ("W2006", "InvalidConfig"),
        ("W2007", "Decode"),
    ];
    assert!(readme.contains("| Code | Variant | Meaning |"));
    for (code, variant) in codes {
        let row = row(&readme, &format!("| `{code}` |"));
        assert!(
            row.contains(&format!("`{variant}`")),
            "the {code} row names the {variant} variant: {row}"
        );
        assert!(
            row.len() > format!("| `{code}` | `{variant}` | ").len() + 10,
            "the {code} row explains the error: {row}"
        );
    }

    // The constants behind the codes are part of the public API and are
    // individually documented.
    let source = common::crate_source("error");
    for (code, _) in codes {
        assert!(
            source.contains(&format!("pub const CODE_")),
            "the code constants are public"
        );
        assert!(
            source.contains(&format!("\"{code}\"")),
            "{code} is defined where the docs can link it"
        );
    }

    // A code documented in the README must match the code the error renders.
    let error = wirekit::WirekitError::invalid_config("the documentation check");
    assert_eq!(error.code(), "W2006");
    assert!(error.to_string().starts_with("W2006"));
}

#[test]
fn ac_081_each_provider_ships_a_runnable_example() {
    let manifest = common::repo_file("Cargo.toml");

    let providers = ["noaa", "twilio", "sendgrid", "slack"];
    for provider in providers {
        assert!(
            manifest.contains(&format!("name = \"{provider}\"")),
            "the {provider} example is declared"
        );
        assert!(
            manifest.contains(&format!("required-features = [\"{provider}\"]")),
            "the {provider} example needs its own feature"
        );

        let source = common::repo_file(&format!("examples/{provider}.rs"));
        assert!(
            source.contains("#[tokio::main"),
            "the {provider} example has an async entry point"
        );
        assert!(
            source.contains(&format!("wirekit::{provider}::")),
            "the {provider} example uses its provider module"
        );
        assert!(
            source.contains("BASE_URL"),
            "the {provider} example targets the provider's base URL"
        );
    }

    // The three credential-bearing examples exit cleanly and explain what to
    // set when the environment is empty.
    for (file, variable) in [
        ("twilio.rs", "TWILIO_ACCOUNT_SID"),
        ("sendgrid.rs", "SENDGRID_API_KEY"),
        ("slack.rs", "SLACK_BOT_TOKEN"),
    ] {
        let source = common::repo_file(&format!("examples/{file}"));
        assert!(
            source.contains(&format!("std::env::var(\"{variable}\")")),
            "the {file} example reads {variable}"
        );
        assert!(
            source.contains("eprintln!(") && source.contains("\"set "),
            "the {file} example says which variables to set"
        );
        assert!(
            source.contains("return Ok(());"),
            "the {file} example exits successfully when the variables are absent"
        );
    }

    // NOAA needs no key: it falls back to a descriptive agent and always runs.
    let noaa = common::repo_file("examples/noaa.rs");
    assert!(noaa.contains("std::env::var(\"NOAA_USER_AGENT\").unwrap_or_else"));
    assert!(
        !noaa.contains("return Ok(());"),
        "the NOAA example must run without any credential"
    );
    assert!(
        noaa.contains("wirekit-example/"),
        "the NOAA example sends an identifiable user agent by default"
    );

    // `cargo build --examples --all-features` checks the rest of the contract
    // (the examples compile); the manifest scopes each one to its feature.
    assert!(!manifest.contains("[[bin]]"));
}

#[test]
fn ac_082_the_release_runbook_covers_every_step() {
    let doc = common::repo_file("../../RELEASING.md");

    let (dado, wirekit) = doc
        .split_once("# Releasing wirekit")
        .expect("the runbook has a wirekit section");

    for heading in [
        "## 1. Pre-flight checks",
        "## 2. Version and changelog",
        "## 3. Package and dry-run",
        "## 4. Publish",
        "## 5. Tag the release",
        "## 6. Post-publish verification",
    ] {
        assert!(
            wirekit.contains(heading),
            "the wirekit runbook has the {heading:?} step"
        );
    }

    assert!(wirekit.contains("wirekit-v0.1.0"));
    assert!(wirekit.contains("git tag -a wirekit-v0.1.0"));
    assert!(wirekit.contains("git push origin wirekit-v0.1.0"));
    assert!(wirekit.contains("cargo publish -p wirekit --dry-run"));
    assert!(
        wirekit
            .lines()
            .any(|line| line.trim() == "cargo publish -p wirekit"),
        "the runbook publishes wirekit with an explicit command"
    );
    assert!(wirekit.contains("cargo search wirekit"));
    assert!(wirekit.contains("crates.io/api/v1/crates/wirekit"));
    assert!(wirekit.contains("docs.rs"));
    assert!(wirekit.contains("## wirekit release checklist"));
    assert!(
        !wirekit.contains("cargo publish -p dado"),
        "the wirekit section publishes only wirekit"
    );

    // Yank and rollback: the runbook states the policy and both commands. The
    // subsection is shared by both crates and its example names dado, so a
    // wirekit maintainer must substitute the crate name.
    assert!(doc.contains("Yank or rollback"));
    assert!(doc.contains("cargo yank --version 0.1.0"));
    assert!(doc.contains("--undo"));
    assert!(doc.contains("cargo yank --version 0.1.0 dado"));
    assert!(
        dado.contains("Yank or rollback"),
        "the shared yank guidance precedes the wirekit section"
    );
    assert!(
        doc.contains("attempt to overwrite a published version"),
        "the runbook forbids replacing a published version"
    );

    // The checklist turns the prose into a gate.
    for item in [
        "`cargo publish -p wirekit --dry-run` succeeds",
        "Tag `wirekit-v0.1.0` is pushed",
        "docs.rs builds with all features",
    ] {
        assert!(
            wirekit.contains(item),
            "the wirekit checklist tracks {item:?}"
        );
    }
}
