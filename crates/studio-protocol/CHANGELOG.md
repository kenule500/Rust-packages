# Changelog

All notable changes to `studio-protocol` are documented here.

## 0.1.0 - unreleased

- JSON envelope and the preview, program, scene patch, overlay, role, whisper,
  teleprompter, ping, and pong messages.
- Server-side role and permission table.
- Pure state machine with a single-assignment program cut.
- Integer major/minor negotiation with unknown-major rejection.
- Duplicate-ID rejection and deterministic scene/overlay ordering.
- Fixed-window 50-messages-per-second participant budget.
