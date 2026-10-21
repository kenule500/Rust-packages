use std::collections::HashMap;
use std::fmt;
use std::sync::Arc;
use std::time::Instant;

use chrono::{DateTime, Duration, Utc};
use serde_json::Value;
use tracing::{debug, info, warn};

use crate::timestamp::{self, TimestampDecision};
use crate::{
    Action, ActionId, ActionResult, ActionStatus, Batch, BatchResult, ConflictStrategy, EntityType,
    Operation, Storage, SyncError, Transport,
};

/// The default attempt ceiling before an action is dead-lettered.
pub const DEFAULT_MAX_ATTEMPTS: u32 = 5;
/// The default number of actions sent per transport call.
pub const DEFAULT_BATCH_SIZE: usize = 50;
/// The default clock-skew tolerance in minutes.
pub const DEFAULT_TOLERANCE_MINUTES: i64 = 5;
/// The first retry delay, in milliseconds.
pub const BASE_BACKOFF_MILLIS: u64 = 250;
/// The retry delay ceiling, in milliseconds.
pub const MAX_BACKOFF_MILLIS: u64 = 30_000;

/// Drains a durable queue through a transport in bounded batches.
///
/// The engine persists an action before acknowledging it, replays pending
/// actions in enqueue order, and settles each action independently. A retrying
/// or dead-lettered action never blocks later actions.
///
/// The engine never sleeps. Retryable actions stay pending with an incremented
/// attempt count; [`SyncEngine::retry_delay`] exposes the capped exponential
/// delay for the host scheduler.
pub struct SyncEngine {
    storage: Arc<dyn Storage>,
    transport: Arc<dyn Transport>,
    conflict: ConflictStrategy,
    max_attempts: u32,
    batch_size: usize,
    tolerance: Duration,
}

impl SyncEngine {
    /// Creates an engine with default settings.
    ///
    /// Defaults: `LastWriteWins`, five attempts, batches of 50, and a
    /// five-minute tolerance.
    pub fn new(storage: Arc<dyn Storage>, transport: Arc<dyn Transport>) -> Self {
        Self {
            storage,
            transport,
            conflict: ConflictStrategy::default(),
            max_attempts: DEFAULT_MAX_ATTEMPTS,
            batch_size: DEFAULT_BATCH_SIZE,
            tolerance: Duration::minutes(DEFAULT_TOLERANCE_MINUTES),
        }
    }

    /// Selects the conflict strategy.
    pub fn with_conflict(mut self, conflict: ConflictStrategy) -> Self {
        self.conflict = conflict;
        self
    }

    /// Sets the attempt ceiling. Values below one are treated as one.
    pub fn with_max_attempts(mut self, max_attempts: u32) -> Self {
        self.max_attempts = max_attempts.max(1);
        self
    }

    /// Sets the transport batch size. Values below one are treated as one.
    pub fn with_batch_size(mut self, batch_size: usize) -> Self {
        self.batch_size = batch_size.max(1);
        self
    }

    /// Sets the clock-skew tolerance used for timestamp audit decisions.
    pub fn with_tolerance(mut self, tolerance: Duration) -> Self {
        self.tolerance = tolerance;
        self
    }

    /// Returns the configured conflict strategy.
    pub fn conflict(&self) -> &ConflictStrategy {
        &self.conflict
    }

    /// Returns the attempt ceiling.
    pub fn max_attempts(&self) -> u32 {
        self.max_attempts
    }

    /// Returns the transport batch size.
    pub fn batch_size(&self) -> usize {
        self.batch_size
    }

    /// Returns the clock-skew tolerance.
    pub fn tolerance(&self) -> Duration {
        self.tolerance
    }

    /// Persists an action and returns its id.
    ///
    /// The action is durable before `Ok` is returned. A storage failure is
    /// returned as a typed error.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidAction`] for a malformed action, and the
    /// backend's [`SyncError`] when the append fails.
    pub async fn enqueue(&self, action: Action) -> Result<ActionId, SyncError> {
        action.validate()?;
        let id = action.id;
        self.storage.append(action).await?;

        let queue_depth = match self.storage.pending().await {
            Ok(pending) => pending.len(),
            Err(_) => 0,
        };
        debug!(
            action_id = %id,
            queue_depth,
            "syncforge.enqueue"
        );
        Ok(id)
    }

    /// Creates and persists an action, returning the enqueued action.
    ///
    /// # Errors
    ///
    /// Returns the same typed errors as [`SyncEngine::enqueue`].
    pub async fn enqueue_new(
        &self,
        entity: impl Into<EntityType>,
        operation: Operation,
        payload: Value,
    ) -> Result<Action, SyncError> {
        let action = Action::new(entity, operation, payload);
        self.enqueue(action.clone()).await?;
        Ok(action)
    }

