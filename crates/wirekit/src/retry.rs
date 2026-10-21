//! Retry policy: exponential backoff with jitter, `Retry-After`, and
//! classification.
//!
//! Scope: one policy type shared by every provider (CON-208, NFR-018), the
//! delay computation, and the parsing of `Retry-After` in both its
//! numeric-seconds and HTTP-date forms.
//!
//! Non-goals: a per-provider retry loop (forbidden by NFR-018), blocking
//! sleeps (NFR-005), and retrying any 4xx other than 429.

use std::time::{Duration, SystemTime, UNIX_EPOCH};

use rand::Rng;

use crate::error::WirekitError;

/// Bounds for the multiplicative jitter applied to a backoff delay.
///
/// A factor of `1.0` means "no jitter"; `0.5..=1.5` spreads retries so that
/// many clients do not retry in lockstep.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct JitterBounds {
    /// Smallest multiplier applied to the computed delay.
    pub min: f64,
    /// Largest multiplier applied to the computed delay.
    pub max: f64,
}

impl JitterBounds {
    /// Build bounds, rejecting a negative, non-finite, or inverted range.
    pub fn new(min: f64, max: f64) -> Result<Self, WirekitError> {
        if !min.is_finite() || !max.is_finite() || min < 0.0 || max < min {
            return Err(WirekitError::invalid_config(
                "jitter bounds must be finite and satisfy 0.0 <= min <= max",
            ));
        }
        Ok(Self { min, max })
    }
}

impl Default for JitterBounds {
    fn default() -> Self {
        Self { min: 0.5, max: 1.5 }
    }
}

/// The retry policy shared by every provider.
///
/// Defaults follow CON-208: three attempts in total, a 200 ms base delay, a
/// 10 s ceiling, and jitter between 0.5x and 1.5x.
///
/// ```
/// # use wirekit::RetryPolicy;
/// let policy = RetryPolicy::default();
/// assert_eq!(policy.attempts, 3);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct RetryPolicy {
    /// Total attempts, including the first one. Must be at least 1.
    pub attempts: u32,
    /// Delay before the second attempt.
    pub base_delay: Duration,
    /// Ceiling for any single delay.
    pub max_delay: Duration,
    /// Multiplicative jitter bounds.
    pub jitter: JitterBounds,
}

impl Default for RetryPolicy {
    fn default() -> Self {
        Self {
            attempts: 3,
            base_delay: Duration::from_millis(200),
            max_delay: Duration::from_secs(10),
            jitter: JitterBounds::default(),
        }
    }
}

impl RetryPolicy {
    /// The policy that never retries.
    #[must_use]
    pub fn none() -> Self {
        Self {
            attempts: 1,
            ..Self::default()
        }
    }

    /// Reject a policy that cannot be executed.
    pub fn validate(&self) -> Result<(), WirekitError> {
        if self.attempts == 0 {
            return Err(WirekitError::invalid_config(
                "retry attempts must be at least 1",
            ));
        }
        if self.max_delay < self.base_delay {
            return Err(WirekitError::invalid_config(
                "retry max_delay must be greater than or equal to base_delay",
            ));
        }
        if !self.jitter.min.is_finite()
            || !self.jitter.max.is_finite()
            || self.jitter.min < 0.0
            || self.jitter.max < self.jitter.min
        {
            return Err(WirekitError::invalid_config(
                "retry jitter bounds must be finite and satisfy 0.0 <= min <= max",
            ));
        }
        Ok(())
    }

    /// The delay before the attempt following `attempt` (1-based).
    ///
    /// The delay grows as `base_delay * 2^(attempt - 1)`, is capped at
    /// `max_delay`, and is then multiplied by a jitter factor in
    /// `[jitter.min, jitter.max]`. The result is always within
    /// `[0, max_delay]`.
    #[must_use]
    pub fn delay_for(&self, attempt: u32) -> Duration {
        let mut rng = rand::thread_rng();
        self.delay_for_with(attempt, &mut rng)
    }

