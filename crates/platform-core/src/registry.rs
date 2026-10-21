use std::collections::HashMap;
use std::fmt;

use crate::{Capability, IntegrationError, ProviderId, ProviderRef};

/// An explicit, instance-scoped collection of providers.
///
/// Registration happens at startup. There is no global mutable singleton and
/// no lazy registration: two registries in the same process are independent.
#[derive(Default)]
pub struct Registry {
    providers: Vec<ProviderRef>,
    index: HashMap<ProviderId, usize>,
}

impl Registry {
    /// Creates an empty registry.
    pub fn new() -> Self {
        Self::default()
    }

    /// Registers a provider.
    ///
    /// # Errors
    ///
    /// Returns [`IntegrationError::DuplicateProvider`] when a provider with the
    /// same identifier is already registered. The original entry is preserved.
    pub fn register(&mut self, provider: ProviderRef) -> Result<(), IntegrationError> {
        let id = provider.id();
        if self.index.contains_key(&id) {
            return Err(IntegrationError::duplicate_provider(id));
        }
        let position = self.providers.len();
        self.providers.push(provider);
        self.index.insert(id, position);
        Ok(())
    }

    /// Looks up a provider by identifier.
    pub fn get(&self, id: &ProviderId) -> Option<&ProviderRef> {
        self.index
            .get(id)
            .and_then(|position| self.providers.get(*position))
    }

    /// Iterates over providers in registration order.
    pub fn iter(&self) -> impl Iterator<Item = &ProviderRef> {
        self.providers.iter()
    }

    /// Returns the declared capabilities for a provider.
    pub fn capabilities(&self, id: &ProviderId) -> Option<&'static [Capability]> {
        self.get(id).map(|provider| provider.capabilities())
    }

    /// Returns the number of registered providers.
    pub fn len(&self) -> usize {
        self.providers.len()
    }

    /// Returns `true` when no providers are registered.
    pub fn is_empty(&self) -> bool {
        self.providers.is_empty()
    }
}

impl fmt::Debug for Registry {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let ids: Vec<&str> = self.providers.iter().map(|p| p.id().as_str()).collect();
        f.debug_struct("Registry").field("providers", &ids).finish()
    }
}
