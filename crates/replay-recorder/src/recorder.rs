//! The recorder: track lifecycle, segment rotation, budget, recovery, and clips.

use std::collections::BTreeMap;
use std::fs;
use std::path::PathBuf;

use crate::budget::{BudgetDecision, DiskBudget, PressureAction, PressureEvent, PressurePolicy};
use crate::error::RecorderError;
use crate::index::{self, SegmentEntry, SegmentIndex};
use crate::ring::{extract_clip, ClipHandle};
use crate::segment::{is_complete, SegmentWriter};
use crate::sink::Sink;
use crate::spool::{self, Spool};
use crate::track::{Packet, SessionId, Track, TrackId};

/// A summary of crash recovery work performed while opening the recorder.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct RecoveryReport {
    /// Segments with a clean footer that were added to the index.
    pub finalized: u64,
    /// Segments without a clean footer that were removed.
    pub discarded: u64,
    /// Tracks whose index needed a repair.
    pub repaired_indexes: u64,
}

/// Recorder construction parameters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecorderConfig {
    /// Spool root directory.
    pub spool_root: PathBuf,
    /// Session identifier.
    pub session: SessionId,
    /// Isolated tracks.
    pub tracks: Vec<Track>,
    /// Segment retention policy.
    pub sink: Sink,
    /// Disk budget and pressure policy.
    pub budget: DiskBudget,
    /// Maximum segment duration in milliseconds.
    pub segment_duration_ms: u64,
    /// Maximum segment size in bytes.
    pub segment_max_bytes: u64,
    /// Free bytes supplied by the caller's platform layer.
    pub free_bytes: u64,
}

impl RecorderConfig {
    /// Creates a configuration with a segment sink and conservative defaults:
    /// 6 s or 256 MiB per segment, 10 GiB cap, 1 GiB free-space floor, and
    /// stop-oldest pressure.
    pub fn new(spool_root: impl Into<PathBuf>, session: SessionId, tracks: Vec<Track>) -> Self {
        let spool_root = spool_root.into();
        Self {
            sink: Sink::segment(spool_root.clone()),
            spool_root,
            session,
            tracks,
            budget: DiskBudget::new(
                10 * 1024 * 1024 * 1024,
                1024 * 1024 * 1024,
                PressurePolicy::StopOldest,
            ),
            segment_duration_ms: 6_000,
            segment_max_bytes: 256 * 1024 * 1024,
            free_bytes: u64::MAX,
        }
    }

    /// Validates the configuration.
    pub fn validate(&self) -> Result<(), RecorderError> {
        if self.tracks.is_empty() {
            return Err(RecorderError::EmptyTracks);
        }
        if !self.tracks.iter().any(|track| track.enabled) {
            return Err(RecorderError::InvalidConfig {
                detail: "at least one track must be enabled".to_string(),
            });
        }
        if self.segment_duration_ms == 0 || self.segment_max_bytes == 0 {
            return Err(RecorderError::InvalidConfig {
                detail: "segment duration and size must be non-zero".to_string(),
            });
        }
        self.budget.validate()?;
        for (index, track) in self.tracks.iter().enumerate() {
            if self.tracks[..index]
                .iter()
                .any(|candidate| candidate.id == track.id)
            {
                return Err(RecorderError::DuplicateTrack {
                    track: track.id.to_string(),
                });
            }
        }
        match &self.sink {
            Sink::Ring(ring) if ring.window_seconds == 0 || ring.window_bytes == 0 => {
                Err(RecorderError::InvalidConfig {
                    detail: "ring window seconds and bytes must be non-zero".to_string(),
                })
            }
            Sink::Tee(tee) if tee.root.as_os_str().is_empty() => {
                Err(RecorderError::InvalidConfig {
                    detail: "tee root must not be empty".to_string(),
                })
            }
            _ => Ok(()),
        }
    }
}

#[derive(Debug)]
struct ActiveTrack {
    writer: SegmentWriter,
    last_dts: i64,
    packets: u64,
}

