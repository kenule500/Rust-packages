//! Track, packet, and identifier types.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::error::RecorderError;

/// A validated recording-session identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SessionId(String);

impl SessionId {
    /// Validates and creates a session identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RecorderError> {
        validate_id(value.into(), "session id").map(Self)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SessionId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated track identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrackId(String);

impl TrackId {
    /// Validates and creates a track identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RecorderError> {
        validate_id(value.into(), "track id").map(Self)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for TrackId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated source identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SourceId(String);

impl SourceId {
    /// Validates and creates a source identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, RecorderError> {
        validate_id(value.into(), "source id").map(Self)
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SourceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_id(value: String, kind: &'static str) -> Result<String, RecorderError> {
    if value.is_empty()
        || value.len() > 64
        || !value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        || !value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
    {
        return Err(RecorderError::InvalidId { kind, value });
    }
    let upper = value.to_ascii_uppercase();
    let reserved = matches!(
        upper.as_str(),
        "CON"
            | "PRN"
            | "AUX"
            | "NUL"
            | "COM1"
            | "COM2"
            | "COM3"
            | "COM4"
            | "COM5"
            | "COM6"
            | "COM7"
            | "COM8"
            | "COM9"
            | "LPT1"
            | "LPT2"
            | "LPT3"
            | "LPT4"
            | "LPT5"
            | "LPT6"
            | "LPT7"
            | "LPT8"
            | "LPT9"
    );
    if reserved {
        return Err(RecorderError::InvalidId { kind, value });
    }
    Ok(value)
}

/// The codec carried by a track's opaque packets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TrackCodec {
    /// H.264 / AVC.
    H264,
    /// H.265 / HEVC.
    Hevc,
    /// AV1.
    Av1,
    /// AAC audio.
    Aac,
    /// Opus audio.
    Opus,
}

impl TrackCodec {
    /// Returns the stable codec name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::H264 => "h264",
            Self::Hevc => "hevc",
            Self::Av1 => "av1",
            Self::Aac => "aac",
            Self::Opus => "opus",
        }
    }
}

/// The segment container.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Container {
    /// Matroska; the default.
    Mkv,
    /// MP4.
    Mp4,
}

impl Container {
    /// Returns the file extension without a dot.
    #[must_use]
    pub const fn extension(self) -> &'static str {
        match self {
            Self::Mkv => "mkv",
            Self::Mp4 => "mp4",
        }
    }
}

/// One isolated track.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Track {
    /// Track identifier.
    pub id: TrackId,
    /// Source identifier.
    pub source: SourceId,
    /// Codec of the packets written to the track.
    pub codec: TrackCodec,
    /// Segment container.
    pub container: Container,
    /// Whether the track is enabled.
    pub enabled: bool,
}

impl Track {
    /// Creates an enabled MKV track.
    pub fn new(id: TrackId, source: SourceId, codec: TrackCodec) -> Self {
        Self {
            id,
            source,
            codec,
            container: Container::Mkv,
            enabled: true,
        }
    }
}

/// One already-encoded media packet.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Packet {
    /// Destination track.
    pub track: TrackId,
    /// Decode timestamp in microseconds.
    pub dts: i64,
    /// Presentation timestamp in microseconds.
    pub pts: i64,
    /// Duration in microseconds.
    pub duration: u32,
    /// Whether the packet is a keyframe.
    pub keyframe: bool,
    /// Opaque encoded payload.
    pub data: Vec<u8>,
}

impl Packet {
    /// Creates a packet.
    #[must_use]
    pub fn new(
        track: TrackId,
        dts: i64,
        pts: i64,
        duration: u32,
        keyframe: bool,
        data: Vec<u8>,
    ) -> Self {
        Self {
            track,
            dts,
            pts,
            duration,
            keyframe,
            data,
        }
    }
}
