//! The mandatory link store contract.
//!
//! Stateless signature verification alone does not make a link revocable or
//! single-use. Revocation and nonce consumption are enforced by the store.
//!
//! # Implementation obligations
//!
//! - Revocation and consumption must be atomic per code.
//! - SQL implementations must use parameterised queries, and Redis or other
//!   stores must use equivalent argument binding. Never concatenate a code, a
//!   token, or a claim into a command string.
//! - Failures must return [`LinkError::Store`]; a store must never panic.
//! - The store is the authority: an implementation may not rely on a client
//!   having checked a link.

use std::collections::{HashMap, HashSet};
use std::future::Future;
use std::sync::Mutex;

use crate::claims::Claims;
use crate::code::Code;
use crate::error::LinkError;

/// Persistence required for revocation and single use.
pub trait LinkStore: Send + Sync {
    /// Stores or replaces the claims for a code.
    fn put(
        &self,
        code: &Code,
        claims: &Claims,
    ) -> impl Future<Output = Result<(), LinkError>> + Send;

    /// Returns the active claims, or `None` for missing, revoked, or consumed
    /// codes. Use [`LinkStore::is_revoked`] and [`LinkStore::is_consumed`] to
    /// distinguish the three cases.
    fn get(&self, code: &Code) -> impl Future<Output = Result<Option<Claims>, LinkError>> + Send;

    /// Revokes a code. Returns whether the code existed or is now revoked.
    fn revoke(&self, code: &Code) -> impl Future<Output = Result<bool, LinkError>> + Send;

    /// Atomically consumes a code. Returns `true` only for the first caller.
    fn consume(&self, code: &Code) -> impl Future<Output = Result<bool, LinkError>> + Send;

    /// Returns whether a code is revoked.
    ///
    /// The default reports `false`, which is only safe when the implementation
    /// removes revoked entries and treats `None` as revoked. Implement it when
    /// the three states must remain distinguishable.
    fn is_revoked(&self, _code: &Code) -> impl Future<Output = Result<bool, LinkError>> + Send {
        async { Ok(false) }
    }

    /// Returns whether a code was consumed.
    ///
    /// The default reports `false`; implement it to return a distinct
    /// [`LinkError::Consumed`] instead of [`LinkError::NotFound`].
    fn is_consumed(&self, _code: &Code) -> impl Future<Output = Result<bool, LinkError>> + Send {
        async { Ok(false) }
    }
}

/// A bounded-scope in-memory store for examples and tests.
///
/// It is not a production store: entries are never evicted and the process owns
/// all state, so revocation does not survive a restart. It exists to show the
/// contract and to make the example runnable offline.
#[derive(Debug, Default)]
pub struct MemoryLinkStore {
    links: Mutex<HashMap<String, Claims>>,
    revoked: Mutex<HashSet<String>>,
    consumed: Mutex<HashSet<String>>,
}

impl MemoryLinkStore {
    /// Creates an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    fn links(&self) -> Result<std::sync::MutexGuard<'_, HashMap<String, Claims>>, LinkError> {
        self.links.lock().map_err(|_| LinkError::Store {
            detail: "link map is poisoned".to_string(),
        })
    }

    fn revoked(&self) -> Result<std::sync::MutexGuard<'_, HashSet<String>>, LinkError> {
        self.revoked.lock().map_err(|_| LinkError::Store {
            detail: "revocation set is poisoned".to_string(),
        })
    }

    fn consumed(&self) -> Result<std::sync::MutexGuard<'_, HashSet<String>>, LinkError> {
        self.consumed.lock().map_err(|_| LinkError::Store {
            detail: "consumption set is poisoned".to_string(),
        })
    }
}

impl LinkStore for MemoryLinkStore {
    async fn put(&self, code: &Code, claims: &Claims) -> Result<(), LinkError> {
        self.links()?
            .insert(code.as_str().to_string(), claims.clone());
        Ok(())
    }

    async fn get(&self, code: &Code) -> Result<Option<Claims>, LinkError> {
        if self.revoked()?.contains(code.as_str()) || self.consumed()?.contains(code.as_str()) {
            return Ok(None);
        }
        Ok(self.links()?.get(code.as_str()).cloned())
    }

    async fn revoke(&self, code: &Code) -> Result<bool, LinkError> {
        let existed = self.links()?.remove(code.as_str()).is_some();
        self.revoked()?.insert(code.as_str().to_string());
        Ok(existed)
    }

    async fn consume(&self, code: &Code) -> Result<bool, LinkError> {
        if self.revoked()?.contains(code.as_str()) || self.consumed()?.contains(code.as_str()) {
            return Ok(false);
        }
        let existed = self.links()?.remove(code.as_str()).is_some();
        if existed {
            self.consumed()?.insert(code.as_str().to_string());
        }
        Ok(existed)
    }

    async fn is_revoked(&self, code: &Code) -> Result<bool, LinkError> {
        Ok(self.revoked()?.contains(code.as_str()))
    }

    async fn is_consumed(&self, code: &Code) -> Result<bool, LinkError> {
        Ok(self.consumed()?.contains(code.as_str()))
    }
}