/// A packet-in recorder with isolated tracks.
#[derive(Debug)]
pub struct Recorder {
    spool: Spool,
    session: SessionId,
    tracks: Vec<Track>,
    sink: Sink,
    budget: DiskBudget,
    segment_duration_ms: u64,
    segment_max_bytes: u64,
    free_bytes: u64,
    active: BTreeMap<TrackId, ActiveTrack>,
    indexes: BTreeMap<TrackId, SegmentIndex>,
    events: Vec<PressureEvent>,
    recovery: RecoveryReport,
    clip_sequence: u64,
    used_bytes: u64,
}

impl Recorder {
    /// Opens a recorder, creates the spool directories, and repairs the index.
    pub fn open(config: RecorderConfig) -> Result<Self, RecorderError> {
        config.validate()?;
        let spool = Spool::new(config.spool_root);
        spool
            .ensure_session(&config.session)
            .map_err(|error| RecorderError::Io {
                path: spool.session_dir(&config.session).display().to_string(),
                detail: error.to_string(),
            })?;

        let mut indexes = BTreeMap::new();
        let mut recovery = RecoveryReport::default();
        for track in &config.tracks {
            spool
                .ensure_track(&config.session, &track.id)
                .map_err(|error| RecorderError::Io {
                    path: spool
                        .track_dir(&config.session, &track.id)
                        .display()
                        .to_string(),
                    detail: error.to_string(),
                })?;
            let (index, report) = recover_track(&spool, &config.session, track)?;
            recovery.finalized += report.finalized;
            recovery.discarded += report.discarded;
            recovery.repaired_indexes += report.repaired_indexes;
            indexes.insert(track.id.clone(), index);
        }

        let used_bytes = indexes.values().map(SegmentIndex::total_bytes).sum();
        Ok(Self {
            spool,
            session: config.session,
            tracks: config.tracks,
            sink: config.sink,
            budget: config.budget,
            segment_duration_ms: config.segment_duration_ms,
            segment_max_bytes: config.segment_max_bytes,
            free_bytes: config.free_bytes,
            active: BTreeMap::new(),
            indexes,
            events: Vec::new(),
            recovery,
            clip_sequence: 0,
            used_bytes,
        })
    }

    /// Returns the session identifier.
    #[must_use]
    pub fn session(&self) -> &SessionId {
        &self.session
    }

    /// Returns the spool.
    #[must_use]
    pub fn spool(&self) -> &Spool {
        &self.spool
    }

    /// Returns the recovery report produced when the recorder opened.
    #[must_use]
    pub fn recovery_report(&self) -> RecoveryReport {
        self.recovery
    }

    /// Returns the track index.
    #[must_use]
    pub fn index(&self, track: &TrackId) -> Option<&SegmentIndex> {
        self.indexes.get(track)
    }

    /// Returns the current retained bytes (closed segments plus active data).
    #[must_use]
    pub fn used_bytes(&self) -> u64 {
        self.used_bytes
            + self
                .active
                .values()
                .map(|track| track.writer.bytes())
                .sum::<u64>()
    }

    /// Returns accumulated pressure events.
    #[must_use]
    pub fn events(&self) -> &[PressureEvent] {
        &self.events
    }

    /// Takes accumulated pressure events.
    pub fn drain_events(&mut self) -> Vec<PressureEvent> {
        std::mem::take(&mut self.events)
    }

    /// Updates the free-space value used by the budget policy.
    pub fn set_free_bytes(&mut self, free_bytes: u64) {
        self.free_bytes = free_bytes;
    }

    /// Starts writing a track. Calling it twice is idempotent.
    pub fn start(&mut self, track: &TrackId) -> Result<(), RecorderError> {
        self.track_config(track)?;
        if self.active.contains_key(track) {
            return Ok(());
        }
        if !self.ensure_budget(track, 0)? {
            return Ok(());
        }
        self.open_writer(track)
    }

    /// Stops a track and finalises its active segment.
    pub fn stop(&mut self, track: &TrackId) -> Result<(), RecorderError> {
        if !self.active.contains_key(track) {
            return Err(RecorderError::TrackNotStarted {
                track: track.to_string(),
            });
        }
        self.finish_track(track)
    }