    pub(crate) fn delay_for_with<R: Rng + ?Sized>(&self, attempt: u32, rng: &mut R) -> Duration {
        let exponent = attempt.saturating_sub(1).min(31);
        let growth = 2_f64.powi(exponent as i32);
        let base = self.base_delay.as_secs_f64() * growth;
        let capped = base.min(self.max_delay.as_secs_f64());
        let factor = if self.jitter.max > self.jitter.min {
            rng.gen_range(self.jitter.min..=self.jitter.max)
        } else {
            self.jitter.min
        };
        let jittered = (capped * factor).clamp(0.0, self.max_delay.as_secs_f64());
        Duration::from_secs_f64(jittered)
    }

    /// The delay to wait after `attempt`, honouring a provider-supplied
    /// `Retry-After` value when it is longer than the computed backoff.
    #[must_use]
    pub fn delay_before_retry(&self, attempt: u32, retry_after: Option<Duration>) -> Duration {
        let backoff = self.delay_for(attempt);
        match retry_after {
            Some(wait) if wait > backoff => wait.min(self.max_delay),
            _ => backoff,
        }
    }
}

/// Parse a `Retry-After` header value.
///
/// Accepts delay-seconds and the IMF-fixdate form of an HTTP date. An
/// unparseable value returns `None` so the caller falls back to jittered
/// backoff instead of panicking (EC-007). A date in the past yields a zero
/// wait.
#[must_use]
pub fn parse_retry_after(value: &str) -> Option<Duration> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return None;
    }

    if let Ok(seconds) = trimmed.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }

    parse_imf_fixdate(trimmed).map(|target| {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs());
        Duration::from_secs(target.saturating_sub(now))
    })
}

/// Parse `Sun, 06 Nov 1994 08:49:37 GMT` into a Unix timestamp.
fn parse_imf_fixdate(value: &str) -> Option<u64> {
    let mut parts = value.split_whitespace();
    let _weekday = parts.next()?;
    let day: u32 = parts.next()?.parse().ok()?;
    let month = month_from_name(parts.next()?)?;
    let year: i64 = parts.next()?.parse().ok()?;
    let time = parts.next()?;
    let zone = parts.next()?;

    if !zone.eq_ignore_ascii_case("GMT") && !zone.eq_ignore_ascii_case("UTC") {
        return None;
    }
    if !(1..=31).contains(&day) {
        return None;
    }

    let mut clock = time.split(':');
    let hour: u64 = clock.next()?.parse().ok()?;
    let minute: u64 = clock.next()?.parse().ok()?;
    let second: u64 = clock.next()?.parse().ok()?;
    if hour > 23 || minute > 59 || second > 60 {
        return None;
    }

    let days = days_from_civil(year, month, day);
    if days < 0 {
        return None;
    }
    let seconds = days as u64 * 86_400 + hour * 3_600 + minute * 60 + second;
    Some(seconds)
}

fn month_from_name(name: &str) -> Option<u32> {
    const MONTHS: [&str; 12] = [
        "Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec",
    ];
    MONTHS
        .iter()
        .position(|candidate| candidate.eq_ignore_ascii_case(name))
        .map(|index| index as u32 + 1)
}

/// Days since 1970-01-01 for a proleptic Gregorian date.
fn days_from_civil(year: i64, month: u32, day: u32) -> i64 {
    let year = if month <= 2 { year - 1 } else { year };
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let year_of_era = year - era * 400;
    let month_prime = (i64::from(month) + 9) % 12;
    let day_of_year = (153 * month_prime + 2) / 5 + i64::from(day) - 1;
    let day_of_era = year_of_era * 365 + year_of_era / 4 - year_of_era / 100 + day_of_year;
    era * 146_097 + day_of_era - 719_468
}

/// Whether the retry loop may attempt `error` again.
///
/// A provider may declare body-level error codes that must never be retried,
/// such as Slack's `channel_not_found` and `not_in_channel` (FR-043).
pub(crate) fn should_retry(error: &WirekitError, non_retryable: &[String]) -> bool {
    if let Some(code) = error.api_code() {
        if non_retryable.iter().any(|declared| declared == code) {
            return false;
        }
    }
    error.is_retryable()
}
