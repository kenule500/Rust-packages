//! Crate-level constraints: no unsafe, no domain models, no `.env` reads, and
//! complete AC coverage (AC-037 to AC-042).

use crate::common;

#[test]
fn ac_037_the_crate_root_forbids_unsafe_code_and_denies_missing_docs() {
    let lib = common::crate_source("lib");

    assert!(
        lib.contains("#![forbid(unsafe_code)]"),
        "the crate root must forbid unsafe code"
    );
    assert!(
        lib.contains("#![deny(missing_docs)]"),
        "the crate root must deny missing documentation, which is how every public item is documented"
    );
    assert!(
        lib.contains("pub mod noaa")
            && lib.contains("pub mod twilio")
            && lib.contains("pub mod sendgrid")
            && lib.contains("pub mod slack"),
        "every provider module is declared behind its feature"
    );

    for (name, source) in common::crate_sources() {
        assert!(
            !source.contains("unsafe {") && !source.contains("unsafe fn"),
            "{name} contains unsafe code"
        );
    }
}

#[test]
fn ac_038_features_are_additive_and_provider_modules_stay_compiled() {
    let manifest = common::repo_file("Cargo.toml");
    assert!(
        manifest.contains("default = []"),
        "the default feature set enables no provider"
    );

    let providers = ["noaa", "twilio", "sendgrid", "slack"];
    for provider in providers {
        let prefix = format!("{provider} = ");
        let line = manifest
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with(&prefix))
            .unwrap_or_else(|| panic!("the {provider} feature is declared"));
        for other in providers {
            if other != provider {
                assert!(
                    !line.contains(other),
                    "the {provider} feature must not enable {other}: {line}"
                );
            }
        }
        let compiled = match provider {
            "noaa" => wirekit::noaa::BASE_URL == "https://api.weather.gov",
            "twilio" => wirekit::twilio::BASE_URL == "https://api.twilio.com/2010-04-01",
            "sendgrid" => wirekit::sendgrid::BASE_URL == "https://api.sendgrid.com/v3",
            "slack" => wirekit::slack::BASE_URL == "https://slack.com/api",
            other => panic!("unexpected provider {other}"),
        };
        assert!(compiled, "the {provider} module is compiled and exported");
    }

    // A provider feature only pulls its own optional dependencies.
    let twilio = manifest
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("twilio = "))
        .expect("the twilio feature");
    assert!(twilio.contains("dep:sha1") && twilio.contains("dep:serde_urlencoded"));
    let sendgrid = manifest
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with("sendgrid = "))
        .expect("the sendgrid feature");
    assert!(sendgrid.contains("dep:hex") && sendgrid.contains("dep:p256"));
}

#[test]
fn ac_039_the_workspace_targets_rust_1_85_and_edition_2021() {
    let workspace = common::repo_file("../../Cargo.toml");
    assert!(
        workspace.contains("edition = \"2021\""),
        "the workspace edition is 2021"
    );
    assert!(
        workspace.contains("rust-version = \"1.85\""),
        "the workspace MSRV is 1.85"
    );

    let manifest = common::repo_file("Cargo.toml");
    assert!(manifest.contains("edition.workspace = true"));
    assert!(manifest.contains("rust-version.workspace = true"));
}

#[test]
fn ac_040_no_database_tenant_billing_or_application_domain_types() {
    let forbidden = [
        "struct Tenant",
        "enum Tenant",
        "tenant_id",
        "struct Billing",
        "struct Invoice",
        "struct Subscription",
        "struct Account",
        "struct Customer",
        "billing_account",
        "database_url",
        "diesel::",
        "sqlx::",
    ];

    for (name, source) in common::crate_sources() {
        for needle in forbidden {
            assert!(
                !source.contains(needle),
                "{name} introduces the application-domain concept `{needle}`"
            );
        }
    }

    // The public surface is HTTP, retry, rate-limit, and provider types only.
    let lib = common::crate_source("lib");
    for expected in [
        "pub mod auth",
        "pub mod client",
        "pub mod error",
        "pub mod pipeline",
        "pub mod rate_limit",
        "pub mod retry",
        "pub mod secret",
        "pub mod trace",
    ] {
        assert!(lib.contains(expected), "the crate exports {expected}");
    }
}

#[test]
fn ac_041_the_crate_never_reads_dotenv_files_for_credentials() {
    // Documentation lines are excluded: the crate docs state the guarantee
    // ("never reads a `.env` file"), they do not perform a read.
    fn code_only(source: &str) -> String {
        source
            .lines()
            .filter(|line| !line.trim_start().starts_with("//"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    for (name, source) in common::crate_sources() {
        let code = code_only(&source);
        for needle in [
            "std::env::var",
            "std::env::vars",
            "dotenv",
            "dotenvy",
            ".env",
            "env::args",
        ] {
            assert!(
                !code.contains(needle),
                "{name} reads credentials from the environment ({needle})"
            );
        }
    }

    // Credentials arrive only through the explicit constructors.
    let auth = code_only(&common::crate_source("auth"));
    assert!(auth.contains("pub fn new("));
    assert!(
        !auth.contains("env!"),
        "auth must not read a compile-time environment variable either"
    );
}

#[test]
fn ac_042_every_acceptance_criterion_has_exactly_one_named_test() {
    let mut declared: Vec<u32> = Vec::new();
    for (name, source) in common::test_sources() {
        for line in source.lines() {
            // Only definitions count, so the scanner's own source (which
            // mentions `fn ac_` inside a string) is not a false positive.
            let trimmed = line.trim_start();
            let definition = trimmed
                .strip_prefix("fn ac_")
                .or_else(|| trimmed.strip_prefix("async fn ac_"));
            let Some(rest) = definition else {
                continue;
            };
            let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
            let id = digits
                .parse::<u32>()
                .unwrap_or_else(|_| panic!("{name} has an unnumbered ac_ test: {line}"));
            declared.push(id);
        }
    }
    declared.sort_unstable();

    let duplicates: Vec<u32> = declared
        .windows(2)
        .filter(|pair| pair[0] == pair[1])
        .map(|pair| pair[0])
        .collect();
    assert!(
        duplicates.is_empty(),
        "more than one test carries these ids: {duplicates:?}"
    );

    let expected = common::spec_ac_ids();
    assert_eq!(
        expected.len(),
        82,
        "the specification declares 82 acceptance criteria"
    );
    assert_eq!(
        declared, expected,
        "every acceptance criterion must have a test, and no test may invent one"
    );
}
