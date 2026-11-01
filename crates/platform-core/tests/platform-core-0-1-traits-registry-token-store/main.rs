//! Acceptance tests for `platform-core-0-1-traits-registry-token-store`
//! (AC-001 to AC-013). Every test name carries its AC id (CON-405).

use std::sync::Arc;

use platform_core::{
    Capability, MemoryTokenStore, Provider, ProviderId, ProviderRef, ProvisioningMode, Registry,
    Secret, TenantId, TokenStore,
};

struct TestProvider {
    name: &'static str,
    caps: &'static [Capability],
}

impl Provider for TestProvider {
    fn id(&self) -> ProviderId {
        ProviderId::new(self.name)
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.caps
    }
}

static MESSAGES: &[Capability] = &[Capability::new("messages.send", ProvisioningMode::Api)];
static NOTHING: &[Capability] = &[];

fn provider(name: &'static str, caps: &'static [Capability]) -> ProviderRef {
    Arc::new(TestProvider { name, caps })
}

#[test]
fn ac_001_the_identity_and_token_surface_exists_without_the_registry() {
    // The registry is feature-gated; identity, error, and token types are not.
    let tenant = TenantId::generate();
    assert_eq!(tenant.as_uuid(), tenant.as_uuid());

    let provider = ProviderId::new("twilio");
    assert_eq!(provider.as_str(), "twilio");

    let secret = Secret::new(String::from("token"));
    assert_eq!(secret.expose(), "token");
}

#[test]
fn ac_002_tenant_and_provider_ids_are_cheap_value_types() {
    let first = TenantId::generate();
    let second = TenantId::generate();
    assert_ne!(first, second, "fresh tenant ids differ");
    assert_eq!(first, first.clone(), "tenant ids compare by value");

    let provider = ProviderId::new("slack");
    assert_eq!(provider, ProviderId::new("slack"));
    assert_ne!(provider, ProviderId::new("twilio"));
    let copied = provider;
    assert_eq!(copied.as_str(), provider.as_str());
}

#[test]
fn ac_003_provisioning_modes_are_distinct() {
    let modes = [
        ProvisioningMode::Api,
        ProvisioningMode::Partner,
        ProvisioningMode::Manual,
        ProvisioningMode::Unsupported,
    ];

    for (index, mode) in modes.iter().enumerate() {
        for other in &modes[index + 1..] {
            assert_ne!(mode, other, "each provisioning mode is distinct");
        }
    }
}

#[test]
fn ac_004_a_capability_pairs_an_opaque_key_with_its_mode() {
    let capability = Capability::new("messages.send", ProvisioningMode::Partner);

    assert_eq!(capability.key, "messages.send");
    assert_eq!(capability.mode, ProvisioningMode::Partner);
}

#[test]
fn ac_005_capability_keys_are_opaque() {
    // The crate must not normalise, lowercase, or reinterpret the key.
    let odd = Capability::new("  Messages.Send  ", ProvisioningMode::Manual);
    assert_eq!(odd.key, "  Messages.Send  ");
}

#[test]
fn ac_006_a_registered_provider_is_visible_through_the_registry() {
    let mut registry = Registry::new();
    registry
        .register(provider("twilio", MESSAGES))
        .expect("the first registration succeeds");

    assert_eq!(registry.len(), 1);
    assert!(!registry.is_empty());

    let id = ProviderId::new("twilio");
    assert!(
        registry.get(&id).is_some(),
        "a registered provider is found"
    );
    assert_eq!(registry.iter().count(), 1);
    assert_eq!(registry.capabilities(&id).map(<[Capability]>::len), Some(1));
}

#[test]
fn ac_007_a_duplicate_provider_id_is_rejected() {
    let mut registry = Registry::new();
    registry
        .register(provider("slack", MESSAGES))
        .expect("the first registration succeeds");

    let error = registry
        .register(provider("slack", NOTHING))
        .expect_err("the second registration is rejected");

    assert_eq!(registry.len(), 1, "the registry is unchanged");
    let rendered = format!("{error}");
    assert!(
        rendered.contains("slack"),
        "the error names the id: {rendered}"
    );
}

#[test]
fn ac_008_registration_is_explicit_and_there_is_no_global_registry() {
    let mut first = Registry::new();
    first
        .register(provider("twilio", MESSAGES))
        .expect("the registration succeeds");

    let second = Registry::new();
    assert!(second.is_empty(), "a fresh registry holds nothing");
    assert_eq!(first.len(), 1, "registries are independent values");
}

#[tokio::test]
async fn ac_009_the_token_store_is_keyed_by_tenant_and_provider() {
    let store = MemoryTokenStore::new();
    let tenant = TenantId::generate();
    let other = TenantId::generate();
    let provider_id = ProviderId::new("twilio");

    assert!(store
        .get(tenant.clone(), provider_id)
        .await
        .expect("an absent token is not an error")
        .is_none());

    store
        .put(tenant.clone(), provider_id, Secret::new(String::from("t1")))
        .await
        .expect("the write succeeds");

    let stored = store
        .get(tenant.clone(), provider_id)
        .await
        .expect("the read succeeds")
        .expect("the token is present");
    assert_eq!(stored.expose(), "t1");

    assert!(
        store.get(other, provider_id).await.expect("read").is_none(),
        "another tenant never sees this token"
    );

    store
        .revoke(tenant.clone(), provider_id)
        .await
        .expect("the revoke succeeds");
    assert!(store
        .get(tenant, provider_id)
        .await
        .expect("read")
        .is_none());
}

#[tokio::test]
async fn ac_010_the_memory_store_is_gated_and_tracks_its_size() {
    let store = MemoryTokenStore::new();
    assert!(store.is_empty());

    store
        .put(
            TenantId::generate(),
            ProviderId::new("slack"),
            Secret::new(String::from("s")),
        )
        .await
        .expect("the write succeeds");

    assert_eq!(store.len(), 1);
}

#[test]
fn ac_011_a_secret_never_renders_its_value() {
    let secret = Secret::new(String::from("super-secret"));
    let rendered = format!("{secret:?}");

    assert!(rendered.contains("[REDACTED]"), "redacted: {rendered}");
    assert!(!rendered.contains("super-secret"));
    let displayed = format!("{secret}");
    assert!(
        displayed.contains("[REDACTED]"),
        "Display redacts: {displayed}"
    );
    assert_eq!(secret.expose(), "super-secret", "callers can still read it");
}

#[tokio::test]
async fn ac_012_an_absent_token_is_none_rather_than_an_error() {
    let store = MemoryTokenStore::new();
    let missing = store
        .get(TenantId::generate(), ProviderId::new("adp"))
        .await
        .expect("absence is not an error");

    assert!(missing.is_none(), "no token is fabricated");
}

#[test]
fn ac_013_the_crate_declares_its_lints_and_ships_its_licences() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    let lib = std::fs::read_to_string(root.join("src").join("lib.rs")).expect("lib.rs is readable");

    assert!(lib.contains("#![forbid(unsafe_code)]"));
    assert!(lib.contains("#![deny(missing_docs)]"));
    assert!(root.join("LICENSE-MIT").is_file());
    assert!(root.join("LICENSE-APACHE").is_file());
}
