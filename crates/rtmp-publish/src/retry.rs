//! Bounded exponential backoff with injected jitter.
//!
//! The caller supplies the jitter value, so schedules are deterministic in
//! tests and offline planning. Only transport failures are retryable;
//! authentication failures never are.

use std::time::Duration;

use crate::error::PublishError;
use crate::probe::Health;

/// The maximum number of retries after the initial attempt.
pub const MAX_RETRIES: u32 = 10;

/// A bounded retry policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetryPolicy {
    /// Delay before the first retry.
    pub base: Duration,
    /// Number of retries after the initial attempt, `1..=10`.
    pub max_attempts: u32,
    /// Upper bound for any single delay.
    pub cap: Duration,
}

impl RetryPolicy {
    /// Validates and creates a retry policy.
    pub fn new(base: Duration, max_attempts: u32, cap: Duration) -> Result<Self, PublishError> {
        let policy = Self {
            base,
            max_attempts,
            cap,
        };
        policy.validate()?;
        Ok(policy)
    }

    /// Validates the policy bounds.
    pub fn validate(&self) -> Result<(), PublishError> {
        if self.base.is_zero() {
            return Err(PublishError::InvalidPolicy {
                detail: "retry base must be non-zero".to_string(),
            });
        }
        if self.max_attempts == 0 || self.max_attempts > MAX_RETRIES {
            return Err(PublishError::InvalidPolicy {
                detail: format!("retry attempts must be 1..={MAX_RETRIES}"),
            });
        }
        if self.cap < self.base {
            return Err(PublishError::InvalidPolicy {
                detail: "retry cap must be at least the base delay".to_string(),
            });
        }
        Ok(())
    }

    /// Returns the delay before retry `attempt` (zero-based).
    ///
    /// The exponential value is `base * 2^attempt`, capped at `cap`. The
    /// injected `jitter` in `0.0..=1.0` scales the delay to 50-100% of that
    /// value, so the result is always at or below the cap.
    #[must_use]
    pub fn delay(&self, attempt: u32, jitter: f64) -> Duration {
        let factor = 1_u32 << attempt.min(16);
        let raw = self.base.saturating_mul(factor).min(self.cap);
        let jitter = if jitter.is_finite() {
            jitter.clamp(0.0, 1.0)
        } else {
            0.0
        };
        raw.mul_f64(0.5 + 0.5 * jitter)
    }

    /// Returns the retry plan for a probe result.
    ///
    /// `AuthRejected`, `Reachable`, and `Unknown` produce an empty plan.
    #[must_use]
    pub fn plan(&self, health: &Health, jitter: f64) -> RetryPlan {
        let retry = health.is_retryable();
        let delays = if retry {
            (0..self.max_attempts)
                .map(|attempt| self.delay(attempt, jitter))
                .collect()
        } else {
            Vec::new()
        };
        RetryPlan {
            retry,
            total_delay: delays.iter().sum(),
            delays,
        }
    }
}

/// A computed retry schedule.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryPlan {
    /// Whether the failure is retryable.
    pub retry: bool,
    /// Delay before each retry, in order.
    pub delays: Vec<Duration>,
    /// Sum of all delays, bounded by `attempts * cap`.
    pub total_delay: Duration,
}
