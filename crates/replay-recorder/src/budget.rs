//! Disk budget and pressure policy.

use serde::{Deserialize, Serialize};

use crate::error::RecorderError;
use crate::track::TrackId;

/// What to do when the budget is reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressurePolicy {
    /// Delete the oldest complete segment and continue.
    StopOldest,
    /// Stop recording on the affected track.
    StopNewest,
    /// Refuse the write with a typed error.
    Refuse,
}

/// The action taken after pressure was detected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PressureAction {
    /// Oldest segments were deleted.
    DeletedOldest,
    /// The track was stopped.
    StoppedTrack,
    /// The write was refused.
    Refused,
}

/// An emitted pressure event.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PressureEvent {
    /// Configured policy.
    pub policy: PressurePolicy,
    /// Action actually taken.
    pub action: PressureAction,
    /// Bytes used before the action.
    pub used_bytes: u64,
    /// Configured maximum.
    pub max_bytes: u64,
    /// Free bytes reported by the caller.
    pub free_bytes: u64,
    /// Configured minimum free space.
    pub min_free_bytes: u64,
    /// Track affected, when known.
    pub track: Option<TrackId>,
}

/// Disk budget configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiskBudget {
    /// Maximum bytes the spool may occupy.
    pub max_bytes: u64,
    /// Minimum free bytes that must remain on the volume.
    pub min_free_bytes: u64,
    /// Policy applied when either limit is reached.
    pub on_pressure: PressurePolicy,
}

impl DiskBudget {
    /// Creates a budget.
    #[must_use]
    pub fn new(max_bytes: u64, min_free_bytes: u64, on_pressure: PressurePolicy) -> Self {
        Self {
            max_bytes,
            min_free_bytes,
            on_pressure,
        }
    }

    /// Validates the budget values.
    pub fn validate(&self) -> Result<(), RecorderError> {
        if self.max_bytes == 0 {
            return Err(RecorderError::InvalidConfig {
                detail: "max_bytes must be non-zero".to_string(),
            });
        }
        Ok(())
    }

    /// Evaluates whether a write of `additional` bytes is allowed.
    #[must_use]
    pub fn evaluate(&self, used_bytes: u64, free_bytes: u64, additional: u64) -> BudgetDecision {
        let over_cap = used_bytes.saturating_add(additional) > self.max_bytes;
        let under_floor = free_bytes.saturating_sub(additional) < self.min_free_bytes;
        if over_cap || under_floor {
            BudgetDecision::Pressure {
                policy: self.on_pressure,
                over_cap,
                under_floor,
            }
        } else {
            BudgetDecision::Allowed
        }
    }
}

/// The outcome of a budget check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDecision {
    /// The write fits.
    Allowed,
    /// The write triggers the configured policy.
    Pressure {
        /// Configured policy.
        policy: PressurePolicy,
        /// Whether the byte cap was exceeded.
        over_cap: bool,
        /// Whether the free-space floor would be crossed.
        under_floor: bool,
    },
}