    /// Finalises every active track.
    pub fn close(&mut self) -> Result<(), RecorderError> {
        let active: Vec<TrackId> = self.active.keys().cloned().collect();
        for track in active {
            self.finish_track(&track)?;
        }
        Ok(())
    }

    /// Writes one packet.
    ///
    /// Returns `Some(entry)` when the write caused a segment rotation. Returns
    /// `Ok(None)` when the packet was written or dropped by the stop-newest
    /// pressure policy; pressure events are available through [`Self::events`].
    pub fn write_packet(&mut self, packet: Packet) -> Result<Option<SegmentEntry>, RecorderError> {
        let track = self.track_config(&packet.track)?;
        if !track.enabled {
            return Err(RecorderError::TrackStopped {
                track: packet.track.to_string(),
            });
        }
        if packet.data.is_empty() {
            return Err(RecorderError::EmptyPacket {
                track: packet.track.to_string(),
            });
        }
        let Some(active) = self.active.get(&packet.track) else {
            return Err(RecorderError::TrackNotStarted {
                track: packet.track.to_string(),
            });
        };
        if packet.dts < active.last_dts {
            return Err(RecorderError::NonMonotonicTimestamp {
                track: packet.track.to_string(),
                previous: active.last_dts,
                received: packet.dts,
            });
        }
        let rotate = active.writer.should_rotate(packet.dts);
        let mut rotated = None;
        if rotate {
            rotated = self.rotate(&packet.track)?;
            if !self.active.contains_key(&packet.track) {
                return Ok(rotated);
            }
        }

        if !self.ensure_budget(&packet.track, packet.data.len() as u64)? {
            return Ok(rotated);
        }

        let Some(active) = self.active.get_mut(&packet.track) else {
            return Ok(rotated);
        };
        active.writer.write(packet.dts, packet.pts, &packet.data)?;
        active.last_dts = packet.dts;
        active.packets += 1;
        Ok(rotated)
    }

    /// Extracts the last `last_ms` of a track from the ring.
    ///
    /// The active segment is not modified, so recording may continue after the
    /// call.
    pub fn clip(&mut self, track: &TrackId, last_ms: u64) -> Result<ClipHandle, RecorderError> {
        self.track_config(track)?;
        let Some(ring) = self.sink.as_ring() else {
            return Err(RecorderError::Clip {
                detail: "clip requires the ring sink".to_string(),
            });
        };
        let entries = ring.clip_entries(track, last_ms.max(1));
        if entries.is_empty() {
            return Err(RecorderError::Clip {
                detail: "the ring has no complete segment for this track".to_string(),
            });
        }
        let sequence = self.clip_sequence;
        self.clip_sequence = self.clip_sequence.saturating_add(1);
        extract_clip(&self.spool, &self.session, track, &entries, sequence)
    }

    fn track_config(&self, track: &TrackId) -> Result<&Track, RecorderError> {
        self.tracks
            .iter()
            .find(|candidate| &candidate.id == track)
            .ok_or_else(|| RecorderError::UnknownTrack {
                track: track.to_string(),
            })
    }

    fn open_writer(&mut self, track: &TrackId) -> Result<(), RecorderError> {
        let config = self.track_config(track)?.clone();
        let sequence = self
            .indexes
            .get(track)
            .and_then(|index| index.segments.last())
            .map_or(0, |entry| entry.sequence.saturating_add(1));
        let writer = SegmentWriter::open(
            &self.spool,
            &self.session,
            track,
            crate::segment::SegmentSpec {
                sequence,
                codec: config.codec,
                container: config.container,
                max_duration_ms: self.segment_duration_ms,
                max_bytes: self.segment_max_bytes,
            },
        )?;
        self.active.insert(
            track.clone(),
            ActiveTrack {
                writer,
                last_dts: i64::MIN,
                packets: 0,
            },
        );
        Ok(())
    }

    fn rotate(&mut self, track: &TrackId) -> Result<Option<SegmentEntry>, RecorderError> {
        let Some(active) = self.active.remove(track) else {
            return Ok(None);
        };
        let entry = active.writer.close()?;
        let entry = self.after_close(track, entry)?;
        if self.ensure_budget(track, 0)? {
            self.open_writer(track)?;
        }
        Ok(Some(entry))
    }

