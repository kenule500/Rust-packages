//! Per-destination circuit breaker.
//!
//! The breaker is a small state machine with three states and bounded failure
//! counting. It never retries on its own; it only reports whether a probe is
//! allowed and transitions on recorded health.

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::destination::DestinationId;
use crate::error::PublishError;
use crate::probe::Health;

/// Circuit-breaker states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BreakerState {
    /// Probes are allowed.
    Closed,
    /// Probes are refused until the cool-down elapses.
    Open,
    /// One probe is allowed to test recovery.
    HalfOpen,
}

/// Breaker thresholds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BreakerConfig {
    /// Consecutive failures that open the breaker, `1..=10`.
    pub failure_threshold: u32,
    /// Time the breaker stays open before a half-open probe.
    pub cooldown: Duration,
}

impl Default for BreakerConfig {
    fn default() -> Self {
        Self {
            failure_threshold: 3,
            cooldown: Duration::from_secs(30),
        }
    }
}

impl BreakerConfig {
    /// Validates and creates a breaker configuration.
    pub fn new(failure_threshold: u32, cooldown: Duration) -> Result<Self, PublishError> {
        if failure_threshold == 0 || failure_threshold > 10 {
            return Err(PublishError::InvalidPolicy {
                detail: "breaker threshold must be 1..=10".to_string(),
            });
        }
        if cooldown.is_zero() {
            return Err(PublishError::InvalidPolicy {
                detail: "breaker cool-down must be non-zero".to_string(),
            });
        }
        Ok(Self {
            failure_threshold,
            cooldown,
        })
    }
}

/// One destination's breaker state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CircuitBreaker {
    /// Current state.
    pub state: BreakerState,
    /// Consecutive failure count.
    pub failures: u32,
    /// When the breaker opened.
    pub opened_at: Option<Instant>,
}

impl Default for CircuitBreaker {
    fn default() -> Self {
        Self {
            state: BreakerState::Closed,
            failures: 0,
            opened_at: None,
        }
    }
}

impl CircuitBreaker {
    /// Creates a closed breaker.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns whether the breaker is open.
    #[must_use]
    pub const fn is_open(&self) -> bool {
        matches!(self.state, BreakerState::Open)
    }

    /// Returns whether a probe is allowed now.
    ///
    /// An open breaker moves to half-open once the cool-down has elapsed.
    pub fn allow(&mut self, now: Instant, config: &BreakerConfig) -> bool {
        match self.state {
            BreakerState::Closed => true,
            BreakerState::HalfOpen => true,
            BreakerState::Open => {
                let opened = self.opened_at.unwrap_or(now);
                if now.saturating_duration_since(opened) >= config.cooldown {
                    self.state = BreakerState::HalfOpen;
                    true
                } else {
                    false
                }
            }
        }
    }

    /// Records one probe result.
    ///
    /// A reachable endpoint closes the breaker. Authentication rejection counts
    /// as a failure because repeating the same key cannot succeed. `Unknown`
    /// leaves the breaker unchanged.
    pub fn record(&mut self, health: &Health, now: Instant, config: &BreakerConfig) {
        match health {
            Health::Reachable { .. } => {
                self.state = BreakerState::Closed;
                self.failures = 0;
                self.opened_at = None;
            }
            Health::Unknown => {}
            Health::AuthRejected | Health::Unreachable(_) => {
                self.failures = self.failures.saturating_add(1);
                if self.failures >= config.failure_threshold {
                    self.state = BreakerState::Open;
                    self.opened_at = Some(now);
                }
            }
        }
    }
}

/// Breakers keyed by destination.
///
/// One entry exists per destination the caller has probed, so memory is bounded
/// by the caller's destination set rather than by probe volume.
#[derive(Debug, Clone, Default)]
pub struct BreakerSet {
    breakers: BTreeMap<DestinationId, CircuitBreaker>,
}

impl BreakerSet {
    /// Creates an empty set.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns a breaker, creating a closed one on first use.
    pub fn get_mut(&mut self, destination: &DestinationId) -> &mut CircuitBreaker {
        self.breakers.entry(destination.clone()).or_default()
    }

    /// Returns a breaker without creating one.
    #[must_use]
    pub fn get(&self, destination: &DestinationId) -> Option<&CircuitBreaker> {
        self.breakers.get(destination)
    }

    /// Returns whether a destination's breaker is open.
    #[must_use]
    pub fn is_open(&self, destination: &DestinationId) -> bool {
        self.get(destination).is_some_and(CircuitBreaker::is_open)
    }

    /// Returns the number of tracked destinations.
    #[must_use]
    pub fn len(&self) -> usize {
        self.breakers.len()
    }

    /// Returns whether no destination is tracked.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.breakers.is_empty()
    }
}
