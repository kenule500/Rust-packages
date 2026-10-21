/// How a provider account is provisioned for a tenant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProvisioningMode {
    /// The tenant connects through the provider's public API.
    Api,
    /// The tenant is provisioned through a partner or reseller agreement.
    Partner,
    /// The account is configured manually by an operator.
    Manual,
    /// The provider is known but cannot serve this tenant.
    Unsupported,
}

/// One capability a provider declares for a tenant.
///
/// Capability keys are opaque strings owned by the adapter. The crate never
/// infers support from the presence of a dependency or a module.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capability {
    /// The adapter's stable capability key, for example `messages.send`.
    pub key: &'static str,
    /// How this capability is provisioned.
    pub mode: ProvisioningMode,
}

impl Capability {
    /// Creates a capability declaration.
    pub const fn new(key: &'static str, mode: ProvisioningMode) -> Self {
        Self { key, mode }
    }
}
