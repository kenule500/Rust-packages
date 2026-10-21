//! Spool layout and validated path derivation.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::track::{Container, SessionId, TrackId};

/// The on-disk spool layout.
///
/// ```text
/// <root>/<session-id>/track-<id>/seg-<n>.mkv
/// <root>/<session-id>/track-<id>/index.json
/// <root>/<session-id>/clips/clip-<n>.mkv
/// ```
///
/// Every path is derived from validated identifiers; user input never becomes a
/// path segment directly.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Spool {
    root: PathBuf,
}

impl Spool {
    /// Creates a spool rooted at the given directory.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// Returns the spool root.
    #[must_use]
    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Returns the session directory.
    #[must_use]
    pub fn session_dir(&self, session: &SessionId) -> PathBuf {
        self.root.join(session.as_str())
    }

    /// Returns the track directory.
    #[must_use]
    pub fn track_dir(&self, session: &SessionId, track: &TrackId) -> PathBuf {
        self.session_dir(session)
            .join(format!("track-{}", track.as_str()))
    }

    /// Returns a segment path.
    #[must_use]
    pub fn segment_path(
        &self,
        session: &SessionId,
        track: &TrackId,
        sequence: u64,
        container: Container,
    ) -> PathBuf {
        self.track_dir(session, track)
            .join(format!("seg-{sequence}.{}", container.extension()))
    }

    /// Returns the index path for a track.
    #[must_use]
    pub fn index_path(&self, session: &SessionId, track: &TrackId) -> PathBuf {
        self.track_dir(session, track).join("index.json")
    }

    /// Returns a clip path.
    #[must_use]
    pub fn clip_path(&self, session: &SessionId, sequence: u64) -> PathBuf {
        self.session_dir(session)
            .join("clips")
            .join(format!("clip-{sequence}.mkv"))
    }

    /// Creates the session and clip directories.
    pub fn ensure_session(&self, session: &SessionId) -> io::Result<()> {
        fs::create_dir_all(self.session_dir(session).join("clips"))
    }

    /// Creates a track directory.
    pub fn ensure_track(&self, session: &SessionId, track: &TrackId) -> io::Result<()> {
        fs::create_dir_all(self.track_dir(session, track))
    }

    /// Scans a session directory and returns each track's segment files in
    /// sequence order.
    pub fn scan(&self, session: &SessionId) -> io::Result<Vec<(TrackId, Vec<PathBuf>)>> {
        let session_dir = self.session_dir(session);
        if !session_dir.exists() {
            return Ok(Vec::new());
        }
        let mut tracks = Vec::new();
        for entry in fs::read_dir(&session_dir)? {
            let entry = entry?;
            if !entry.file_type()?.is_dir() {
                continue;
            }
            let name = entry.file_name();
            let Some(name) = name.to_str() else {
                continue;
            };
            let Some(raw_id) = name.strip_prefix("track-") else {
                continue;
            };
            let Ok(track) = TrackId::new(raw_id) else {
                continue;
            };
            let mut segments = Vec::new();
            for file in fs::read_dir(entry.path())? {
                let file = file?;
                if !file.file_type()?.is_file() {
                    continue;
                }
                let file_name = file.file_name();
                let Some(file_name) = file_name.to_str() else {
                    continue;
                };
                if file_name.starts_with("seg-") {
                    segments.push(file.path());
                }
            }
            segments.sort_by_key(|left| sequence_of(left));
            tracks.push((track, segments));
        }
        tracks.sort_by(|left, right| left.0.cmp(&right.0));
        Ok(tracks)
    }
}

impl AsRef<Path> for Spool {
    fn as_ref(&self) -> &Path {
        &self.root
    }
}

/// Extracts `N` from `seg-N.ext`; malformed names sort first.
#[must_use]
pub fn sequence_of(path: &Path) -> u64 {
    path.file_name()
        .and_then(|name| name.to_str())
        .and_then(|name| name.strip_prefix("seg-"))
        .and_then(|name| name.split('.').next())
        .and_then(|number| number.parse().ok())
        .unwrap_or(0)
}

/// Returns the container implied by a segment extension.
#[must_use]
pub fn container_of(path: &Path) -> Option<Container> {
    match path.extension().and_then(|extension| extension.to_str()) {
        Some("mkv") => Some(Container::Mkv),
        Some("mp4") => Some(Container::Mp4),
        _ => None,
    }
}
