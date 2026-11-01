#[cfg(feature = "memory")]
use std::collections::HashMap;
use std::fmt;
#[cfg(feature = "memory")]
use std::sync::RwLock;

use async_trait::async_trait;
use secrecy::{ExposeSecret, SecretString};
use thiserror::Error;

use crate::{ProviderId, TenantId};

/// A credential value that is redacted from `Debug` and `Display`.
///
/// The wrapper keeps secret material behind an explicit [`Secret::expose`]
/// call, does not implement `Serialize`, and zeroizes its buffer on drop
/// (through `secrecy`).
pub struct Secret(SecretString);

impl Secret {
    /// Wraps a credential value.
    pub fn new(value: impl Into<String>) -> Self {
        Self(SecretString::new(value.into().into_boxed_str()))
    }

    /// Exposes the credential value.
    ///
    /// Call this only at the point where the credential is sent to the
    /// provider. Never log the result.
    pub fn expose(&self) -> &str {
        self.0.expose_secret()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret([REDACTED])")
    }
}

/// An error returned by a [`TokenStore`] backend.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum TokenError {
    /// The backend failed to read or write the credential.
    #[error("PC-TOKEN-BACKEND: {0}")]
    Backend(String),
    /// The backend configuration is invalid.
    #[error("PC-TOKEN-CONFIG: {0}")]
    Config(String),
}

/// A storage-agnostic credential store keyed by tenant and provider.
///
/// The trait is async and object-safe. Backends own encryption, rotation, and
/// durability; `platform-core` owns only the contract.
#[async_trait]
pub trait TokenStore: Send + Sync {
    /// Reads the credential for a tenant and provider.
    async fn get(
        &self,
        tenant: TenantId,
        provider: ProviderId,
    ) -> Result<Option<Secret>, TokenError>;

    /// Writes or replaces the credential for a tenant and provider.
    async fn put(
        &self,
        tenant: TenantId,
        provider: ProviderId,
        token: Secret,
    ) -> Result<(), TokenError>;

    /// Removes the credential for a tenant and provider.
    ///
    /// Revoking an absent credential is not an error.
    async fn revoke(&self, tenant: TenantId, provider: ProviderId) -> Result<(), TokenError>;
}

/// An in-memory [`TokenStore`] for tests and single-binary tools.
///
/// Entries are keyed by `(TenantId, ProviderId)`, so a lookup cannot cross
/// tenants. The store is not durable and must not be used as production
/// credential storage.
#[cfg(feature = "memory")]
#[derive(Default)]
pub struct MemoryTokenStore {
    entries: RwLock<HashMap<(TenantId, ProviderId), Secret>>,
}

#[cfg(feature = "memory")]
impl MemoryTokenStore {
    /// Creates an empty token store.
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns the number of stored credentials.
    pub fn len(&self) -> usize {
        self.entries.read().map(|guard| guard.len()).unwrap_or(0)
    }

    /// Returns `true` when no credentials are stored.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(feature = "memory")]
impl fmt::Debug for MemoryTokenStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MemoryTokenStore")
            .field("entries", &format_args!("<redacted>"))
            .field("len", &self.len())
            .finish()
    }
}

#[cfg(feature = "memory")]
#[async_trait]
impl TokenStore for MemoryTokenStore {
    async fn get(
        &self,
        tenant: TenantId,
        provider: ProviderId,
    ) -> Result<Option<Secret>, TokenError> {
        let guard = self
            .entries
            .read()
            .map_err(|_| TokenError::Backend("token store lock poisoned".to_owned()))?;
        Ok(guard
            .get(&(tenant, provider))
            .map(|token| Secret::new(token.expose())))
    }

    async fn put(
        &self,
        tenant: TenantId,
        provider: ProviderId,
        token: Secret,
    ) -> Result<(), TokenError> {
        let mut guard = self
            .entries
            .write()
            .map_err(|_| TokenError::Backend("token store lock poisoned".to_owned()))?;
        guard.insert((tenant, provider), token);
        Ok(())
    }

    async fn revoke(&self, tenant: TenantId, provider: ProviderId) -> Result<(), TokenError> {
        let mut guard = self
            .entries
            .write()
            .map_err(|_| TokenError::Backend("token store lock poisoned".to_owned()))?;
        guard.remove(&(tenant, provider));
        Ok(())
    }
}
