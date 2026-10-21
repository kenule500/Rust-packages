use std::fmt;

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde_json::Value;
use thiserror::Error;

use crate::{DeadLetterRecord, DeliveryId, ProviderId, TenantId, VerifiedEvent};

/// The processing state of a delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeliveryStatus {
    /// The delivery is waiting to be handled.
    Pending,
    /// A handler attempt failed, but the delivery may still be retried.
    Failed,
    /// The handler completed successfully.
    Processed,
    /// The delivery exceeded the configured attempts and was dead-lettered.
    DeadLettered,
}

/// A durable record of one inbound delivery.
#[derive(Clone)]
pub struct DeliveryRecord {
    /// The delivery identity.
    pub delivery: DeliveryId,
    /// The tenant that owns the connection.
    pub tenant: TenantId,
    /// The provider family that sent the request.
    pub provider: ProviderId,
    /// The provider's event identifier.
    pub event_id: String,
    /// The provider's event type.
    pub event_type: String,
    /// The exact bytes that were verified.
    pub raw: Bytes,
    /// The current processing state.
    pub status: DeliveryStatus,
    /// The number of failed handler attempts.
    pub attempts: u32,
}

impl DeliveryRecord {
    /// Builds a pending record from a verified event.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError::Config`] when the provider sent no event
    /// identifier. Deduplication requires one; supply a provider-specific
    /// fallback before calling this helper.
    pub fn from_event(event: &VerifiedEvent) -> Result<Self, StoreError> {
        let event_id = event.event_id.clone().ok_or_else(|| {
            StoreError::Config(format!(
                "provider `{}` sent no event identifier",
                event.provider
            ))
        })?;

        Ok(Self {
            delivery: event.delivery,
            tenant: event.tenant,
            provider: event.provider,
            event_id,
            event_type: event.event_type.clone(),
            raw: event.raw.clone(),
            status: DeliveryStatus::Pending,
            attempts: 0,
        })
    }
}

impl fmt::Debug for DeliveryRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeliveryRecord")
            .field("delivery", &self.delivery)
            .field("tenant", &self.tenant)
            .field("provider", &self.provider)
            .field("event_id", &self.event_id)
            .field("event_type", &self.event_type)
            .field("raw_len", &self.raw.len())
            .field("status", &self.status)
            .field("attempts", &self.attempts)
            .finish()
    }
}

/// One queued outbox message.
///
/// `insert_if_new` writes the delivery and this message under one lock, so a
/// duplicate delivery never produces a second message.
#[derive(Clone)]
pub struct OutboxMessage {
    /// The delivery this message belongs to.
    pub delivery: DeliveryId,
    /// The tenant that owns the connection.
    pub tenant: TenantId,
    /// The provider family that sent the request.
    pub provider: ProviderId,
    /// The provider's event type.
    pub event_type: String,
    /// The parsed payload.
    pub payload: Value,
}

impl OutboxMessage {
    /// Builds a message from a verified event.
    pub fn from_event(event: &VerifiedEvent) -> Self {
        Self {
            delivery: event.delivery,
            tenant: event.tenant,
            provider: event.provider,
            event_type: event.event_type.clone(),
            payload: event.payload.clone(),
        }
    }
}

impl fmt::Debug for OutboxMessage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("OutboxMessage")
            .field("delivery", &self.delivery)
            .field("tenant", &self.tenant)
            .field("provider", &self.provider)
            .field("event_type", &self.event_type)
            .field("payload", &"<redacted>")
            .finish()
    }
}

/// The result of inserting a delivery.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum InsertOutcome {
    /// The delivery was new; the outbox message was enqueued.
    Inserted,
    /// The tenant, provider, and event id were already present; nothing was
    /// enqueued.
    Duplicate,
}

/// A durable delivery store with transactional insert plus enqueue.
#[async_trait]
pub trait DeliveryStore: Send + Sync {
    /// Inserts a delivery and enqueues its outbox message atomically.
    ///
    /// Returns [`InsertOutcome::Duplicate`] when the tuple of tenant,
    /// provider, and event id already exists. A duplicate never enqueues a
    /// second message.
    async fn insert_if_new(
        &self,
        record: DeliveryRecord,
        message: OutboxMessage,
    ) -> Result<InsertOutcome, StoreError>;

    /// Marks a delivery as processed.
    async fn mark_processed(&self, tenant: TenantId, id: DeliveryId) -> Result<(), StoreError>;

    /// Records a failed handler attempt.
    ///
    /// Returns a [`DeadLetterRecord`] when the attempt count reaches the
    /// store's configured maximum. The error text is redacted before storage.
    async fn fail(
        &self,
        tenant: TenantId,
        id: DeliveryId,
        error: &str,
    ) -> Result<Option<DeadLetterRecord>, StoreError>;

    /// Reads a delivery.
    async fn get(
        &self,
        tenant: TenantId,
        id: DeliveryId,
    ) -> Result<Option<DeliveryRecord>, StoreError>;

    /// Lists dead-letter records for a tenant.
    async fn dead_letters(&self, tenant: TenantId) -> Result<Vec<DeadLetterRecord>, StoreError>;

    /// Re-enqueues a dead-lettered or failed delivery with the same id.
    ///
    /// A delivery that already succeeded returns [`InsertOutcome::Duplicate`]
    /// and is not enqueued again.
    async fn replay(&self, tenant: TenantId, id: DeliveryId) -> Result<InsertOutcome, StoreError>;
}

/// A queue of outbox messages.
#[async_trait]
pub trait Outbox: Send + Sync {
    /// Enqueues a message outside the delivery transaction.
    ///
    /// Prefer [`DeliveryStore::insert_if_new`] when the message belongs to a
    /// delivery, because that method keeps the insert and enqueue atomic.
    async fn enqueue(&self, message: OutboxMessage) -> Result<(), OutboxError>;

    /// Removes and returns the oldest message.
    async fn pop(&self) -> Result<Option<OutboxMessage>, OutboxError>;

    /// Returns the number of queued messages.
    async fn len(&self) -> Result<usize, OutboxError>;

    /// Returns `true` when the queue is empty.
    async fn is_empty(&self) -> Result<bool, OutboxError> {
        Ok(self.len().await? == 0)
    }
}

/// An error returned by a delivery store.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum StoreError {
    /// The delivery does not exist for this tenant.
    #[error("HG-STORE-NOT-FOUND: {0}")]
    NotFound(String),
    /// The store configuration or request is invalid.
    #[error("HG-STORE-CONFIG: {0}")]
    Config(String),
    /// The backend failed.
    #[error("HG-STORE-BACKEND: {0}")]
    Backend(String),
}

/// An error returned by an outbox.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum OutboxError {
    /// The outbox backend failed.
    #[error("HG-OUTBOX-BACKEND: {0}")]
    Backend(String),
    /// The outbox is closed and cannot accept messages.
    #[error("HG-OUTBOX-CLOSED: the outbox is closed")]
    Closed,
}

/// The current instant, used by memory implementations for timestamps.
pub(crate) fn now() -> DateTime<Utc> {
    Utc::now()
}
