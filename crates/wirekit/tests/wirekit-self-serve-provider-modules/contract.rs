//! Cross-module acceptance tests for the shared contract (AC-002, AC-005,
//! AC-006, AC-007).

use std::path::PathBuf;

use wirekit::accuweather::AccuWeatherConfig;
use wirekit::deepgram::DeepgramConfig;
use wirekit::klarna::{KlarnaConfig, KlarnaRegion};
use wirekit::lob::LobConfig;
use wirekit::pagerduty::PagerDutyConfig;
use wirekit::yelp::YelpConfig;
use wirekit::Secret;

use crate::common;

/// The six modules and the feature that gates each one.
const MODULES: [(&str, &str); 6] = [
    ("accuweather", "accuweather"),
    ("lob", "lob"),
    ("pagerduty", "pagerduty"),
    ("deepgram", "deepgram"),
    ("yelp", "yelp"),
    ("klarna", "klarna"),
];

fn crate_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

/// Normalise the line endings of a checked-out file.
///
/// The assertions in this module compare against multi-line literals joined
/// with `\n`. A checkout that materialises CRLF (a Windows runner, or
/// `core.autocrlf=true`) would otherwise fail every such comparison even
/// though the file itself is correct, so the readers normalise once here
/// instead of repeating the conversion at each assertion.
fn normalise_line_endings(source: String) -> String {
    source.replace("\r\n", "\n")
}

fn module_source(module: &str) -> String {
    let path = crate_dir().join("src").join(format!("{module}.rs"));
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("{} is readable: {error}", path.display()));
    normalise_line_endings(source)
}

fn manifest() -> String {
    let source = std::fs::read_to_string(crate_dir().join("Cargo.toml"))
        .expect("the crate manifest is readable");
    normalise_line_endings(source)
}

fn lib_source() -> String {
    let source = std::fs::read_to_string(crate_dir().join("src").join("lib.rs"))
        .expect("the crate root is readable");
    normalise_line_endings(source)
}

