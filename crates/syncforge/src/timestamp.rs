use chrono::{DateTime, Duration, Utc};

use crate::Resolution;

/// The audit verdict for a client timestamp.
///
/// A device clock that disagrees with the server beyond the configured
/// tolerance is never silently trusted: it is either clamped to the server time
/// (the client ran ahead) or rejected as an audit time (the client ran behind).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimestampVerdict {
    /// The client and server clocks agree within the tolerance.
    Accepted,
    /// The client clock is ahead of the server by more than the tolerance. The
    /// recorded time is clamped to the server timestamp.
    Clamped {
        /// The timestamp the caller should record instead.
        timestamp: DateTime<Utc>,
        /// The signed server-minus-client skew.
        skew: Duration,
    },
    /// The client clock is behind the server by more than the tolerance. The
    /// client timestamp is not usable as an audit time.
    Rejected {
        /// The signed server-minus-client skew.
        skew: Duration,
    },
}

/// The audit decision for a client and server timestamp pair.
///
/// The client timestamp is never rewritten. `skew` is the server clock minus
/// the client clock; a positive value means the server is later.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TimestampDecision {
    /// The timestamp supplied by the client.
    pub client_timestamp: DateTime<Utc>,
    /// The timestamp recorded by the server or engine.
    pub server_timestamp: DateTime<Utc>,
    /// The configured tolerance.
    pub tolerance: Duration,
    /// The signed difference between the server and client clocks.
    pub skew: Duration,
    /// Whether the two clocks are within the configured tolerance.
    pub within_tolerance: bool,
    /// The typed accept, clamp, or reject outcome.
    pub verdict: TimestampVerdict,
}

impl TimestampDecision {
    /// Returns the later side, preferring the server on an exact tie.
    pub fn later(&self) -> Resolution {
        if self.client_timestamp > self.server_timestamp {
            Resolution::Client
        } else {
            Resolution::Server
        }
    }

    /// Returns the later of the two timestamps, preferring the server on a tie.
    pub fn preferred_timestamp(&self) -> DateTime<Utc> {
        self.client_timestamp.max(self.server_timestamp)
    }

    /// Returns the client timestamp to record: the client's own reading when the
    /// verdict accepted it, the clamped server time when it ran ahead, and the
    /// server timestamp when the client time was rejected.
    pub fn effective_client_timestamp(&self) -> DateTime<Utc> {
        match self.verdict {
            TimestampVerdict::Accepted => self.client_timestamp,
            TimestampVerdict::Clamped { timestamp, .. } => timestamp,
            TimestampVerdict::Rejected { .. } => self.server_timestamp,
        }
    }
}

/// Compares a client timestamp with a server timestamp and returns the typed
/// verdict, the signed skew, and both original timestamps.
pub fn decide(
    client_timestamp: DateTime<Utc>,
    server_timestamp: DateTime<Utc>,
    tolerance: Duration,
) -> TimestampDecision {
    let skew_millis = server_timestamp
        .timestamp_millis()
        .saturating_sub(client_timestamp.timestamp_millis());
    let tolerance_millis = u64::try_from(tolerance.num_milliseconds().max(0)).unwrap_or(u64::MAX);
    let within_tolerance = skew_millis.unsigned_abs() <= tolerance_millis;
    let skew = Duration::milliseconds(skew_millis);

    let verdict = if within_tolerance {
        TimestampVerdict::Accepted
    } else if skew_millis < 0 {
        // The client clock is ahead of the server: clamp to the server time.
        TimestampVerdict::Clamped {
            timestamp: server_timestamp,
            skew,
        }
    } else {
        // The client clock is behind the server: the client time is not a
        // trustworthy audit time.
        TimestampVerdict::Rejected { skew }
    };

    TimestampDecision {
        client_timestamp,
        server_timestamp,
        tolerance,
        skew,
        within_tolerance,
        verdict,
    }
}
