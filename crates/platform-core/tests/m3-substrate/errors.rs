//! AC-051: every `platform-core` error renders a stable code and implements
//! `std::error::Error`.

use platform_core::{ErrorCode, IntegrationError, ProviderId};

fn assert_stable_error(error: &IntegrationError, code: &str, label: &str) {
    assert!(
        error.to_string().starts_with(code),
        "AC-051: {label} must render the stable code {code}, got {error}"
    );
    let _: &dyn std::error::Error = error;
}

#[test]
fn ac_051_platform_core_errors_expose_stable_codes_and_implement_error() {
    assert_stable_error(&IntegrationError::auth("missing credentials"), "PC-AUTH", "auth");
    assert_stable_error(&IntegrationError::scope("missing scope"), "PC-SCOPE", "scope");
    assert_stable_error(
        &IntegrationError::rate_limit("slow down", Some(30)),
        "PC-RATE-LIMIT",
        "rate limit",
    );
    assert_stable_error(&IntegrationError::quota("quota exhausted"), "PC-QUOTA", "quota");
    assert_stable_error(
        &IntegrationError::provider("provider failed"),
        "PC-PROVIDER",
        "provider",
    );
    assert_stable_error(
        &IntegrationError::transport("connection reset"),
        "PC-TRANSPORT",
        "transport",
    );
    assert_stable_error(
        &IntegrationError::unsupported("not supported"),
        "PC-UNSUPPORTED",
        "unsupported",
    );
    assert_stable_error(
        &IntegrationError::duplicate_provider(ProviderId::new("acme")),
        "PC-DUPLICATE-PROVIDER",
        "duplicate provider",
    );
    assert_stable_error(
        &IntegrationError::config("invalid configuration"),
        "PC-CONFIG",
        "config",
    );

    assert_eq!(
        ErrorCode::RateLimit.as_str(),
        "PC-RATE-LIMIT",
        "AC-051: every error category must expose a stable code"
    );
}