/// The `//!` module documentation, with the marker removed.
fn module_docs(source: &str) -> String {
    source
        .lines()
        .take_while(|line| line.starts_with("//!") || line.trim().is_empty())
        .filter(|line| line.starts_with("//!"))
        .map(|line| line.trim_start_matches("//!").trim_start())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The signature of every `pub async fn`, up to its opening brace.
fn async_signatures(source: &str) -> Vec<String> {
    source
        .split("pub async fn")
        .skip(1)
        .map(|chunk| chunk.split('{').next().unwrap_or_default().to_string())
        .collect()
}

#[test]
fn ac_002_every_module_maps_failures_into_wirekit_error() {
    for (module, _) in MODULES {
        let source = module_source(module);
        assert!(
            source.contains("WirekitError"),
            "{module} must reference the shared error type"
        );
        assert!(
            !source.contains("impl std::error::Error")
                && !source.contains("impl Error for")
                && !source.contains("thiserror::Error"),
            "{module} must not introduce a second error type (CON-207)"
        );
        assert!(
            !source.contains("enum Wirekit"),
            "{module} must not define a competing error enum"
        );

        let signatures = async_signatures(&source);
        assert!(
            !signatures.is_empty(),
            "{module} exposes asynchronous calls"
        );
        for signature in signatures {
            assert!(
                signature.contains("WirekitError"),
                "{module} maps a call into WirekitError: {signature}"
            );
        }
    }
}

#[test]
fn ac_004_every_module_uses_the_shared_retry_policy_and_its_own_rate_limit_key() {
    for (module, _) in MODULES {
        let source = module_source(module);
        assert!(
            !source.contains("RetryPolicy"),
            "{module} must not build a retry policy of its own (CON-208)"
        );
        assert!(
            !source.contains("fn should_retry") && !source.contains("backoff"),
            "{module} must not re-implement the shared retry loop"
        );
        let expected = format!(".rate_limit_key(\"{module}\")");
        assert!(
            source.contains(&expected),
            "{module} must apply its own token bucket key: {expected}"
        );
    }
}

#[test]
fn ac_005_every_module_holds_its_credential_in_a_redacting_secret() {
    let configs: Vec<(&str, String)> = vec![
        (
            "accuweather",
            format!(
                "{:?}",
                AccuWeatherConfig::new(Secret::new(String::from(common::API_KEY)))
            ),
        ),
        (
            "lob",
            format!(
                "{:?}",
                LobConfig::new(Secret::new(String::from(common::API_KEY))).with_test_mode()
            ),
        ),
        (
            "pagerduty",
            format!(
                "{:?}",
                PagerDutyConfig::new(
                    Secret::new(String::from(common::API_KEY)),
                    Secret::new(String::from("routing-key"))
                )
            ),
        ),
        (
            "deepgram",
            format!(
                "{:?}",
                DeepgramConfig::new(Secret::new(String::from(common::API_KEY)))
            ),
        ),
        (
            "yelp",
            format!(
                "{:?}",
                YelpConfig::new(Secret::new(String::from(common::API_KEY)))
            ),
        ),
        (
            "klarna",
            format!(
                "{:?}",
                KlarnaConfig::new(
                    Secret::new(String::from("merchant")),
                    Secret::new(String::from(common::API_KEY)),
                    KlarnaRegion::Europe
                )
            ),
        ),
    ];

    for (module, rendered) in configs {
        assert!(
            rendered.contains("***"),
            "{module} renders a redacted secret"
        );
        assert!(
            !rendered.contains(common::API_KEY),
            "{module} must never render its secret: {rendered}"
        );
    }

    for (module, _) in MODULES {
        let source = module_source(module);
        assert!(
            source.contains("Secret<"),
            "{module} holds its credential in a Secret"
        );
        assert!(
            !source.contains("impl Serialize for"),
            "{module} must not serialize a secret field"
        );
    }
}

#[test]
fn ac_006_features_are_independent_and_default_is_empty() {
    let manifest = manifest();
    assert!(
        manifest.contains("default = []"),
        "the default feature set enables no provider"
    );

    for (module, feature) in MODULES {
        let expected = format!("{feature} = []");
        let line = manifest
            .lines()
            .map(str::trim)
            .find(|line| line.starts_with(&format!("{feature} = ")))
            .unwrap_or_else(|| panic!("the {feature} feature is declared"));
        assert_eq!(
            line, expected,
            "the {feature} feature enables only its own module"
        );
        for (other, other_feature) in MODULES {
            if other != module {
                assert!(
                    !line.contains(other_feature),
                    "{feature} must not enable {other_feature}: {line}"
                );
            }
        }
    }

    let lib = lib_source();
    for (module, feature) in MODULES {
        let gate = format!("#[cfg(feature = \"{feature}\")]\npub mod {module};");
        assert!(
            lib.contains(&gate),
            "{module} is declared behind its feature: {gate}"
        );
    }

    let required = manifest
        .split("name = \"wirekit-self-serve-provider-modules\"")
        .nth(1)
        .and_then(|rest| rest.split("required-features = [").nth(1))
        .and_then(|rest| rest.split(']').next())
        .expect("the acceptance-test target requires the six features");
    for (_, feature) in MODULES {
        assert!(
            required.contains(&format!("\"{feature}\"")),
            "the test target requires {feature}: {required}"
        );
    }
}

#[test]
fn ac_007_the_crate_lints_hold_and_each_module_states_scope_and_non_goals() {
    let lib = lib_source();
    assert!(lib.contains("#![forbid(unsafe_code)]"));
    assert!(lib.contains("#![deny(missing_docs)]"));

    for (module, _) in MODULES {
        let source = module_source(module);
        assert!(
            !source.contains("unsafe {") && !source.contains("unsafe fn"),
            "{module} contains unsafe code"
        );
        let docs = module_docs(&source);
        assert!(
            docs.contains("Scope") && docs.contains("Non-goals"),
            "{module} must state its scope and non-goals"
        );
        assert!(
            docs.contains("Built from `catalogue/"),
            "{module} must cite the catalogue design file it implements"
        );
    }
}

/// Keep the helper honest: the module directory really contains exactly the
/// six files this suite names.
#[test]
fn ac_001_the_six_modules_exist_where_the_manifest_declares_them() {
    let src = crate_dir().join("src");
    for (module, _) in MODULES {
        assert!(
            src.join(format!("{module}.rs")).is_file(),
            "{module}.rs exists"
        );
    }
}
