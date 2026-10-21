//! Segment index and atomic JSON persistence.

use std::fs;
use std::io::Write;
use std::path::Path;

use serde::{Deserialize, Serialize};

use crate::error::RecorderError;
use crate::track::{Container, SessionId, TrackCodec, TrackId};

/// One closed segment recorded in the index.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentEntry {
    /// Monotonic sequence number within the track.
    pub sequence: u64,
    /// Segment file name inside the track directory.
    pub file: String,
    /// First presentation timestamp in microseconds.
    pub start_pts: i64,
    /// Duration in milliseconds.
    pub duration_ms: u64,
    /// Byte length of the local segment.
    pub bytes: u64,
    /// Codec carried by the segment.
    pub codec: TrackCodec,
    /// Container of the segment.
    pub container: Container,
    /// Optional duplicate path written by a tee sink.
    pub duplicate: Option<String>,
    /// Whether the segment has a complete trailer.
    pub complete: bool,
}

impl SegmentEntry {
    /// Returns whether the segment is complete and safe to read.
    #[must_use]
    pub fn is_readable(&self) -> bool {
        self.complete && self.bytes > 0
    }
}

/// The index for one track.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SegmentIndex {
    /// Session identifier.
    pub session: SessionId,
    /// Track identifier.
    pub track: TrackId,
    /// Closed segments in sequence order.
    pub segments: Vec<SegmentEntry>,
}

impl SegmentIndex {
    /// Creates an empty index.
    #[must_use]
    pub fn new(session: SessionId, track: TrackId) -> Self {
        Self {
            session,
            track,
            segments: Vec::new(),
        }
    }

    /// Inserts or replaces a segment by sequence number and keeps order stable.
    pub fn upsert(&mut self, entry: SegmentEntry) {
        match self
            .segments
            .binary_search_by_key(&entry.sequence, |candidate| candidate.sequence)
        {
            Ok(index) => self.segments[index] = entry,
            Err(index) => self.segments.insert(index, entry),
        }
    }

    /// Removes a segment by sequence number.
    pub fn remove(&mut self, sequence: u64) -> Option<SegmentEntry> {
        self.segments
            .binary_search_by_key(&sequence, |candidate| candidate.sequence)
            .ok()
            .map(|index| self.segments.remove(index))
    }

    /// Returns the total byte length of complete segments.
    #[must_use]
    pub fn total_bytes(&self) -> u64 {
        self.segments.iter().map(|segment| segment.bytes).sum()
    }

    /// Returns the oldest complete segment.
    #[must_use]
    pub fn oldest(&self) -> Option<&SegmentEntry> {
        self.segments.iter().find(|segment| segment.is_readable())
    }

    /// Returns the newest complete segment.
    #[must_use]
    pub fn newest(&self) -> Option<&SegmentEntry> {
        self.segments
            .iter()
            .rev()
            .find(|segment| segment.is_readable())
    }
}

/// Loads an index, returning an empty index when the file does not exist.
pub fn load_or_default(
    path: &Path,
    session: SessionId,
    track: TrackId,
) -> Result<SegmentIndex, RecorderError> {
    if !path.exists() {
        return Ok(SegmentIndex::new(session, track));
    }
    let text = fs::read_to_string(path).map_err(|error| RecorderError::Index {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    serde_json::from_str(&text).map_err(|error| RecorderError::Index {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}

/// Writes the index through a temporary file and a rename.
///
/// The temporary file is flushed and synced before the rename so a crash leaves
/// either the old complete index or the new complete index.
pub fn write_atomic(path: &Path, index: &SegmentIndex) -> Result<(), RecorderError> {
    let text = serde_json::to_string_pretty(index).map_err(|error| RecorderError::Index {
        path: path.display().to_string(),
        detail: error.to_string(),
    })?;
    let temporary = path.with_extension("json.tmp");
    {
        let mut file = fs::File::create(&temporary).map_err(|error| RecorderError::Index {
            path: temporary.display().to_string(),
            detail: error.to_string(),
        })?;
        file.write_all(text.as_bytes())
            .and_then(|()| file.sync_all())
            .map_err(|error| RecorderError::Index {
                path: temporary.display().to_string(),
                detail: error.to_string(),
            })?;
    }
    if path.exists() {
        fs::remove_file(path).map_err(|error| RecorderError::Index {
            path: path.display().to_string(),
            detail: error.to_string(),
        })?;
    }
    fs::rename(&temporary, path).map_err(|error| RecorderError::Index {
        path: path.display().to_string(),
        detail: error.to_string(),
    })
}
