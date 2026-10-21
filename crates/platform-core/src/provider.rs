use std::sync::Arc;

use crate::{Capability, ProviderId};

/// A provider family registered with the application.
///
/// Implementations are supplied by the adapter crate. The trait is object-safe
/// so a registry can hold heterogeneous providers behind [`Arc`].
pub trait Provider: Send + Sync {
    /// Returns the immutable identifier for this provider.
    fn id(&self) -> ProviderId;

    /// Returns the capabilities this provider declares.
    ///
    /// The slice is static because capability declarations are fixed at
    /// startup. An empty slice means the provider declares no capabilities.
    fn capabilities(&self) -> &'static [Capability];
}

/// A shared, type-erased provider handle.
pub type ProviderRef = Arc<dyn Provider>;
