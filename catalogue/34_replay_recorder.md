# 34 — replay-recorder

**Package:** `replay-recorder` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library, optional `ffmpeg` feature
**Licence:** MIT OR Apache-2.0

`replay-recorder` captures multiple isolated tracks, keeps a rolling replay
buffer, and writes segmented or tee-muxed output with a disk budget.

It is the recording layer under a live studio. It does not encode video for
delivery and it does not stream.

---

## Why this is not a wirekit module

No network, no provider. It is also not `ffscene`: composition decides what the
program looks like; recording decides what is preserved and for how long.

---

## Scope

### Core types

```rust
pub struct Recorder {
    pub tracks: Vec<Track>,
    pub sink: Sink,
    pub budget: DiskBudget,
}

pub struct Track {
    pub id: TrackId,
    pub source: SourceId,
    pub codec: TrackCodec,   // H264, HEVC, AAC, Opus, PCM
    pub container: Container, // MP4, MKV
    pub enabled: bool,
}

pub enum Sink {
    Segment { dir: PathBuf, duration: Duration, pattern: String },
    Tee { outputs: Vec<PathBuf> },
    Ring { dir: PathBuf, seconds: u32 },
}

pub struct DiskBudget {
    pub max_bytes: u64,
    pub min_free_bytes: u64,
    pub on_pressure: PressurePolicy, // StopOldest, StopNewest, Refuse
}
```

### Operations

- **Start/stop per track.** Isolated tracks mean one source can be remuxed
  without touching the others.
- **Replay ring.** Keep the last *N* seconds in a bounded ring on disk and
  expose `clip(last: Duration) -> ClipHandle` without stopping the recording.
- **Segment and tee.** Segment by duration or size; tee the same encoded stream
  to local disk and a network sink.
- **Disk pressure.** Enforce `max_bytes` and `min_free_bytes`; apply the
  configured policy and emit an event. Never fill the volume.
- **Crash recovery.** On restart, scan the spool and finalise or discard
  partial segments deterministically.

### File layout

```
spool/
  <session-id>/
    track-<id>/
      seg-000001.mkv
      seg-000002.mkv
      index.json
```

The index is written after each segment close, so a crash loses at most one
segment.

---

## Non-goals

- **Not an encoder implementation.** Use FFmpeg, `openh264`, or hardware.
- **Not a transport.** No RTMP/SRT/WebRTC.
- **Not a NLE.** No timeline editing, no transitions.
- **Not a cloud uploader.** It writes files; the app ships them.

---

## Integration

1. The app opens a `Recorder` with a sink and a budget.
2. Each source attaches a `Track`.
3. The media pipeline writes encoded packets to the track.
4. Clip extraction reads the ring and produces a self-contained file.
5. On shutdown, `finalise()` closes segments and flushes the index.

---

## Test plan

- Kill the process mid-segment; restart recovers all but the last segment.
- Ring buffer produces a clip that decodes cleanly and has the requested
  duration ±1 frame.
- Disk budget triggers at the configured threshold and respects the policy.
- Tee output is byte-identical to the local segment.
- Many tracks (16) do not drop packets under sustained load.
- Clock changes and monotonic timestamps do not corrupt segment durations.

---

## Open questions

1. **Crate name.** `replay-recorder` is free; `recorder` is taken.
2. **Container default.** MKV for crash resilience, MP4 for compatibility. Ship
   both, default to MKV?
3. **Ring implementation.** Pre-allocated sparse file, or segment rotation? A
   sparse file is simpler to clip; rotation is safer across filesystems.
4. **Hardware encode.** Does the crate own encoder selection, or accept an
   already-encoded packet stream?
