# 32 — ffscene

**Package:** `ffscene` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library
**Licence:** MIT OR Apache-2.0

`ffscene` turns a declarative scene — layers, sources, transforms, encoder
settings — into a valid FFmpeg `filter_complex` graph and a set of output
descriptors. It is the composition model behind a live studio, without the
studio.

It compiles; it does not encode. It emits the arguments, the graph, and the
mapping from logical layer to filter label. A separate process runs FFmpeg.

---

## Why this is not a wirekit module

`wirekit` is an HTTP client for provider APIs. `ffscene` is a local media graph
compiler with no network and no credentials. They share nothing but Rust.

It is also not `mixbus` (audio) and not `rtmp-publish` (egress): a scene can be
rendered to a file with no network at all.

---

## Scope

### Core types

```rust
pub struct Canvas { pub width: u32, pub height: u32, pub fps: Rational }

pub struct Scene {
    pub canvas: Canvas,
    pub background: Background,
    pub layers: Vec<Layer>,
}

pub struct Layer {
    pub id: LayerId,
    pub source: SourceId,
    pub transform: Transform,   // scale, crop, position, rotation
    pub z: i32,
    pub visible: bool,
    pub blend: BlendMode,
    pub opacity: f32,
}

pub struct EncoderProfile {
    pub codec: Codec,           // H264, HEVC, AV1
    pub rate_control: RateControl, // CBR, VBR, CRF
    pub preset: String,
    pub keyint: u32,
    pub profile: Option<String>,
}

pub struct Ladder { pub rungs: Vec<Rung> }
```

### Operations

- **Compile.** `compile(&Scene, &Output) -> Result<CompiledGraph, CompileError>`
  returns `filter_complex`, input labels, output labels, and per-layer mapping.
- **Validate.** Reject cycles, missing sources, out-of-range transforms, and
  unsupported blend modes *before* FFmpeg starts.
- **Ladder.** Expand one scene into several encoder profiles (720p/1080p) with
  correct `scale` and `split` nodes.
- **GPU probe.** Detect available encoders (`h264_nvenc`, `hevc_qsv`,
  `h264_amf`, `libx264`) and downgrade the profile explicitly, never silently.
- **Output descriptors.** Emit `Output::File`, `Output::Rtmp`, `Output::Srt`,
  `Output::Whip` — the descriptor only; `rtmp-publish` owns connection health.

### Determinism

The same `Scene` must compile to byte-identical arguments. Node order is
stable, labels are deterministic, and no timestamps appear in the graph. This
makes golden-file tests possible.

---

## Non-goals

- **Not an encoder.** No FFmpeg bindings, no process supervision. Emit args.
- **Not a capture layer.** Screen, window, camera, and NDI capture stay in the
  platform layer.
- **Not an audio mixer.** Use `mixbus`; the two compose at the output stage.
- **Not a UI.** No layout editing, no drag-and-drop.
- **Not a transport.** No RTMP/SRT sockets.

---

## Integration

1. The app owns the scene document and its persistence.
2. `ffscene` validates and compiles it to a graph.
3. The media service launches FFmpeg with the emitted arguments, or feeds the
   graph to a long-lived encoder process.
4. Scene edits recompile a subgraph rather than restarting the whole pipeline
   where the output allows it.

`ffscene` does not know what a “stream” is. It knows what a canvas and a layer
are.

---

## Test plan

- Golden files: a fixed scene compiles to an exact argument vector.
- Cycle detection and missing-source rejection.
- Transform math against known crop/scale cases.
- Ladder expansion produces the expected split/scale chain.
- GPU downgrade path is explicit and logged.
- 4K/60 scene compiles without overflow or label collision.
- Determinism: compile twice, assert byte equality.

---

## Open questions

1. **Crate name.** `ffscene` is free; `media-engine` and `scene-graph` are taken
   on crates.io. Confirm `ffscene` before publish.
2. **Graph dialect.** Target FFmpeg 7.x filter syntax only, or keep a
   compatibility shim?
3. **Scene schema ownership.** Does the JSON schema live in this crate or in the
   app? Publishing it makes the crate opinionated; keeping it out makes it
   flexible.
4. **Preview path.** Should `ffscene` emit a low-resolution preview graph, or
   is preview the app's problem?
