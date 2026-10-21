//! Bounded replay-ring clip extraction.

use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::PathBuf;

use crate::error::RecorderError;
use crate::index::SegmentEntry;
use crate::segment::SEGMENT_FOOTER;
use crate::spool::Spool;
use crate::track::{SessionId, TrackId};

/// A clip produced from the replay ring.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClipHandle {
    /// Clip file path inside the session's `clips` directory.
    pub path: PathBuf,
    /// Duration covered by the contributing segments.
    pub duration_ms: u64,
    /// Clip byte length.
    pub bytes: u64,
    /// Number of contributing segments.
    pub segments: usize,
}

/// Concatenates the packet payloads of the selected segments into one clip.
///
/// Segment footers are stripped and one clean footer is written at the end.
/// The operation reads and writes files but does not touch the active segment,
/// so recording can continue immediately afterwards.
pub(crate) fn extract_clip(
    spool: &Spool,
    session: &SessionId,
    track: &TrackId,
    entries: &[SegmentEntry],
    sequence: u64,
) -> Result<ClipHandle, RecorderError> {
    if entries.is_empty() {
        return Err(RecorderError::Clip {
            detail: "no complete segment is available in the ring".to_string(),
        });
    }

    let path = spool.clip_path(session, sequence);
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|error| RecorderError::Clip {
            detail: format!("cannot create {}: {error}", parent.display()),
        })?;
    }
    let temporary = path.with_extension("mkv.tmp");
    let mut output = File::create(&temporary).map_err(|error| RecorderError::Clip {
        detail: format!("cannot create {}: {error}", temporary.display()),
    })?;

    let mut bytes = 0_u64;
    let mut duration_ms = 0_u64;

    for entry in entries {
        let segment_path = spool.segment_path(session, track, entry.sequence, entry.container);
        let mut input = File::open(&segment_path).map_err(|error| RecorderError::Clip {
            detail: format!("cannot read {}: {error}", segment_path.display()),
        })?;
        let mut data = Vec::new();
        input
            .read_to_end(&mut data)
            .map_err(|error| RecorderError::Clip {
                detail: format!("cannot read {}: {error}", segment_path.display()),
            })?;
        if data.ends_with(SEGMENT_FOOTER) {
            data.truncate(data.len() - SEGMENT_FOOTER.len());
        }
        bytes = bytes.saturating_add(data.len() as u64);
        duration_ms = duration_ms.saturating_add(entry.duration_ms);
        output
            .write_all(&data)
            .map_err(|error| RecorderError::Clip {
                detail: format!("cannot write {}: {error}", temporary.display()),
            })?;
    }

    output
        .write_all(SEGMENT_FOOTER)
        .and_then(|()| output.flush())
        .and_then(|()| output.sync_all())
        .map_err(|error| RecorderError::Clip {
            detail: format!("cannot finish {}: {error}", temporary.display()),
        })?;
    drop(output);

    if path.exists() {
        fs::remove_file(&path).map_err(|error| RecorderError::Clip {
            detail: format!("cannot replace {}: {error}", path.display()),
        })?;
    }
    fs::rename(&temporary, &path).map_err(|error| RecorderError::Clip {
        detail: format!("cannot publish {}: {error}", path.display()),
    })?;

    Ok(ClipHandle {
        path,
        duration_ms,
        bytes: bytes.saturating_add(SEGMENT_FOOTER.len() as u64),
        segments: entries.len(),
    })
}
