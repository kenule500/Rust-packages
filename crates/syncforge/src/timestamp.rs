use chrono::{DateTime, Duration, Utc};

use crate::Resolution;

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
}

/// Compares a client timestamp with a server timestamp.
pub fn decide(
    client_timestamp: DateTime<Utc>,
    server_timestamp: DateTime<Utc>,
    tolerance: Duration,
) -> TimestampDecision {
    let skew_millis = server_timestamp
        .timestamp_millis()
        .saturating_sub(client_timestamp.timestamp_millis());
    let tolerance_millis = u64::try_from(tolerance.num_milliseconds().max(0)).unwrap_or(u64::MAX);

    TimestampDecision {
        client_timestamp,
        server_timestamp,
        tolerance,
        skew: Duration::milliseconds(skew_millis),
        within_tolerance: skew_millis.unsigned_abs() <= tolerance_millis,
    }
}
