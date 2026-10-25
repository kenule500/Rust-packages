//! AC-001 through AC-006: tenant identity, the explicit provider registry, and
//! per-instance registration state.

use std::sync::Arc;

use platform_core::{
    Capability, IntegrationError, MemoryTokenStore, Provider, ProviderId, ProvisioningMode,
    Registry, Secret, TenantId, TokenStore,
};

/// A provider whose identifier and capabilities are fixed for the test.
struct TestProvider {
    id: ProviderId,
    capabilities: &'static [Capability],
}

impl Provider for TestProvider {
    fn id(&self) -> ProviderId {
        self.id
    }

    fn capabilities(&self) -> &'static [Capability] {
        self.capabilities
    }
}

static MESSAGING_CAPABILITIES: [Capability; 2] = [
    Capability::new("messages.send", ProvisioningMode::Api),
    Capability::new("messages.status", ProvisioningMode::Partner),
];

static VOICE_CAPABILITIES: [Capability; 1] =
    [Capability::new("calls.start", ProvisioningMode::Manual)];

fn provider(id: &'static str, capabilities: &'static [Capability]) -> Arc<dyn Provider> {
    Arc::new(TestProvider {
        id: ProviderId::new(id),
        capabilities,
    })
}

/// AC-001: tokens stored for two tenants never cross a tenant boundary.
#[tokio::test]
async fn ac_001_a_tenant_reads_only_its_own_provider_token() {
    let store = MemoryTokenStore::new();
    let provider_id = ProviderId::new("acme");
    let tenant_a = TenantId::generate();
    let tenant_b = TenantId::generate();

    store
        .put(tenant_a, provider_id, Secret::new("token-for-tenant-a"))
        .await
        .expect("AC-001: tenant A token must be stored");
    store
        .put(tenant_b, provider_id, Secret::new("token-for-tenant-b"))
        .await
        .expect("AC-001: tenant B token must be stored");

    let from_a = store
        .get(tenant_a, provider_id)
        .await
        .expect("AC-001: tenant A read must succeed")
        .expect("AC-001: tenant A must have a token");
    let from_b = store
        .get(tenant_b, provider_id)
        .await
        .expect("AC-001: tenant B read must succeed")
        .expect("AC-001: tenant B must have a token");

    assert_eq!(
        from_a.expose(),
        "token-for-tenant-a",
        "AC-001: tenant A must read only its own token"
    );
    assert_eq!(
        from_b.expose(),
        "token-for-tenant-b",
        "AC-001: tenant B must read only its own token"
    );
    assert_ne!(
        from_a.expose(),
        from_b.expose(),
        "AC-001: a provider token must never be shared between tenants"
    );

    let unknown = TenantId::generate();
    assert!(
        store
            .get(unknown, provider_id)
            .await
            .expect("AC-001: unknown tenant read must succeed")
            .is_none(),
        "AC-001: an unknown tenant must not receive another tenant's token"
    );
}

/// AC-002: an unregistered provider yields `None` and never panics.
#[test]
fn ac_002_unknown_provider_lookup_returns_none_without_panicking() {
    let registry = Registry::new();
    let missing = ProviderId::new("not-registered");

    assert!(
        registry.get(&missing).is_none(),
        "AC-002: an unregistered provider must return None"
    );
    assert!(
        registry.capabilities(&missing).is_none(),
        "AC-002: capabilities must not be inferred for an unregistered provider"
    );
    assert!(registry.is_empty(), "AC-002: a new registry must be empty");
    assert_eq!(
        registry.iter().count(),
        0,
        "AC-002: iterating an empty registry must yield nothing"
    );
}

/// AC-003: capability lookup returns exactly the declared keys and modes.
#[test]
fn ac_003_capabilities_match_the_declared_slice() {
    let mut registry = Registry::new();
    registry
        .register(provider("messaging", &MESSAGING_CAPABILITIES))
        .expect("AC-003: provider registration must succeed");

    let declared = registry
        .capabilities(&ProviderId::new("messaging"))
        .expect("AC-003: a registered provider must report capabilities");

    assert_eq!(
        declared,
        MESSAGING_CAPABILITIES.as_slice(),
        "AC-003: the registry must return exactly the declared capabilities"
    );
    assert_eq!(
        declared
            .iter()
            .map(|capability| capability.key)
            .collect::<Vec<_>>(),
        vec!["messages.send", "messages.status"],
        "AC-003: capability keys must match the declaration"
    );
    assert_eq!(
        declared
            .iter()
            .map(|capability| capability.mode)
            .collect::<Vec<_>>(),
        vec![ProvisioningMode::Api, ProvisioningMode::Partner],
        "AC-003: provisioning modes must match the declaration"
    );
}

/// AC-004: a duplicate registration is a typed error and preserves the entry.
#[test]
fn ac_004_duplicate_registration_fails_without_replacing_the_entry() {
    let mut registry = Registry::new();
    registry
        .register(provider("acme", &MESSAGING_CAPABILITIES))
        .expect("AC-004: first registration must succeed");

    let error = registry
        .register(provider("acme", &VOICE_CAPABILITIES))
        .expect_err("AC-004: a duplicate provider id must be rejected");

    assert!(
        matches!(error, IntegrationError::DuplicateProvider { .. }),
        "AC-004: a duplicate registration must return a typed duplicate error, got {error:?}"
    );
    assert_eq!(registry.len(), 1, "AC-004: the registry must keep one entry");
    assert_eq!(
        registry
            .capabilities(&ProviderId::new("acme"))
            .expect("AC-004: the original entry must still be present"),
        MESSAGING_CAPABILITIES.as_slice(),
        "AC-004: the original provider entry must be unchanged"
    );
}

/// AC-005: iteration follows registration order deterministically.
#[test]
fn ac_005_registry_iterates_in_registration_order() {
    let mut registry = Registry::new();
    for id in ["alpha", "beta", "gamma"] {
        registry
            .register(provider(id, &MESSAGING_CAPABILITIES))
            .expect("AC-005: registration must succeed");
    }

    let order: Vec<&str> = registry
        .iter()
        .map(|registered| registered.id().as_str())
        .collect();

    assert_eq!(
        order,
        vec!["alpha", "beta", "gamma"],
        "AC-005: providers must be yielded in registration order"
    );

    let repeated: Vec<&str> = registry
        .iter()
        .map(|registered| registered.id().as_str())
        .collect();
    assert_eq!(
        order, repeated,
        "AC-005: iteration order must be deterministic across calls"
    );
}

/// AC-006: two registries in one process share no mutable state.
#[test]
fn ac_006_registries_are_isolated_from_each_other() {
    let mut first = Registry::new();
    let mut second = Registry::new();

    first
        .register(provider("first-only", &MESSAGING_CAPABILITIES))
        .expect("AC-006: registration in the first registry must succeed");

    assert_eq!(first.len(), 1, "AC-006: the first registry must hold one provider");
    assert!(
        second.is_empty(),
        "AC-006: registering in one registry must not affect another"
    );
    assert!(
        second.get(&ProviderId::new("first-only")).is_none(),
        "AC-006: the second registry must not see the first registry's provider"
    );

    second
        .register(provider("second-only", &VOICE_CAPABILITIES))
        .expect("AC-006: registration in the second registry must succeed");

    assert_eq!(first.len(), 1, "AC-006: the first registry must be unchanged");
    assert_eq!(second.len(), 1, "AC-006: the second registry must hold one provider");
    assert!(
        first.get(&ProviderId::new("second-only")).is_none(),
        "AC-006: no global mutable registry may be shared between instances"
    );
}
