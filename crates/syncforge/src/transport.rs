use std::collections::{HashMap, HashSet, VecDeque};
use std::fmt;
use std::sync::{Mutex, MutexGuard};

use async_trait::async_trait;

use crate::{Action, ActionId, ActionResult, ActionStatus, Batch, BatchResult, SyncError};

/// Sends a batch of actions to a server.
///
/// A transport returns exactly one [`ActionResult`] per action. Results may be
/// in any order; the engine matches them by [`ActionId`].
#[async_trait]
pub trait Transport: Send + Sync {
    /// Sends one batch.
    ///
    /// # Errors
    ///
    /// Returns [`SyncError::Transport`] when the whole batch could not be
    /// delivered. Per-action failures use [`ActionResult`] instead.
    async fn send(&self, batch: &Batch) -> Result<BatchResult, SyncError>;
}

/// A scripted [`Transport`] for examples and tests.
///
/// By default every unseen action is `Applied`; repeating an action id returns
/// `Duplicate` instead of applying it twice. A test can script a whole-batch
/// response with [`MockTransport::script_batch`] or one action's response with
/// [`MockTransport::script_action`].
pub struct MockTransport {
    default: ActionStatus,
    inner: Mutex<MockInner>,
}

#[derive(Default)]
struct MockInner {
    batch_scripts: VecDeque<BatchResult>,
    action_scripts: HashMap<ActionId, VecDeque<ActionResult>>,
    seen: HashSet<ActionId>,
    applied: Vec<ActionId>,
    calls: usize,
}

impl MockTransport {
    /// Creates a transport whose default outcome is `Applied`.
    pub fn new() -> Self {
        Self::with_default(ActionStatus::Applied)
    }

    /// Creates a transport with a default outcome for unseen actions.
    pub fn with_default(default: ActionStatus) -> Self {
        Self {
            default,
            inner: Mutex::new(MockInner::default()),
        }
    }

    /// Queues a whole-batch response. Responses are consumed in FIFO order.
    pub fn script_batch(&self, result: BatchResult) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.batch_scripts.push_back(result);
        }
    }

    /// Queues one action's response. Repeated calls for the same id are
    /// consumed FIFO across replay calls.
    pub fn script_action(&self, result: ActionResult) {
        if let Ok(mut inner) = self.inner.lock() {
            inner
                .action_scripts
                .entry(result.action_id)
                .or_default()
                .push_back(result);
        }
    }

    /// Returns the number of transport calls made.
    pub fn calls(&self) -> usize {
        self.lock().map(|inner| inner.calls).unwrap_or(0)
    }

    /// Returns the action ids applied exactly once, in application order.
    pub fn applied(&self) -> Vec<ActionId> {
        self.lock()
            .map(|inner| inner.applied.clone())
            .unwrap_or_default()
    }

    /// Returns the number of actions applied exactly once.
    pub fn applied_count(&self) -> usize {
        self.lock().map(|inner| inner.applied.len()).unwrap_or(0)
    }

    /// Returns `true` when the action id has been seen by the mock server.
    pub fn is_seen(&self, id: ActionId) -> bool {
        self.lock()
            .map(|inner| inner.seen.contains(&id))
            .unwrap_or(false)
    }

    fn lock(&self) -> Result<MutexGuard<'_, MockInner>, SyncError> {
        self.inner
            .lock()
            .map_err(|_| SyncError::Transport("mock transport lock poisoned".to_owned()))
    }

    fn default_result(&self, inner: &mut MockInner, action: &Action) -> ActionResult {
        if inner.seen.contains(&action.id) {
            return ActionResult::duplicate(action.id);
        }

        match self.default {
            ActionStatus::Applied => {
                inner.seen.insert(action.id);
                inner.applied.push(action.id);
                ActionResult::applied(action.id)
            }
            ActionStatus::Duplicate => {
                inner.seen.insert(action.id);
                ActionResult::duplicate(action.id)
            }
            ActionStatus::Rejected => ActionResult::rejected(action.id),
            ActionStatus::Retry => ActionResult::retry(action.id),
            ActionStatus::Conflict => ActionResult::new(action.id, ActionStatus::Conflict),
        }
    }
}

impl Default for MockTransport {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for MockTransport {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (calls, applied) = self
            .lock()
            .map(|inner| (inner.calls, inner.applied.len()))
            .unwrap_or((0, 0));

        f.debug_struct("MockTransport")
            .field("default", &self.default)
            .field("calls", &calls)
            .field("applied", &applied)
            .finish()
    }
}

#[async_trait]
impl Transport for MockTransport {
    async fn send(&self, batch: &Batch) -> Result<BatchResult, SyncError> {
        let mut inner = self.lock()?;
        inner.calls = inner.calls.saturating_add(1);

        if let Some(script) = inner.batch_scripts.pop_front() {
            for result in &script.results {
                match result.status {
                    ActionStatus::Applied => {
                        if inner.seen.insert(result.action_id) {
                            inner.applied.push(result.action_id);
                        }
                    }
                    ActionStatus::Duplicate | ActionStatus::Conflict => {
                        inner.seen.insert(result.action_id);
                    }
                    ActionStatus::Rejected | ActionStatus::Retry => {}
                }
            }
            return Ok(script);
        }

        let mut results = Vec::with_capacity(batch.len());
        for action in &batch.actions {
            let scripted = inner
                .action_scripts
                .get_mut(&action.id)
                .and_then(VecDeque::pop_front);
            let result = match scripted {
                Some(result) => result,
                None => self.default_result(&mut inner, action),
            };

            match result.status {
                ActionStatus::Applied => {
                    if inner.seen.insert(result.action_id) {
                        inner.applied.push(result.action_id);
                    }
                }
                ActionStatus::Duplicate | ActionStatus::Conflict => {
                    inner.seen.insert(result.action_id);
                }
                ActionStatus::Rejected | ActionStatus::Retry => {}
            }

            results.push(result);
        }

        Ok(BatchResult::from_results(results))
    }
}
