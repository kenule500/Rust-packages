//! Per-provider token buckets.
//!
//! Scope: a hand-rolled token bucket with per-provider (and per-account)
//! overrides, a non-blocking acquire that yields to the async runtime, and a
//! tracing event for every rate-limit hit (CON-209, NFR-012).
//!
//! Non-goals: distributed quota accounting (that is `quota-ledger`, CON-107),
//! blocking waits (NFR-005), and provider-specific rate-limit headers — the
//! shared retry loop already honours `Retry-After`.

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use crate::error::WirekitError;

/// One provider's (or one account's) rate limit.
///
/// `capacity` is the burst size and `refill_per_second` is the sustained rate.
/// A config of `capacity = 1, refill_per_second = 1.0` allows one request per
/// second with no burst.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RateLimitConfig {
    /// Maximum tokens held in the bucket.
    pub capacity: u32,
    /// Tokens restored per second.
    pub refill_per_second: f64,
}

impl RateLimitConfig {
    /// Build a configuration, rejecting a zero capacity or a non-positive
    /// refill rate.
    pub fn new(capacity: u32, refill_per_second: f64) -> Result<Self, WirekitError> {
        if capacity == 0 {
            return Err(WirekitError::invalid_config(
                "rate limit capacity must be at least 1",
            ));
        }
        if !refill_per_second.is_finite() || refill_per_second <= 0.0 {
            return Err(WirekitError::invalid_config(
                "rate limit refill_per_second must be a positive, finite number",
            ));
        }
        Ok(Self {
            capacity,
            refill_per_second,
        })
    }

    /// A sustained rate with a one-second burst.
    pub fn per_second(requests_per_second: f64) -> Result<Self, WirekitError> {
        let capacity = requests_per_second.ceil().max(1.0) as u32;
        Self::new(capacity, requests_per_second)
    }
}

struct BucketState {
    available: f64,
    last_refill: Instant,
}

/// A token bucket for one provider or one account.
///
/// Cloning the `Arc` shares the bucket, so concurrent requests for the same
/// provider are throttled in aggregate (EC-015).
pub struct TokenBucket {
    config: RateLimitConfig,
    /// Tokens restored per second, taken from the configuration.
    refill_rate: f64,
    state: Mutex<BucketState>,
}

impl TokenBucket {
    /// Create a bucket that starts full.
    #[must_use]
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            refill_rate: config.refill_per_second,
            state: Mutex::new(BucketState {
                available: f64::from(config.capacity),
                last_refill: Instant::now(),
            }),
        }
    }

    /// The configuration this bucket was built from.
    #[must_use]
    pub fn config(&self) -> RateLimitConfig {
        self.config
    }

    /// Tokens currently available, after applying elapsed refills.
    #[must_use]
    pub fn available(&self) -> f64 {
        let mut state = lock(&self.state);
        refill(&mut state, self.config);
        state.available
    }

    /// Take one token without waiting.
    ///
    /// Returns `None` when a token was taken, or the wait needed before one is
    /// available.
    #[must_use]
    pub fn try_acquire(&self) -> Option<Duration> {
        let mut state = lock(&self.state);
        refill(&mut state, self.config);
        if state.available >= 1.0 {
            state.available -= 1.0;
            None
        } else {
            let deficit = 1.0 - state.available;
            Some(Duration::from_secs_f64(deficit / self.refill_rate))
        }
    }

    /// Wait, yielding to the runtime, until one token is available.
    ///
    /// Emits a tracing event for each wait. The lock is never held across an
    /// `await`, so no runtime thread is blocked (NFR-005).
    pub async fn acquire(&self) {
        loop {
            match self.try_acquire() {
                None => return,
                Some(wait) => {
                    tracing::debug!(
                        target: "wirekit::rate_limit",
                        wait_ms = wait.as_millis() as u64,
                        capacity = self.config.capacity,
                        refill_per_second = self.refill_rate,
                        "rate limit hit; waiting for a token"
                    );
                    tokio::time::sleep(wait).await;
                }
            }
        }
    }
}

fn refill(state: &mut BucketState, config: RateLimitConfig) {
    let elapsed = state.last_refill.elapsed().as_secs_f64();
    if elapsed > 0.0 {
        state.available =
            (state.available + elapsed * config.refill_per_second).min(f64::from(config.capacity));
        state.last_refill = Instant::now();
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // A poisoned lock means another thread panicked while holding it. The
    // bucket state is still usable, and panicking here would violate the
    // crate's no-panic rule.
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The set of buckets owned by one client, keyed by provider or account.
pub(crate) struct RateLimiter {
    default: Option<RateLimitConfig>,
    overrides: HashMap<String, RateLimitConfig>,
    buckets: Mutex<HashMap<String, Arc<TokenBucket>>>,
}

impl RateLimiter {
    pub(crate) fn new(
        default: Option<RateLimitConfig>,
        overrides: HashMap<String, RateLimitConfig>,
    ) -> Self {
        Self {
            default,
            overrides,
            buckets: Mutex::new(HashMap::new()),
        }
    }

    /// The configuration that applies to `key`: an exact match, then the
    /// segment before `:` (so `twilio:AC123` inherits `twilio`), then the
    /// client default.
    fn config_for(&self, key: &str) -> Option<RateLimitConfig> {
        if let Some(config) = self.overrides.get(key) {
            return Some(*config);
        }
        if let Some((prefix, _)) = key.split_once(':') {
            if let Some(config) = self.overrides.get(prefix) {
                return Some(*config);
            }
        }
        self.default
    }

    pub(crate) fn bucket_for(&self, key: &str) -> Option<Arc<TokenBucket>> {
        let config = self.config_for(key)?;
        let mut buckets = lock(&self.buckets);
        Some(
            buckets
                .entry(key.to_string())
                .or_insert_with(|| Arc::new(TokenBucket::new(config)))
                .clone(),
        )
    }

    pub(crate) async fn acquire(&self, key: &str) {
        if let Some(bucket) = self.bucket_for(key) {
            bucket.acquire().await;
        }
    }
}
