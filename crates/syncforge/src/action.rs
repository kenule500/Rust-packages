use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::{ContextV7, Timestamp, Uuid};

use crate::SyncError;

static LAST_MILLIS: AtomicU64 = AtomicU64::new(0);

/// A time-ordered action identifier.
///
/// Identifiers are UUID v7 values. The timestamp component is monotonically
/// non-decreasing within the process, including when two actions are created in
/// the same millisecond.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ActionId(Uuid);

impl ActionId {
    /// Generates a new UUID v7 with a process-monotonic timestamp component.
    pub fn generate() -> Self {
        let millis = next_millis();
        let seconds = millis / 1_000;
        let nanos = ((millis % 1_000) * 1_000_000) as u32;
        let timestamp = Timestamp::from_unix(ContextV7::new(), seconds, nanos);
        Self(Uuid::new_v7(timestamp))
    }

    /// Wraps an existing UUID.
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }

    /// Returns the underlying UUID.
    pub const fn as_uuid(&self) -> &Uuid {
        &self.0
    }

    /// Returns the UUID v7 timestamp component in milliseconds.
    pub fn timestamp_millis(&self) -> u64 {
        self.0
            .get_timestamp()
            .map(|timestamp| {
                let (seconds, nanos) = timestamp.to_unix();
                seconds
                    .saturating_mul(1_000)
                    .saturating_add(u64::from(nanos / 1_000_000))
            })
            .unwrap_or(0)
    }
}

impl fmt::Display for ActionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(f)
    }
}

/// The kind of entity an action targets.
///
/// The value is application-defined, for example `work_order`, `invoice`, or
/// `contact`. Keeping it as a string lets one queue carry several entities
/// without an enum migration.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EntityType(String);

impl EntityType {
    /// Creates an entity type.
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// Returns the entity type as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for EntityType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<&str> for EntityType {
    fn from(value: &str) -> Self {
        Self::new(value)
    }
}

impl From<String> for EntityType {
    fn from(value: String) -> Self {
        Self::new(value)
    }
}

impl AsRef<str> for EntityType {
    fn as_ref(&self) -> &str {
        self.as_str()
    }
}

/// The operation an action performs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Operation {
    /// Create an entity.
    Create,
    /// Update an entity.
    Update,
    /// Delete an entity.
    Delete,
}

impl Operation {
    /// Returns the stable lowercase name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Create => "create",
            Self::Update => "update",
            Self::Delete => "delete",
        }
    }
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// One durable client action.
///
/// An action is created with a UUID v7 [`ActionId`], its own idempotency key,
/// the client's clock reading, and zero attempts. The client timestamp is audit
/// data: replay never rewrites it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    /// The action identity, generated at creation.
    pub id: ActionId,
    /// The entity type the action targets.
    pub entity: EntityType,
    /// The operation to perform.
    pub operation: Operation,
    /// The action payload.
    pub payload: Value,
    /// The client's timestamp at creation.
    pub client_timestamp: DateTime<Utc>,
    /// The idempotency key sent with the action.
    pub idempotency_key: Uuid,
    /// The number of attempts already made.
    pub attempts: u32,
}

impl Action {
    /// Creates an action with a generated id, idempotency key, and timestamp.
    pub fn new(entity: impl Into<EntityType>, operation: Operation, payload: Value) -> Self {
        Self {
            id: ActionId::generate(),
            entity: entity.into(),
            operation,
            payload,
            client_timestamp: Utc::now(),
            idempotency_key: Uuid::now_v7(),
            attempts: 0,
        }
    }

    /// Replaces the client timestamp, for tests and imports.
    pub fn with_client_timestamp(mut self, client_timestamp: DateTime<Utc>) -> Self {
        self.client_timestamp = client_timestamp;
        self
    }

    /// Replaces the action identifier.
    pub fn with_id(mut self, id: ActionId) -> Self {
        self.id = id;
        self
    }

    /// Replaces the idempotency key.
    pub fn with_idempotency_key(mut self, idempotency_key: Uuid) -> Self {
        self.idempotency_key = idempotency_key;
        self
    }

    /// Replaces the attempt count.
    pub fn with_attempts(mut self, attempts: u32) -> Self {
        self.attempts = attempts;
        self
    }

    /// Validates the fields that must never be empty or nil.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidAction`] for an empty entity type, a nil
    /// action id, or a nil idempotency key.
    pub fn validate(&self) -> Result<(), SyncError> {
        if self.entity.as_str().trim().is_empty() {
            return Err(SyncError::InvalidAction(
                "entity type must not be empty".to_owned(),
            ));
        }
        if self.id.as_uuid().is_nil() {
            return Err(SyncError::InvalidAction(
                "action id must not be nil".to_owned(),
            ));
        }
        if self.idempotency_key.is_nil() {
            return Err(SyncError::InvalidAction(
                "idempotency key must not be nil".to_owned(),
            ));
        }
        Ok(())
    }
}

fn next_millis() -> u64 {
    let now = u64::try_from(Utc::now().timestamp_millis()).unwrap_or(0);
    let mut last = LAST_MILLIS.load(Ordering::Relaxed);

    loop {
        let next = if now > last {
            now
        } else {
            last.saturating_add(1)
        };
        match LAST_MILLIS.compare_exchange_weak(last, next, Ordering::Relaxed, Ordering::Relaxed) {
            Ok(_) => return next,
            Err(current) => last = current,
        }
    }
}
