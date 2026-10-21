//! Fixed-window per-participant message budget.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};

use crate::envelope::ParticipantId;
use crate::error::Rejected;

/// Messages allowed per participant per wall-clock second.
pub const MESSAGES_PER_SECOND: u32 = 50;

/// A fixed-window counter with no queue.
///
/// The caller decides whether a rate-limited message is dropped or retried. The
/// map only holds one counter per participant, so memory does not grow with
/// message volume.
#[derive(Debug, Clone)]
pub struct MessageBudget {
    limit: u32,
    window: BTreeMap<ParticipantId, (i64, u32)>,
}

impl Default for MessageBudget {
    fn default() -> Self {
        Self::per_second()
    }
}

impl MessageBudget {
    /// Creates a budget with the given per-second limit. Zero is treated as one.
    #[must_use]
    pub fn new(limit: u32) -> Self {
        Self {
            limit: limit.max(1),
            window: BTreeMap::new(),
        }
    }

    /// Creates the standard 50-messages-per-second budget.
    #[must_use]
    pub fn per_second() -> Self {
        Self::new(MESSAGES_PER_SECOND)
    }

    /// Returns the configured limit.
    #[must_use]
    pub const fn limit(&self) -> u32 {
        self.limit
    }

    /// Records one message and returns `RateLimited` when the window is full.
    pub fn allow(
        &mut self,
        participant: &ParticipantId,
        now: DateTime<Utc>,
    ) -> Result<(), Rejected> {
        let second = now.timestamp();
        let entry = self.window.entry(*participant).or_insert((second, 0));
        if entry.0 != second {
            *entry = (second, 0);
        }
        if entry.1 >= self.limit {
            return Err(Rejected::RateLimited {
                participant: *participant,
            });
        }
        entry.1 += 1;
        Ok(())
    }

    /// Returns the messages remaining in the current window.
    #[must_use]
    pub fn remaining(&self, participant: &ParticipantId, now: DateTime<Utc>) -> u32 {
        match self.window.get(participant) {
            Some((second, used)) if *second == now.timestamp() => self.limit.saturating_sub(*used),
            _ => self.limit,
        }
    }
}
