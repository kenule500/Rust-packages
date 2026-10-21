use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::{Action, ActionId, ConflictInfo};

/// The disposition of one action after a transport call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionStatus {
    /// The server applied the action.
    Applied,
    /// The server had already applied this action.
    Duplicate,
    /// The server reported a conflict.
    Conflict,
    /// The server rejected the action permanently.
    Rejected,
    /// The action may be retried.
    Retry,
}

impl ActionStatus {
    /// Returns the stable lowercase name.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Applied => "applied",
            Self::Duplicate => "duplicate",
            Self::Conflict => "conflict",
            Self::Rejected => "rejected",
            Self::Retry => "retry",
        }
    }
}

/// A bounded group of actions sent in one transport call.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Batch {
    /// The actions in enqueue order.
    pub actions: Vec<Action>,
}

impl Batch {
    /// Creates a batch.
    pub fn new(actions: Vec<Action>) -> Self {
        Self { actions }
    }

    /// Returns the number of actions.
    pub fn len(&self) -> usize {
        self.actions.len()
    }

    /// Returns `true` when the batch has no actions.
    pub fn is_empty(&self) -> bool {
        self.actions.is_empty()
    }

    /// Iterates over the actions in order.
    pub fn iter(&self) -> std::slice::Iter<'_, Action> {
        self.actions.iter()
    }

    /// Consumes the batch and returns its actions.
    pub fn into_actions(self) -> Vec<Action> {
        self.actions
    }
}

/// The server's result for one action.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionResult {
    /// The action this result describes.
    pub action_id: ActionId,
    /// The disposition.
    pub status: ActionStatus,
    /// The server's timestamp for this action, when it supplied one.
    pub server_timestamp: Option<DateTime<Utc>>,
    /// The conflict details when `status` is [`ActionStatus::Conflict`].
    pub conflict: Option<ConflictInfo>,
}

impl ActionResult {
    /// Builds a result with no timestamp and no conflict.
    pub fn new(action_id: ActionId, status: ActionStatus) -> Self {
        Self {
            action_id,
            status,
            server_timestamp: None,
            conflict: None,
        }
    }

    /// Builds an applied result stamped at the current instant.
    pub fn applied(action_id: ActionId) -> Self {
        Self::new(action_id, ActionStatus::Applied).with_server_timestamp(Utc::now())
    }

    /// Builds a duplicate result stamped at the current instant.
    pub fn duplicate(action_id: ActionId) -> Self {
        Self::new(action_id, ActionStatus::Duplicate).with_server_timestamp(Utc::now())
    }

    /// Builds a rejected result stamped at the current instant.
    pub fn rejected(action_id: ActionId) -> Self {
        Self::new(action_id, ActionStatus::Rejected).with_server_timestamp(Utc::now())
    }

    /// Builds a retryable result stamped at the current instant.
    pub fn retry(action_id: ActionId) -> Self {
        Self::new(action_id, ActionStatus::Retry).with_server_timestamp(Utc::now())
    }

    /// Builds a conflict result from the server's conflict report.
    pub fn conflict(action_id: ActionId, conflict: ConflictInfo) -> Self {
        let server_timestamp = conflict.server_timestamp;
        Self {
            action_id,
            status: ActionStatus::Conflict,
            server_timestamp: Some(server_timestamp),
            conflict: Some(conflict),
        }
    }

    /// Replaces the server timestamp.
    pub fn with_server_timestamp(mut self, server_timestamp: DateTime<Utc>) -> Self {
        self.server_timestamp = Some(server_timestamp);
        self
    }

    /// Attaches conflict information.
    pub fn with_conflict(mut self, conflict: ConflictInfo) -> Self {
        self.conflict = Some(conflict);
        self
    }
}

/// The results of one replay.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct BatchResult {
    /// One result per attempted action, in enqueue order.
    pub results: Vec<ActionResult>,
}

impl BatchResult {
    /// Creates an empty result set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Builds a result set.
    pub fn from_results(results: Vec<ActionResult>) -> Self {
        Self { results }
    }

    /// Appends a result.
    pub fn push(&mut self, result: ActionResult) {
        self.results.push(result);
    }

    /// Returns the number of results.
    pub fn len(&self) -> usize {
        self.results.len()
    }

    /// Returns `true` when no action was attempted.
    pub fn is_empty(&self) -> bool {
        self.results.is_empty()
    }

    /// Returns the number of conflict results.
    pub fn conflicts(&self) -> usize {
        self.results
            .iter()
            .filter(|result| result.status == ActionStatus::Conflict)
            .count()
    }

    /// Finds the result for an action.
    pub fn find(&self, action_id: ActionId) -> Option<&ActionResult> {
        self.results
            .iter()
            .find(|result| result.action_id == action_id)
    }

    /// Consumes the result set and returns its results.
    pub fn into_results(self) -> Vec<ActionResult> {
        self.results
    }
}
