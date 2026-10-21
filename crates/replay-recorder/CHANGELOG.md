# Changelog

All notable changes to `replay-recorder` are documented here.

## 0.1.0 - unreleased

- Isolated per-track packet recording with monotonic timestamp checks.
- Segment, tee, and bounded replay-ring sinks.
- Duration and size rotation with atomic JSON indexes.
- Disk budget with stop-oldest, stop-newest, and refuse policies.
- Deterministic crash recovery from a clean segment footer.
- Ring clip extraction without interrupting the active segment.