    fn finish_track(&mut self, track: &TrackId) -> Result<(), RecorderError> {
        let Some(active) = self.active.remove(track) else {
            return Ok(());
        };
        let entry = active.writer.close()?;
        self.after_close(track, entry)?;
        Ok(())
    }

    fn after_close(
        &mut self,
        track: &TrackId,
        mut entry: SegmentEntry,
    ) -> Result<SegmentEntry, RecorderError> {
        if let Sink::Tee(tee) = &self.sink {
            let target = tee
                .root
                .join(self.session.as_str())
                .join(format!("track-{}", track.as_str()))
                .join(&entry.file);
            if let Some(parent) = target.parent() {
                fs::create_dir_all(parent).map_err(|error| RecorderError::Io {
                    path: parent.display().to_string(),
                    detail: error.to_string(),
                })?;
            }
            let source =
                self.spool
                    .segment_path(&self.session, track, entry.sequence, entry.container);
            fs::copy(&source, &target).map_err(|error| RecorderError::Io {
                path: target.display().to_string(),
                detail: error.to_string(),
            })?;
            entry.duplicate = Some(target.to_string_lossy().into_owned());
        }

        if let Some(index) = self.indexes.get_mut(track) {
            index.upsert(entry.clone());
        }
        self.persist_index(track)?;
        self.used_bytes = self.indexes.values().map(SegmentIndex::total_bytes).sum();

        let evicted = if let Sink::Ring(ring) = &mut self.sink {
            ring.push(track.clone(), entry.clone());
            ring.evict()
        } else {
            Vec::new()
        };
        if !evicted.is_empty() {
            let mut touched: Vec<TrackId> = Vec::new();
            for item in evicted {
                let path = self.spool.segment_path(
                    &self.session,
                    &item.track,
                    item.entry.sequence,
                    item.entry.container,
                );
                let _ = fs::remove_file(&path);
                if let Some(index) = self.indexes.get_mut(&item.track) {
                    index.remove(item.entry.sequence);
                }
                if !touched.contains(&item.track) {
                    touched.push(item.track);
                }
            }
            for track in touched {
                self.persist_index(&track)?;
            }
            self.used_bytes = self.indexes.values().map(SegmentIndex::total_bytes).sum();
        }

        Ok(entry)
    }

    fn persist_index(&self, track: &TrackId) -> Result<(), RecorderError> {
        let path = self.spool.index_path(&self.session, track);
        if let Some(index) = self.indexes.get(track) {
            index::write_atomic(&path, index)?;
        }
        Ok(())
    }

    /// Applies the budget policy before a write.
    ///
    /// Returns `false` when the stop-newest policy stopped the track.
    fn ensure_budget(&mut self, track: &TrackId, additional: u64) -> Result<bool, RecorderError> {
        loop {
            let active_bytes: u64 = self.active.values().map(|item| item.writer.bytes()).sum();
            let used = self.used_bytes + active_bytes;
            match self.budget.evaluate(used, self.free_bytes, additional) {
                BudgetDecision::Allowed => return Ok(true),
                BudgetDecision::Pressure {
                    policy,
                    over_cap,
                    under_floor,
                } => match policy {
                    PressurePolicy::Refuse => {
                        self.emit(track, policy, PressureAction::Refused);
                        return Err(self.budget_error(used, over_cap, under_floor));
                    }
                    PressurePolicy::StopNewest => {
                        self.emit(track, policy, PressureAction::StoppedTrack);
                        self.finish_track(track)?;
                        return Ok(false);
                    }
                    PressurePolicy::StopOldest => {
                        if self.remove_oldest()?.is_some() {
                            self.emit(track, policy, PressureAction::DeletedOldest);
                            continue;
                        }
                        self.emit(track, policy, PressureAction::Refused);
                        return Err(self.budget_error(used, over_cap, under_floor));
                    }
                },
            }
        }
    }

