use std::fmt;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;

use crate::{Action, ActionId, SyncError};

/// Durable storage for the action queue.
///
/// A backend persists an action before acknowledging it, lists pending actions
/// in enqueue order, and records settlement and attempt counts. Storage is
/// injected explicitly; there is no ambient global queue.
#[async_trait]
pub trait Storage: Send + Sync {
    /// Persists a new action.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::InvalidAction`] for a malformed action and
    /// [`SyncError::DuplicateAction`] when the id already exists.
    async fn append(&self, action: Action) -> Result<(), SyncError>;

    /// Lists unsettled actions in enqueue order.
    async fn pending(&self) -> Result<Vec<Action>, SyncError>;

    /// Marks an action as sent.
    async fn mark_sent(&self, id: ActionId) -> Result<(), SyncError>;

    /// Marks an action as permanently failed or dead-lettered.
    async fn mark_failed(&self, id: ActionId) -> Result<(), SyncError>;

    /// Records the latest attempt count, leaving the action pending.
    async fn record_attempt(&self, id: ActionId, attempts: u32) -> Result<(), SyncError>;
}

/// An in-memory [`Storage`] with deterministic enqueue order.
///
/// The store is a single mutex-protected vector. It is intended for tests,
/// single-process tools, and examples; use a durable backend for production.
pub struct MemoryStorage {
    entries: Mutex<Vec<Entry>>,
}

struct Entry {
    action: Action,
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum State {
    Pending,
    Sent,
    Failed,
}

impl MemoryStorage {
    /// Creates an empty store.
    pub fn new() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }

    /// Returns the number of stored actions, including settled ones.
    pub fn len(&self) -> usize {
        self.entries
            .lock()
            .map(|entries| entries.len())
            .unwrap_or(0)
    }

    /// Returns `true` when the store has no actions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn lock(&self) -> Result<MutexGuard<'_, Vec<Entry>>, SyncError> {
        self.entries
            .lock()
            .map_err(|_| SyncError::Storage("memory storage lock poisoned".to_owned()))
    }
}

impl Default for MemoryStorage {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MemoryStorage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (total, pending) = self
            .entries
            .lock()
            .map(|entries| {
                (
                    entries.len(),
                    entries
                        .iter()
                        .filter(|entry| entry.state == State::Pending)
                        .count(),
                )
            })
            .unwrap_or((0, 0));

        f.debug_struct("MemoryStorage")
            .field("actions", &total)
            .field("pending", &pending)
            .finish()
    }
}

#[async_trait]
impl Storage for MemoryStorage {
    async fn append(&self, action: Action) -> Result<(), SyncError> {
        action.validate()?;
        let mut entries = self.lock()?;

        if entries.iter().any(|entry| entry.action.id == action.id) {
            return Err(SyncError::DuplicateAction(action.id.to_string()));
        }

        entries.push(Entry {
            action,
            state: State::Pending,
        });
        Ok(())
    }

    async fn pending(&self) -> Result<Vec<Action>, SyncError> {
        let entries = self.lock()?;
        Ok(entries
            .iter()
            .filter(|entry| entry.state == State::Pending)
            .map(|entry| entry.action.clone())
            .collect())
    }

    async fn mark_sent(&self, id: ActionId) -> Result<(), SyncError> {
        let mut entries = self.lock()?;
        let entry = entries
            .iter_mut()
            .find(|entry| entry.action.id == id)
            .ok_or_else(|| SyncError::NotFound(id.to_string()))?;

        match entry.state {
            State::Pending => {
                entry.state = State::Sent;
                Ok(())
            }
            State::Sent => Ok(()),
            State::Failed => Err(SyncError::InvalidState(format!(
                "action {id} is already failed"
            ))),
        }
    }

    async fn mark_failed(&self, id: ActionId) -> Result<(), SyncError> {
        let mut entries = self.lock()?;
        let entry = entries
            .iter_mut()
            .find(|entry| entry.action.id == id)
            .ok_or_else(|| SyncError::NotFound(id.to_string()))?;

        match entry.state {
            State::Pending | State::Failed => {
                entry.state = State::Failed;
                Ok(())
            }
            State::Sent => Err(SyncError::InvalidState(format!(
                "action {id} is already sent"
            ))),
        }
    }

    async fn record_attempt(&self, id: ActionId, attempts: u32) -> Result<(), SyncError> {
        let mut entries = self.lock()?;
        let entry = entries
            .iter_mut()
            .find(|entry| entry.action.id == id)
            .ok_or_else(|| SyncError::NotFound(id.to_string()))?;

        if entry.state != State::Pending {
            return Err(SyncError::InvalidState(format!(
                "action {id} is not pending"
            )));
        }

        entry.action.attempts = attempts;
        Ok(())
    }
}
