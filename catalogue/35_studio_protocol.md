# 35 — studio-protocol

**Package:** `studio-protocol` (standalone crate)
**Status:** blueprint, not published
**Crate type:** library: message types + state machine; optional `axum` transport
**Licence:** MIT OR Apache-2.0

`studio-protocol` is the control-plane protocol for a live production studio:
preview/program switching, scene and overlay commands, role permissions, ghost
producer, whisper mode, and teleprompter sync.

It defines messages and a state machine. It does not carry media.

---

## Why this is not a wirekit module

No provider, no HTTP API. It is also not `ffscene`: the scene describes the
picture; the protocol describes who may change it and when.

---

## Scope

### Envelope

```rust
pub struct Envelope {
    pub v: u16,             // protocol version
    pub id: MessageId,      // UUID v7
    pub at: DateTime<Utc>,
    pub from: ParticipantId,
    pub body: Message,
}
```

### Messages

| Message | Direction | Purpose |
|---|---|---|
| `Preview` | Director → engine | Put a scene on preview |
| `Program` | Director → engine | Cut preview to program |
| `ScenePatch` | Any permitted → engine | Add/move/remove a layer |
| `Overlay` | Any permitted → engine | Lower-third, alert, scoreboard |
| `Role` | Director → engine | Grant/revoke a participant's permissions |
| `Whisper` | Director → participant | Private note, never on program audio |
| `Teleprompter` | Operator → participants | Script position and speed |
| `Ping` / `Pong` | Both | Latency and liveness |

### Permissions

```rust
pub enum Permission {
    View,
    ControlScene,
    ControlOverlay,
    ControlProgram,
    ControlRoles,
    ControlTeleprompter,
}
```

Permissions are checked server-side. A client may render a disabled button; it
must never be the enforcement point.

### State machine

`StudioState` holds program scene, preview scene, participants, and overlay
stack. Every transition is a pure function of `(state, message, actor)`, which
makes replay and property testing possible.

---

## Non-goals

- **Not a media transport.** Use WebRTC/SRT/RTMP.
- **Not a chat protocol.** Inbox and comments are a different domain.
- **Not a rendering engine.** It emits state; `ffscene` renders.
- **Not a persistence layer.** The app decides what to store.

---

## Integration

1. The studio service owns `StudioState` and applies messages.
2. Clients connect over WebSocket; `studio-protocol` provides framing and
   validation, not the socket.
3. On a program cut, the service emits a render command to the media engine.
4. Teleprompter position is broadcast at a fixed cadence, not per keystroke.

---

## Test plan

- Every message round-trips through JSON and preserves IDs.
- Permission matrix: each role × each message → allowed/denied.
- Program cut is atomic: no state where program and preview are both live.
- Out-of-order messages are rejected or buffered deterministically.
- Replay of a recorded message log reproduces the final state exactly.
- Version negotiation rejects unknown major versions and accepts minor ones.
- Latency budget: 50 messages/second per participant without backpressure.

---

## Open questions

1. **Version skew.** Is `v` a single integer or semver? A single integer with
   “minor may add fields” is simpler.
2. **Authorization model.** Roles only, or arbitrary permission sets per
   participant?
3. **Teleprompter sync.** Server-authoritative position, or client-clock
   interpolation from periodic keyframes?
4. **Binary encoding.** JSON first; is a `postcard`/`bincode` feature worth it?