    fn budget_error(&self, used: u64, over_cap: bool, under_floor: bool) -> RecorderError {
        if over_cap {
            RecorderError::BudgetExceeded {
                used_bytes: used,
                max_bytes: self.budget.max_bytes,
            }
        } else if under_floor {
            RecorderError::FreeSpaceExceeded {
                free_bytes: self.free_bytes,
                min_free_bytes: self.budget.min_free_bytes,
            }
        } else {
            RecorderError::BudgetExceeded {
                used_bytes: used,
                max_bytes: self.budget.max_bytes,
            }
        }
    }

    fn emit(&mut self, track: &TrackId, policy: PressurePolicy, action: PressureAction) {
        self.events.push(PressureEvent {
            policy,
            action,
            used_bytes: self.used_bytes,
            max_bytes: self.budget.max_bytes,
            free_bytes: self.free_bytes,
            min_free_bytes: self.budget.min_free_bytes,
            track: Some(track.clone()),
        });
    }

    fn remove_oldest(&mut self) -> Result<Option<(TrackId, u64)>, RecorderError> {
        let candidate = self
            .indexes
            .iter()
            .filter_map(|(track, index)| {
                index.oldest().map(|entry| {
                    (
                        track.clone(),
                        entry.sequence,
                        entry.start_pts,
                        entry.container,
                    )
                })
            })
            .min_by_key(|(track, sequence, start, _)| (*start, *sequence, track.clone()));
        let Some((track, sequence, _, container)) = candidate else {
            return Ok(None);
        };
        let path = self
            .spool
            .segment_path(&self.session, &track, sequence, container);
        let _ = fs::remove_file(&path);
        if let Some(index) = self.indexes.get_mut(&track) {
            index.remove(sequence);
        }
        self.persist_index(&track)?;
        self.used_bytes = self.indexes.values().map(SegmentIndex::total_bytes).sum();
        Ok(Some((track, sequence)))
    }
}

fn recover_track(
    spool: &Spool,
    session: &SessionId,
    track: &Track,
) -> Result<(SegmentIndex, RecoveryReport), RecorderError> {
    let index_path = spool.index_path(session, &track.id);
    let track_dir = spool.track_dir(session, &track.id);
    let mut index = index::load_or_default(&index_path, session.clone(), track.id.clone())?;
    let mut report = RecoveryReport::default();
    let mut changed = false;

    let entries = fs::read_dir(&track_dir).map_err(|error| RecorderError::Io {
        path: track_dir.display().to_string(),
        detail: error.to_string(),
    })?;
    for entry in entries {
        let entry = entry.map_err(|error| RecorderError::Recovery {
            path: track_dir.display().to_string(),
            detail: error.to_string(),
        })?;
        if !entry
            .file_type()
            .map(|kind| kind.is_file())
            .unwrap_or(false)
        {
            continue;
        }
        let name = entry.file_name();
        let Some(name) = name.to_str() else {
            continue;
        };
        if !name.starts_with("seg-") {
            continue;
        }
        let path = entry.path();
        let sequence = spool::sequence_of(&path);
        let known = index.segments.iter().any(|item| item.sequence == sequence);
        if !is_complete(&path) {
            let _ = fs::remove_file(&path);
            report.discarded += 1;
            if known {
                index.remove(sequence);
                changed = true;
            }
            continue;
        }
        if !known {
            let bytes = fs::metadata(&path)
                .map(|metadata| metadata.len())
                .map_err(|error| RecorderError::Recovery {
                    path: path.display().to_string(),
                    detail: error.to_string(),
                })?;
            index.upsert(SegmentEntry {
                sequence,
                file: name.to_string(),
                start_pts: 0,
                duration_ms: 0,
                bytes,
                codec: track.codec,
                container: spool::container_of(&path).unwrap_or(track.container),
                duplicate: None,
                complete: true,
            });
            report.finalized += 1;
            changed = true;
        }
    }

    let before = index.segments.len();
    index
        .segments
        .retain(|entry| track_dir.join(&entry.file).exists());
    if index.segments.len() != before {
        report.repaired_indexes += 1;
        changed = true;
    }
    if changed {
        index::write_atomic(&index_path, &index)?;
    }
    Ok((index, report))
}
