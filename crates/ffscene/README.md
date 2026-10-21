# ffscene

Deterministic scene-to-FFmpeg compilation. `ffscene` validates an in-memory
scene, builds a stable `filter_complex` graph and argument vector, expands an
encoder ladder, probes an injected encoder list, and describes outputs as data.

## Scope

- Pure validation and compilation: no clock, randomness, DNS, sockets, or
  processes.
- Deterministic ordering and labels, byte-identical for identical input.
- `Output::{File, Rtmp, Srt, Whip}` descriptors with shape validation.
- Explicit encoder selection and downgrade reporting.

## Non-goals

- Running FFmpeg or transporting media.
- Incremental recompilation or live preview.
- Owning the application's scene document schema.
- Connection health, retries, and failover (`rtmp-publish`).

## FFmpeg dialect boundary

The generated graph targets the FFmpeg 7.x filter syntax: `color`, `scale`,
`overlay`, `blend`, `format`, and `setpts`. Hardware encoders are named but not
invoked. A future FFmpeg dialect change is a versioned crate change, not a
runtime guess.

## Determinism

Layers are ordered by `(z, layer id, source id)`, sources are sorted, and labels
are derived only from validated identifiers. No timestamp, process id, address,
or hash iteration order is emitted.

## Usage

```rust
use ffscene::*;

let canvas = Canvas::new(1920, 1080, Rational::new(30, 1)?)?;
let scene = Scene::new(
    canvas,
    Background::rgba(0, 0, 0, 255),
    vec![Layer::new(
        LayerId::new("cam")?,
        SourceId::new("camera")?,
        Transform::new(0, 0, 1920, 1080),
        0,
    )],
);
let graph = compile(&scene, &Output::File { path: "out.mkv".into() })?;
assert!(graph.filter_complex.contains("overlay"));
# Ok::<(), ffscene::CompileError>(())
```

Run the offline example:

```text
cargo run -p ffscene --example compile_scene
```
