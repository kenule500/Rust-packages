//! Backup promotion and cool-down demotion.
//!
//! A single state machine makes promotion decisions, so the active destination
//! cannot flap between primary and backup on every probe.

use std::time::{Duration, Instant};

use crate::destination::{Destination, DestinationId};
use crate::error::PublishError;
use crate::probe::Health;

/// Which destination is active.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailoverPhase {
    /// The primary is active.
    Primary,
    /// The backup is active.
    Backup,
}

/// The result of one evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FailoverDecision {
    /// The active destination did not change.
    Stay,
    /// The backup became active.
    Promoted {
        /// The promoted destination.
        to: DestinationId,
    },
    /// The primary became active again.
    Demoted {
        /// The restored destination.
        to: DestinationId,
    },
    /// The primary breaker is open but no backup exists.
    NoBackup,
}

/// Primary and optional backup with a single promotion phase.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Failover {
    primary: DestinationId,
    backup: Option<DestinationId>,
    phase: FailoverPhase,
    promoted_at: Option<Instant>,
}

impl Failover {
    /// Creates failover state from two identifiers.
    ///
    /// A backup equal to the primary is rejected as a self-reference.
    pub fn new(
        primary: DestinationId,
        backup: Option<DestinationId>,
    ) -> Result<Self, PublishError> {
        if backup.as_ref() == Some(&primary) {
            return Err(PublishError::BackupCycle {
                id: primary.to_string(),
            });
        }
        Ok(Self {
            primary,
            backup,
            phase: FailoverPhase::Primary,
            promoted_at: None,
        })
    }

    /// Creates failover state from a validated destination chain.
    ///
    /// The first backup is used. The destination must already be validated, so
    /// self-reference and cycles have been rejected.
    pub fn from_primary(primary: &Destination) -> Self {
        Self {
            primary: primary.id().clone(),
            backup: primary.backup().map(|backup| backup.id().clone()),
            phase: FailoverPhase::Primary,
            promoted_at: None,
        }
    }

    /// Returns the active destination identifier.
    #[must_use]
    pub fn active_id(&self) -> &DestinationId {
        match self.phase {
            FailoverPhase::Primary => &self.primary,
            FailoverPhase::Backup => self.backup.as_ref().unwrap_or(&self.primary),
        }
    }

    /// Returns the current phase.
    #[must_use]
    pub const fn phase(&self) -> FailoverPhase {
        self.phase
    }

    /// Returns when promotion happened.
    #[must_use]
    pub const fn promoted_at(&self) -> Option<Instant> {
        self.promoted_at
    }

    /// Evaluates one probe cycle.
    ///
    /// - Primary active, breaker open, backup present: promote.
    /// - Backup active, cool-down elapsed, primary reachable, breaker closed:
    ///   demote.
    /// - Backup active otherwise: stay, even if the backup is unhealthy. There
    ///   is no second backup, so demoting early would only add churn.
    pub fn evaluate(
        &mut self,
        primary_health: &Health,
        breaker_open: bool,
        now: Instant,
        cooldown: Duration,
    ) -> FailoverDecision {
        match self.phase {
            FailoverPhase::Primary => {
                if breaker_open && self.backup.is_some() {
                    self.phase = FailoverPhase::Backup;
                    self.promoted_at = Some(now);
                    FailoverDecision::Promoted {
                        to: self.active_id().clone(),
                    }
                } else if breaker_open {
                    FailoverDecision::NoBackup
                } else {
                    FailoverDecision::Stay
                }
            }
            FailoverPhase::Backup => {
                let cooled = self
                    .promoted_at
                    .is_some_and(|promoted| now.saturating_duration_since(promoted) >= cooldown);
                if cooled && !breaker_open && primary_health.is_reachable() {
                    self.phase = FailoverPhase::Primary;
                    self.promoted_at = None;
                    FailoverDecision::Demoted {
                        to: self.primary.clone(),
                    }
                } else {
                    FailoverDecision::Stay
                }
            }
        }
    }
}
