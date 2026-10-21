use std::fmt;

use uuid::Uuid;

/// A tenant in a multi-tenant product.
///
/// Tenant identifiers are opaque to every crate in this workspace. They are
/// carried in every store key and every tenant-scoped query.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct TenantId(Uuid);

impl TenantId {
    /// Wraps an existing UUID.
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Generates a new time-ordered tenant identifier.
    pub fn generate() -> Self {
        Self(Uuid::now_v7())
    }

    /// Returns the underlying UUID.
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }
}

impl fmt::Debug for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("TenantId").field(&self.0).finish()
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

impl From<Uuid> for TenantId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

impl From<TenantId> for Uuid {
    fn from(value: TenantId) -> Self {
        value.0
    }
}

/// A third-party provider family known to the application.
///
/// The identifier is a static string because providers are registered at
/// startup and live for the process lifetime. The crate never enumerates the
/// valid values; the vocabulary belongs to the consuming application.
#[derive(Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProviderId(&'static str);

impl ProviderId {
    /// Creates a provider identifier from a static name.
    pub const fn new(value: &'static str) -> Self {
        Self(value)
    }

    /// Returns the provider name.
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl fmt::Debug for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_tuple("ProviderId").field(&self.0).finish()
    }
}

impl fmt::Display for ProviderId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.0)
    }
}

impl From<&'static str> for ProviderId {
    fn from(value: &'static str) -> Self {
        Self(value)
    }
}
