//! Segment open, write, rotate, and close.

use std::fs::{self, File, OpenOptions};
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use crate::error::RecorderError;
use crate::index::SegmentEntry;
use crate::spool::Spool;
use crate::track::{Container, SessionId, TrackCodec, TrackId};

/// Footer magic written when a segment closes cleanly.
pub const SEGMENT_FOOTER: &[u8; 8] = b"M4SEGFTR";

/// Writes one segment of opaque packets.
#[derive(Debug)]
pub(crate) struct SegmentWriter {
    file: File,
    path: PathBuf,
    file_name: String,
    sequence: u64,
    start_pts: i64,
    last_dts: i64,
    bytes: u64,
    max_duration_us: u64,
    max_bytes: u64,
    codec: TrackCodec,
    container: Container,
}

/// Per-segment writer parameters.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SegmentSpec {
    pub(crate) sequence: u64,
    pub(crate) codec: TrackCodec,
    pub(crate) container: Container,
    pub(crate) max_duration_ms: u64,
    pub(crate) max_bytes: u64,
}

impl SegmentWriter {
    pub(crate) fn open(
        spool: &Spool,
        session: &SessionId,
        track: &TrackId,
        spec: SegmentSpec,
    ) -> Result<Self, RecorderError> {
        let SegmentSpec {
            sequence,
            codec,
            container,
            max_duration_ms,
            max_bytes,
        } = spec;
        let path = spool.segment_path(session, track, sequence, container);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|error| RecorderError::Io {
                path: parent.display().to_string(),
                detail: error.to_string(),
            })?;
        }
        let file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(&path)
            .map_err(|error| RecorderError::Segment {
                path: path.display().to_string(),
                detail: error.to_string(),
            })?;
        let file_name = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("segment")
            .to_string();
        Ok(Self {
            file,
            path,
            file_name,
            sequence,
            start_pts: 0,
            last_dts: i64::MIN,
            bytes: 0,
            max_duration_us: max_duration_ms.saturating_mul(1000),
            max_bytes,
            codec,
            container,
        })
    }

    pub(crate) fn bytes(&self) -> u64 {
        self.bytes
    }

    pub(crate) fn should_rotate(&self, next_dts: i64) -> bool {
        if self.bytes == 0 {
            return false;
        }
        self.bytes >= self.max_bytes
            || next_dts.saturating_sub(self.start_pts) >= self.max_duration_us as i64
    }

    pub(crate) fn write(
        &mut self,
        packet_dts: i64,
        packet_pts: i64,
        data: &[u8],
    ) -> Result<(), RecorderError> {
        if self.bytes == 0 {
            self.start_pts = packet_pts;
            self.last_dts = packet_dts;
        }
        self.file
            .write_all(data)
            .map_err(|error| RecorderError::Segment {
                path: self.path.display().to_string(),
                detail: error.to_string(),
            })?;
        self.bytes += data.len() as u64;
        self.last_dts = packet_dts;
        Ok(())
    }

    pub(crate) fn close(mut self) -> Result<SegmentEntry, RecorderError> {
        self.file
            .write_all(SEGMENT_FOOTER)
            .and_then(|()| self.file.flush())
            .and_then(|()| self.file.sync_all())
            .map_err(|error| RecorderError::Segment {
                path: self.path.display().to_string(),
                detail: error.to_string(),
            })?;
        let duration_us = self.last_dts.saturating_sub(self.start_pts).max(0);
        Ok(SegmentEntry {
            sequence: self.sequence,
            file: self.file_name,
            start_pts: self.start_pts,
            duration_ms: (duration_us / 1000) as u64,
            bytes: self.bytes + SEGMENT_FOOTER.len() as u64,
            codec: self.codec,
            container: self.container,
            duplicate: None,
            complete: true,
        })
    }
}

/// Returns whether a segment file ends with the clean-close footer.
pub(crate) fn is_complete(path: &Path) -> bool {
    let Ok(metadata) = fs::metadata(path) else {
        return false;
    };
    if metadata.len() < SEGMENT_FOOTER.len() as u64 {
        return false;
    }
    let Ok(mut file) = File::open(path) else {
        return false;
    };
    if file
        .seek(SeekFrom::End(-(SEGMENT_FOOTER.len() as i64)))
        .is_err()
    {
        return false;
    }
    let mut footer = [0_u8; SEGMENT_FOOTER.len()];
    file.read_exact(&mut footer).is_ok() && &footer == SEGMENT_FOOTER
}
