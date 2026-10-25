//! AC-007 through AC-009: the token-store contract, redaction, and typed
//! backend failures.

use async_trait::async_trait;
use platform_core::{MemoryTokenStore, ProviderId, Secret, TenantId, TokenError, TokenStore};

/// AC-007: put, read, revoke, then read again.
#[tokio::test]
async fn ac_007_put_read_revoke_round_trip() {
    let store = MemoryTokenStore::new();
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");

    store
        .put(tenant, provider, Secret::new("initial-token"))
        .await
        .expect("AC-007: put must succeed");
    assert_eq!(store.len(), 1, "AC-007: the store must hold one credential");

    let first = store
        .get(tenant, provider)
        .await
        .expect("AC-007: get must succeed")
        .expect("AC-007: the first read must return the token");
    assert_eq!(first.expose(), "initial-token", "AC-007: the token value must round-trip");

    store
        .revoke(tenant, provider)
        .await
        .expect("AC-007: revoke must succeed");
    assert!(
        store
            .get(tenant, provider)
            .await
            .expect("AC-007: get after revoke must succeed")
            .is_none(),
        "AC-007: the second read must return None after revocation"
    );
    assert!(store.is_empty(), "AC-007: the revoked credential must be removed");
}

/// AC-008: `Debug` and `Display` never render secret material.
#[tokio::test]
async fn ac_008_store_and_secret_are_redacted_in_debug_and_display() {
    let store = MemoryTokenStore::new();
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");
    let secret = Secret::new("super-secret-value");

    store
        .put(tenant, provider, Secret::new("super-secret-value"))
        .await
        .expect("AC-008: put must succeed");

    let secret_debug = format!("{secret:?}");
    let secret_display = format!("{secret}");
    let store_debug = format!("{store:?}");

    assert_eq!(
        secret_debug, "Secret([REDACTED])",
        "AC-008: Debug must redact the secret value"
    );
    assert_eq!(
        secret_display, "Secret([REDACTED])",
        "AC-008: Display must redact the secret value"
    );
    assert!(
        !secret_debug.contains("super-secret-value") && !secret_display.contains("super-secret-value"),
        "AC-008: neither Debug nor Display may render the token value"
    );
    assert!(
        !store_debug.contains("super-secret-value"),
        "AC-008: the store's Debug output must not render stored tokens"
    );
    assert!(
        store_debug.to_ascii_lowercase().contains("redacted"),
        "AC-008: the store's Debug output must mark the entries as redacted, got {store_debug}"
    );
}

/// AC-009: a failing backend surfaces a typed error and never panics.
#[tokio::test]
async fn ac_009_backend_failures_surface_typed_errors() {
    struct FailingStore;

    #[async_trait]
    impl TokenStore for FailingStore {
        async fn get(
            &self,
            _tenant: TenantId,
            _provider: ProviderId,
        ) -> Result<Option<Secret>, TokenError> {
            Err(TokenError::Backend("backend unavailable".to_owned()))
        }

        async fn put(
            &self,
            _tenant: TenantId,
            _provider: ProviderId,
            _token: Secret,
        ) -> Result<(), TokenError> {
            Err(TokenError::Backend("backend unavailable".to_owned()))
        }

        async fn revoke(
            &self,
            _tenant: TenantId,
            _provider: ProviderId,
        ) -> Result<(), TokenError> {
            Err(TokenError::Config("backend is misconfigured".to_owned()))
        }
    }

    let store = FailingStore;
    let tenant = TenantId::generate();
    let provider = ProviderId::new("acme");

    let get_error = store
        .get(tenant, provider)
        .await
        .expect_err("AC-009: get must surface the backend failure");
    assert!(
        matches!(get_error, TokenError::Backend(_)),
        "AC-009: get must return a typed TokenError, got {get_error:?}"
    );
    assert!(
        get_error.to_string().starts_with("PC-TOKEN-BACKEND"),
        "AC-009: the error must render its stable code, got {get_error}"
    );

    let put_error = store
        .put(tenant, provider, Secret::new("token"))
        .await
        .expect_err("AC-009: put must surface the backend failure");
    assert!(
        matches!(put_error, TokenError::Backend(_)),
        "AC-009: put must return a typed TokenError, got {put_error:?}"
    );

    let revoke_error = store
        .revoke(tenant, provider)
        .await
        .expect_err("AC-009: revoke must surface the backend failure");
    assert!(
        matches!(revoke_error, TokenError::Config(_)),
        "AC-009: revoke must return a typed TokenError, got {revoke_error:?}"
    );
}
