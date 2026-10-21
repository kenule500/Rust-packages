//! Typed recorder errors with stable codes.

use std::fmt;

/// A recording, spool, budget, or recovery failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecorderError {
    /// The configuration is invalid.
    InvalidConfig {
        /// Human-readable detail.
        detail: String,
    },
    /// An identifier is empty, malformed, reserved, or path-like.
    InvalidId {
        /// The kind of identifier.
        kind: &'static str,
        /// The rejected value.
        value: String,
    },
    /// No tracks were configured.
    EmptyTracks,
    /// Two tracks share an identifier.
    DuplicateTrack {
        /// The duplicated track.
        track: String,
    },
    /// A packet names an unconfigured track.
    UnknownTrack {
        /// The unknown track.
        track: String,
    },
    /// A packet arrived before the track was started.
    TrackNotStarted {
        /// The track.
        track: String,
    },
    /// A packet arrived after the track was stopped.
    TrackStopped {
        /// The track.
        track: String,
    },
    /// A packet has no payload.
    EmptyPacket {
        /// The track.
        track: String,
    },
    /// A packet timestamp is older than the previous packet on the track.
    NonMonotonicTimestamp {
        /// The track.
        track: String,
        /// Previous decode timestamp.
        previous: i64,
        /// Received decode timestamp.
        received: i64,
    },
    /// The disk budget refused a write.
    BudgetExceeded {
        /// Bytes already used.
        used_bytes: u64,
        /// Configured maximum.
        max_bytes: u64,
    },
    /// The minimum free space would be violated.
    FreeSpaceExceeded {
        /// Current free bytes.
        free_bytes: u64,
        /// Configured minimum.
        min_free_bytes: u64,
    },
    /// A segment file operation failed.
    Segment {
        /// Segment path or logical name.
        path: String,
        /// Human-readable detail.
        detail: String,
    },
    /// An index operation failed.
    Index {
        /// Index path.
        path: String,
        /// Human-readable detail.
        detail: String,
    },
    /// Crash recovery could not complete.
    Recovery {
        /// Path involved.
        path: String,
        /// Human-readable detail.
        detail: String,
    },
    /// A clip request could not be satisfied.
    Clip {
        /// Human-readable detail.
        detail: String,
    },
    /// A filesystem operation failed.
    Io {
        /// Path involved.
        path: String,
        /// Human-readable detail.
        detail: String,
    },
}

impl RecorderError {
    /// Returns the stable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::InvalidConfig { .. } => "REC-001",
            Self::InvalidId { .. } => "REC-002",
            Self::EmptyTracks => "REC-003",
            Self::DuplicateTrack { .. } => "REC-004",
            Self::UnknownTrack { .. } => "REC-005",
            Self::TrackNotStarted { .. } => "REC-006",
            Self::TrackStopped { .. } => "REC-007",
            Self::EmptyPacket { .. } => "REC-008",
            Self::NonMonotonicTimestamp { .. } => "REC-009",
            Self::BudgetExceeded { .. } => "REC-010",
            Self::FreeSpaceExceeded { .. } => "REC-011",
            Self::Segment { .. } => "REC-012",
            Self::Index { .. } => "REC-013",
            Self::Recovery { .. } => "REC-014",
            Self::Clip { .. } => "REC-015",
            Self::Io { .. } => "REC-016",
        }
    }
}

impl fmt::Display for RecorderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.code())?;
        match self {
            Self::InvalidConfig { detail } => write!(f, "invalid configuration: {detail}"),
            Self::InvalidId { kind, value } => write!(f, "invalid {kind} {value:?}"),
            Self::EmptyTracks => f.write_str("at least one track is required"),
            Self::DuplicateTrack { track } => write!(f, "duplicate track {track:?}"),
            Self::UnknownTrack { track } => write!(f, "unknown track {track:?}"),
            Self::TrackNotStarted { track } => write!(f, "track {track:?} is not started"),
            Self::TrackStopped { track } => write!(f, "track {track:?} is stopped"),
            Self::EmptyPacket { track } => write!(f, "packet for {track:?} is empty"),
            Self::NonMonotonicTimestamp {
                track,
                previous,
                received,
            } => write!(
                f,
                "track {track:?} received dts {received} after {previous}"
            ),
            Self::BudgetExceeded {
                used_bytes,
                max_bytes,
            } => write!(f, "budget exceeded: {used_bytes} of {max_bytes} bytes"),
            Self::FreeSpaceExceeded {
                free_bytes,
                min_free_bytes,
            } => write!(f, "free space {free_bytes} below minimum {min_free_bytes}"),
            Self::Segment { path, detail } => write!(f, "segment {path}: {detail}"),
            Self::Index { path, detail } => write!(f, "index {path}: {detail}"),
            Self::Recovery { path, detail } => write!(f, "recovery {path}: {detail}"),
            Self::Clip { detail } => write!(f, "clip: {detail}"),
            Self::Io { path, detail } => write!(f, "io {path}: {detail}"),
        }
    }
}

impl std::error::Error for RecorderError {}
