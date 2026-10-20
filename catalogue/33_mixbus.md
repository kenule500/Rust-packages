# 33 — mixbus

**Package:** `mixbus` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library, optional plugin-hosting feature
**Licence:** MIT OR Apache-2.0

`mixbus` is a real-time audio mixing graph for live applications: per-source
gain, pan, mute/solo, sidechain ducking, noise gate, compression, loudness
normalisation, and optional VST3/CLAP plugin hosting.

It processes buffers. It does not open audio devices, decode files, or push
packets.

---

## Why this is not a wirekit module

No network, no credentials, no provider. It is also not `ffscene`: video
composition and audio mixing have different latency budgets and different
failure modes. They compose at the output, not inside one graph type.

---

## Scope

### Core types

```rust
pub struct Mixer {
    pub sample_rate: u32,
    pub block: usize,
    pub buses: Vec<Bus>,
}

pub struct Bus {
    pub id: BusId,
    pub gain_db: f32,
    pub mute: bool,
    pub solo: bool,
    pub inserts: Vec<Insert>,
    pub sends: Vec<Send>,
}

pub struct Source {
    pub id: SourceId,
    pub gain_db: f32,
    pub pan: f32,
    pub duck: Option<Duck>,
}

pub struct Duck {
    pub key: SourceId,
    pub threshold_db: f32,
    pub ratio: f32,
    pub attack_ms: f32,
    pub release_ms: f32,
}
```

### Processing blocks

| Block | Purpose |
|---|---|
| `Gain` | Per-source and per-bus trim |
| `Pan` | Constant-power stereo panning |
| `Gate` | Noise gate with hysteresis |
| `Compressor` | Threshold, ratio, attack, release, makeup |
| `Ducker` | Sidechain keyed by another source (music under speech) |
| `Loudness` | EBU R128 integrated loudness target with true-peak limit |
| `Plugin` | VST3/CLAP host behind the `plugin` feature |

### Operations

- **Process in place.** `process(&mut self, in: &[Frame], out: &mut [Frame])`
  with no allocation on the audio thread.
- **Graph rebuild off-thread.** Changing routing builds a new plan, then swaps
  it at a block boundary. Never lock on the audio thread.
- **Metering.** Peak, RMS, LUFS, and gain-reduction values are published per
  block through a lock-free snapshot.
- **Latency reporting.** Every insert declares its latency so the graph can
  compensate.

### Real-time rules

- No allocation, no locking, no logging, no `await` on the audio thread.
- All buffers are pre-allocated at the maximum block size.
- Plugin scanning and instantiation happen on a control thread.
- A failing plugin is bypassed, not allowed to kill the graph.

---

## Non-goals

- **Not a device layer.** Use `cpal` or the platform audio API.
- **Not a decoder.** Feed interleaved `f32` frames.
- **Not a network transport.** Use RTP/SRT/WebRTC crates.
- **Not a DAW.** No timeline, no MIDI sequencing, no automation lanes.
- **Not a plugin format.** It hosts VST3/CLAP; it does not define one.

---

## Integration

1. The app builds a `Mixer` from its audio routing document.
2. The audio callback pulls frames from sources, calls `process`, and writes to
   the device or encoder.
3. Control-plane edits (mute, gain, routing) are applied through a command
   queue drained at a block boundary.
4. Meters drive the UI from snapshots, never from the audio callback directly.

---

## Test plan

- Silence in, silence out (no DC offset, no drift).
- Gain and pan law correctness against reference values.
- Ducking attack/release timing within one block.
- Gate does not chatter on a noisy speech signal.
- Loudness normalisation hits the target within ±0.5 LU.
- Graph reconfiguration mid-stream produces no click or dropout.
- Plugin crash is contained and bypassed.
- Zero allocations on the audio thread under a custom allocator that aborts.

---

## Open questions

1. **Crate name.** `mixbus` is free; `audio-engine` is taken on crates.io.
2. **Plugin hosting scope.** VST3 and CLAP are large surfaces. Ship host
   support in 1.0, or a `mixbus-plugin` companion?
3. **Sample formats.** `f32` only, or also `i16`/`f64` interleaved?
4. **Threading model.** Caller-owned audio thread, or does `mixbus` own one?
   Caller-owned is simpler and avoids a runtime dependency.
