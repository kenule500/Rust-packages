//! Sink configuration.
//!
//! A sink decides how closed segments are kept. The writer itself always writes
//! to the spool; the sink controls duplication, retention, and clip access.

use std::path::PathBuf;

use crate::index::SegmentEntry;
use crate::track::TrackId;

/// A segment sink: keep every closed segment in the spool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SegmentSink {
    /// Spool root. It must match the recorder's spool for paths to resolve.
    pub root: PathBuf,
}

/// A tee sink: keep every segment in the spool and copy it to a second root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TeeSink {
    /// Duplicate root. The relative session and track layout is preserved.
    pub root: PathBuf,
}

/// One retained ring segment.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct RingEntry {
    /// Track that owns the segment.
    pub track: TrackId,
    /// Closed segment metadata.
    pub entry: SegmentEntry,
}

/// A bounded replay ring.
///
/// The ring retains the newest segments up to both a duration and a byte limit.
/// Eviction is explicit so the recorder can delete files and keep the index in
/// sync.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RingSink {
    /// Retained recording duration in seconds.
    pub window_seconds: u64,
    /// Retained byte cap.
    pub window_bytes: u64,
    pub(crate) entries: Vec<RingEntry>,
}

impl RingSink {
    /// Creates a ring. Both limits must be non-zero.
    #[must_use]
    pub fn new(window_seconds: u64, window_bytes: u64) -> Self {
        Self {
            window_seconds,
            window_bytes,
            entries: Vec::new(),
        }
    }

    /// Pushes a closed segment into the ring.
    pub(crate) fn push(&mut self, track: TrackId, entry: SegmentEntry) {
        self.entries.push(RingEntry { track, entry });
    }

    /// Removes and returns the oldest entries that exceed either limit.
    pub(crate) fn evict(&mut self) -> Vec<RingEntry> {
        let mut evicted = Vec::new();
        while self.over_limit() {
            if self.entries.is_empty() {
                break;
            }
            evicted.push(self.entries.remove(0));
        }
        evicted
    }

    fn over_limit(&self) -> bool {
        let bytes: u64 = self.entries.iter().map(|item| item.entry.bytes).sum();
        let duration_ms: u64 = self.entries.iter().map(|item| item.entry.duration_ms).sum();
        bytes > self.window_bytes || duration_ms > self.window_seconds.saturating_mul(1000)
    }

    /// Returns the number of retained segments.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns whether the ring is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Returns the retained byte total.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.entries.iter().map(|item| item.entry.bytes).sum()
    }

    /// Selects the newest entries for a track covering at least `last_ms`,
    /// returned in recording order.
    #[must_use]
    pub fn clip_entries(&self, track: &TrackId, last_ms: u64) -> Vec<SegmentEntry> {
        let mut selected = Vec::new();
        let mut covered = 0_u64;
        for item in self.entries.iter().rev() {
            if &item.track != track {
                continue;
            }
            selected.push(item.entry.clone());
            covered = covered.saturating_add(item.entry.duration_ms);
            if covered >= last_ms {
                break;
            }
        }
        selected.reverse();
        selected
    }
}

/// How closed segments are managed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sink {
    /// Keep every segment.
    Segment(SegmentSink),
    /// Keep every segment and copy it to a duplicate root.
    Tee(TeeSink),
    /// Keep a bounded replay window.
    Ring(RingSink),
}

impl Sink {
    /// Returns a segment sink rooted at the given path.
    #[must_use]
    pub fn segment(root: impl Into<PathBuf>) -> Self {
        Self::Segment(SegmentSink { root: root.into() })
    }

    /// Returns a tee sink rooted at the given duplicate path.
    #[must_use]
    pub fn tee(root: impl Into<PathBuf>) -> Self {
        Self::Tee(TeeSink { root: root.into() })
    }

    /// Returns whether this is a ring sink.
    #[must_use]
    pub fn is_ring(&self) -> bool {
        matches!(self, Self::Ring(_))
    }

    /// Returns the ring, if this is a ring sink.
    #[must_use]
    pub fn as_ring(&self) -> Option<&RingSink> {
        match self {
            Self::Ring(ring) => Some(ring),
            _ => None,
        }
    }

    /// Returns the mutable ring, if this is a ring sink.
    pub fn as_ring_mut(&mut self) -> Option<&mut RingSink> {
        match self {
            Self::Ring(ring) => Some(ring),
            _ => None,
        }
    }
}
