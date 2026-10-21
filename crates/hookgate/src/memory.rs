use std::collections::{HashMap, VecDeque};
use std::fmt;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;

use crate::store::now;
use crate::{
    DeadLetterRecord, DeliveryId, DeliveryRecord, DeliveryStatus, DeliveryStore, InsertOutcome,
    Outbox, OutboxError, OutboxMessage, ProviderId, StoreError, TenantId,
};

type DeliveryKey = (TenantId, DeliveryId);
type DedupeKey = (TenantId, ProviderId, String);

/// An in-memory [`DeliveryStore`] and [`Outbox`] for tests and small services.
///
/// Delivery insert, dedupe, and outbox enqueue happen under one mutex, so a
/// duplicate delivery never writes a second outbox row. The store is not
/// durable; use a database-backed implementation for production.
pub struct MemoryDeliveryStore {
    max_attempts: u32,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    deliveries: HashMap<DeliveryKey, DeliveryRecord>,
    dedupe: HashMap<DedupeKey, DeliveryId>,
    messages: HashMap<DeliveryKey, OutboxMessage>,
    outbox: VecDeque<OutboxMessage>,
    dead_letters: Vec<DeadLetterRecord>,
}

impl Default for MemoryDeliveryStore {
    fn default() -> Self {
        Self::new()
    }
}

impl MemoryDeliveryStore {
    /// Creates a store with a five-attempt dead-letter threshold.
    pub fn new() -> Self {
        Self {
            max_attempts: 5,
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Creates a store with a custom dead-letter threshold.
    ///
    /// A value of zero is treated as one.
    pub fn with_max_attempts(max_attempts: u32) -> Self {
        Self {
            max_attempts: max_attempts.max(1),
            inner: Mutex::new(Inner::default()),
        }
    }

    /// Returns the dead-letter threshold.
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, StoreError> {
        self.inner
            .lock()
            .map_err(|_| StoreError::Backend("delivery store lock poisoned".to_owned()))
    }
}

impl fmt::Debug for MemoryDeliveryStore {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let counts = self
            .inner
            .lock()
            .map(|inner| {
                (
                    inner.deliveries.len(),
                    inner.outbox.len(),
                    inner.dead_letters.len(),
                )
            })
            .unwrap_or((0, 0, 0));

        f.debug_struct("MemoryDeliveryStore")
            .field("max_attempts", &self.max_attempts)
            .field("deliveries", &counts.0)
            .field("outbox", &counts.1)
            .field("dead_letters", &counts.2)
            .finish()
    }
}

#[async_trait]
impl DeliveryStore for MemoryDeliveryStore {
    async fn insert_if_new(
        &self,
        record: DeliveryRecord,
        message: OutboxMessage,
    ) -> Result<InsertOutcome, StoreError> {
        if message.delivery != record.delivery {
            return Err(StoreError::Config(
                "outbox message does not match the delivery".to_owned(),
            ));
        }
        if message.tenant != record.tenant {
            return Err(StoreError::Config(
                "outbox message tenant does not match the delivery".to_owned(),
            ));
        }

        let mut inner = self.lock()?;
        let key = (record.tenant, record.delivery);
        if inner.deliveries.contains_key(&key) {
            return Ok(InsertOutcome::Duplicate);
        }

        let dedupe_key = (record.tenant, record.provider, record.event_id.clone());
        if inner.dedupe.contains_key(&dedupe_key) {
            return Ok(InsertOutcome::Duplicate);
        }

        inner.dedupe.insert(dedupe_key, record.delivery);
        inner.messages.insert(key, message.clone());
        inner.deliveries.insert(key, record);
        inner.outbox.push_back(message);
        Ok(InsertOutcome::Inserted)
    }

    async fn mark_processed(&self, tenant: TenantId, id: DeliveryId) -> Result<(), StoreError> {
        let mut inner = self.lock()?;
        let record = inner
            .deliveries
            .get_mut(&(tenant, id))
            .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
        record.status = DeliveryStatus::Processed;
        Ok(())
    }

    async fn fail(
        &self,
        tenant: TenantId,
        id: DeliveryId,
        error: &str,
    ) -> Result<Option<DeadLetterRecord>, StoreError> {
        let error = crate::redact_error(error);
        let mut inner = self.lock()?;
        let key = (tenant, id);

        let dead_letter = {
            let record = inner
                .deliveries
                .get_mut(&key)
                .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
            record.attempts = record.attempts.saturating_add(1);

            if record.attempts >= self.max_attempts {
                record.status = DeliveryStatus::DeadLettered;
                Some(DeadLetterRecord {
                    delivery: record.delivery,
                    tenant: record.tenant,
                    provider: record.provider,
                    raw: record.raw.clone(),
                    error,
                    dead_lettered_at: now(),
                })
            } else {
                record.status = DeliveryStatus::Failed;
                None
            }
        };

        if let Some(dead_letter) = &dead_letter {
            inner.dead_letters.push(dead_letter.clone());
        }
        Ok(dead_letter)
    }

    async fn get(
        &self,
        tenant: TenantId,
        id: DeliveryId,
    ) -> Result<Option<DeliveryRecord>, StoreError> {
        let inner = self.lock()?;
        Ok(inner.deliveries.get(&(tenant, id)).cloned())
    }

    async fn dead_letters(&self, tenant: TenantId) -> Result<Vec<DeadLetterRecord>, StoreError> {
        let inner = self.lock()?;
        Ok(inner
            .dead_letters
            .iter()
            .filter(|record| record.tenant == tenant)
            .cloned()
            .collect())
    }

    async fn replay(&self, tenant: TenantId, id: DeliveryId) -> Result<InsertOutcome, StoreError> {
        let mut inner = self.lock()?;
        let key = (tenant, id);

        {
            let record = inner
                .deliveries
                .get_mut(&key)
                .ok_or_else(|| StoreError::NotFound(id.to_string()))?;
            if record.status == DeliveryStatus::Processed {
                return Ok(InsertOutcome::Duplicate);
            }
            record.status = DeliveryStatus::Pending;
            record.attempts = 0;
        }

        let message = inner
            .messages
            .get(&key)
            .cloned()
            .ok_or_else(|| StoreError::Backend("outbox message is missing".to_owned()))?;
        inner.outbox.push_back(message);
        Ok(InsertOutcome::Inserted)
    }
}

#[async_trait]
impl Outbox for MemoryDeliveryStore {
    async fn enqueue(&self, message: OutboxMessage) -> Result<(), OutboxError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| OutboxError::Backend("outbox lock poisoned".to_owned()))?;
        inner.outbox.push_back(message);
        Ok(())
    }

    async fn pop(&self) -> Result<Option<OutboxMessage>, OutboxError> {
        let mut inner = self
            .inner
            .lock()
            .map_err(|_| OutboxError::Backend("outbox lock poisoned".to_owned()))?;
        Ok(inner.outbox.pop_front())
    }

    async fn len(&self) -> Result<usize, OutboxError> {
        let inner = self
            .inner
            .lock()
            .map_err(|_| OutboxError::Backend("outbox lock poisoned".to_owned()))?;
        Ok(inner.outbox.len())
    }
}
