# mixbus

Real-time-safe audio mixing for caller-owned blocks. `mixbus` applies gain,
constant-power pan, gating, compression, sidechain ducking, EBU R128-style
loudness measurement, latency compensation, and true-peak limiting.

## Scope

- Interleaved stereo processing with no allocation, lock, log, or await on the
  audio path.
- Pre-allocated buffers at the configured maximum block size.
- A bounded control queue drained at block start; plan swaps between blocks.
- Lock-free peak, RMS, LUFS, and gain-reduction meters.
- Per-source, per-bus, and master meter slots.

## Non-goals

- Opening devices, decoding files, resampling, or owning a thread.
- Hosting VST3 or CLAP plugins.
- Async runtimes, logging, or persistence.
- Application routing UI.

## Buffer contract

`process(input, output)` takes `frames * sources * 2` input samples in
source-major interleaved order and writes `frames * 2` stereo samples. `frames`
must not exceed the configured [`BlockSize`]. A block never reallocates; the
plan's scratch buffers are sized during construction.

## Real-time rules

- The audio thread only calls `process` and the meter readers.
- `install_plan` and the `set_*` methods are control-thread calls made between
  blocks. Control changes enter a fixed 64-entry queue.
- A plan is built off the audio thread with `MixPlan::build` and swapped at a
  block boundary with `Mixer::install_plan`.
- Input NaN or infinity is replaced with zero and flagged in the meter.

## Latency

Each insert reports `latency_frames`. The plan sums the bus latency, delays
shorter parallel buses to the maximum, and exposes the result through
`Mixer::latency_frames`. Limiter look-ahead contributes latency; gate and
compressor are zero-latency.

## Meters

`Mixer::meter`, `Mixer::source_meter`, and `Mixer::bus_meter` read a
double-buffered atomic snapshot. Readers never call into the processing path.
LUFS is a gated integrated value over the last 64 × 400 ms blocks.

## Example

```text
cargo run -p mixbus --example process_block
```
