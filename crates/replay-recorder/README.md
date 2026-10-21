# replay-recorder

Packet-in replay recording. `replay-recorder` accepts already-encoded packets,
writes isolated per-track segments into a spool, keeps an atomic JSON index,
enforces a disk budget, and extracts clips from a bounded replay ring.

## Scope

- Per-track start, stop, and monotonic timestamp checks.
- Segment rotation by duration or size.
- `Sink::Segment`, `Sink::Tee`, and `Sink::Ring`.
- Crash recovery from a clean segment trailer.
- Clip extraction without touching the active segment.
- Validated identifiers and traversal-safe paths.

## Non-goals

- Encoding, decoding, muxing, or remuxing; packets are opaque payloads.
- Uploading, CDN orchestration, databases, or a network server.
- A global recorder singleton.
- Querying the operating system for free space. The caller supplies
  `free_bytes` so the budget policy is portable and testable.

## Spool layout

```text
<root>/<session>/track-<id>/seg-<n>.mkv
<root>/<session>/track-<id>/index.json
<root>/<session>/clips/clip-<n>.mkv
```

Identifiers are 1-64 ASCII alphanumerics, `-`, or `_`, must start
alphanumeric, and reject Windows reserved device names. User input never becomes
a path segment directly.

## Index and recovery

`index.json` is written through a temporary file, flushed, synced, and renamed.
A segment is complete only when it ends with the 8-byte `M4SEGFTR` footer. On
open, a complete unindexed segment is adopted; a partial segment is removed.
Missing files are dropped from the index.

## Disk budget

`DiskBudget::evaluate` compares the retained bytes and the caller-supplied free
bytes with `max_bytes` and `min_free_bytes`. Pressure applies one of:

- `StopOldest`: delete the oldest complete segment and continue.
- `StopNewest`: finalise the affected track and record an event.
- `Refuse`: return a typed error.

Every action is reported through `Recorder::events` and `drain_events`.

## Replay ring

The ring retains the newest segments up to a duration and byte cap. Eviction
deletes the segment file and removes it from the index. `Recorder::clip` copies
the packet payloads of the selected segments into `clips/clip-<n>.mkv`, stripping
per-segment footers and writing one clean footer.

## Example

```text
cargo run -p replay-recorder --example record_and_clip
```