    /// Replays every pending action in bounded batches.
    ///
    /// Each attempted action receives exactly one [`ActionResult`] in enqueue
    /// order. `Applied` and `Duplicate` settle the action, `Rejected` marks it
    /// failed, a `Conflict` is resolved and settled, and `Retry` increments the
    /// attempt count. Reaching the attempt ceiling marks the action failed and
    /// reports it as `Rejected`; later actions still replay.
    ///
    /// # Errors
    ///
    /// Returns a typed storage, transport, conflict, or protocol error.
    pub async fn replay(&self) -> Result<BatchResult, SyncError> {
        let started = Instant::now();
        let pending = self.storage.pending().await?;

        info!(
            pending = pending.len(),
            batch_size = self.batch_size,
            "syncforge.replay.start"
        );

        if pending.is_empty() {
            let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
            info!(
                duration_ms,
                results = 0usize,
                conflicts = 0usize,
                queue_depth = 0usize,
                "syncforge.replay.finish"
            );
            return Ok(BatchResult::default());
        }

        let mut results = Vec::with_capacity(pending.len());
        let mut conflicts = 0usize;

        for chunk in pending.chunks(self.batch_size) {
            let batch = Batch::new(chunk.to_vec());
            let response = self.transport.send(&batch).await?;
            let mut by_id: HashMap<ActionId, ActionResult> = response
                .results
                .into_iter()
                .map(|result| (result.action_id, result))
                .collect();

            for action in chunk {
                let mut result = by_id
                    .remove(&action.id)
                    .ok_or_else(|| SyncError::MissingResult(action.id.to_string()))?;

                if result.server_timestamp.is_none() {
                    result.server_timestamp = Some(
                        result
                            .conflict
                            .as_ref()
                            .map(|conflict| conflict.server_timestamp)
                            .unwrap_or_else(Utc::now),
                    );
                }

                let mut recorded_attempts = action.attempts;

                match result.status {
                    ActionStatus::Applied | ActionStatus::Duplicate => {
                        self.storage.mark_sent(action.id).await?;
                    }
                    ActionStatus::Rejected => {
                        self.storage.mark_failed(action.id).await?;
                    }
                    ActionStatus::Retry => {
                        let attempts = action.attempts.saturating_add(1);
                        recorded_attempts = attempts;
                        self.storage.record_attempt(action.id, attempts).await?;

                        if attempts >= self.max_attempts {
                            self.storage.mark_failed(action.id).await?;
                            result.status = ActionStatus::Rejected;
                            warn!(
                                action_id = %action.id,
                                attempts,
                                "syncforge.action.dead_letter"
                            );
                        } else {
                            let delay = Self::retry_delay(attempts);
                            debug!(
                                action_id = %action.id,
                                attempts,
                                delay_ms = delay.num_milliseconds(),
                                "syncforge.action.retry"
                            );
                        }
                    }
                    ActionStatus::Conflict => {
                        let mut conflict = result.conflict.clone().ok_or_else(|| {
                            SyncError::InvalidConflict(format!(
                                "action {} reported a conflict without conflict information",
                                action.id
                            ))
                        })?;
                        let decision =
                            self.conflict
                                .resolve(action, &mut conflict, self.tolerance)?;
                        let resolution = conflict.resolution.as_str();
                        result.conflict = Some(conflict);
                        self.storage.mark_sent(action.id).await?;
                        conflicts = conflicts.saturating_add(1);
                        info!(
                            action_id = %action.id,
                            strategy = ?self.conflict,
                            resolution,
                            within_tolerance = decision.within_tolerance,
                            skew_ms = decision.skew.num_milliseconds(),
                            "syncforge.conflict"
                        );
                    }
                }

                debug!(
                    action_id = %action.id,
                    status = result.status.as_str(),
                    attempts = recorded_attempts,
                    "syncforge.action.outcome"
                );
                results.push(result);
            }
        }

        let queue_depth = self.storage.pending().await?.len();
        let duration_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        info!(
            duration_ms,
            results = results.len(),
            conflicts,
            queue_depth,
            "syncforge.replay.finish"
        );

        Ok(BatchResult::from_results(results))
    }

    /// Audits a client timestamp against a server timestamp.
    pub fn timestamp_decision(
        &self,
        action: &Action,
        server_timestamp: DateTime<Utc>,
    ) -> TimestampDecision {
        timestamp::decide(action.client_timestamp, server_timestamp, self.tolerance)
    }

    /// Returns the capped exponential delay for the given attempt count.
    ///
    /// The delay starts at [`BASE_BACKOFF_MILLIS`], doubles per attempt, and is
    /// capped at [`MAX_BACKOFF_MILLIS`]. The engine records the delay but never
    /// sleeps, so the host scheduler decides when to replay again.
    pub fn retry_delay(attempts: u32) -> Duration {
        let exponent = attempts.saturating_sub(1).min(7);
        let millis = BASE_BACKOFF_MILLIS
            .saturating_mul(1_u64 << exponent)
            .min(MAX_BACKOFF_MILLIS);
        Duration::milliseconds(i64::try_from(millis).unwrap_or(i64::MAX))
    }
}

impl fmt::Debug for SyncEngine {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SyncEngine")
            .field("conflict", &self.conflict)
            .field("max_attempts", &self.max_attempts)
            .field("batch_size", &self.batch_size)
            .field("tolerance", &self.tolerance)
            .finish_non_exhaustive()
    }
}
