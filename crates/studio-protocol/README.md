# studio-protocol

A JSON-only studio-control protocol. It defines the envelope, the control
messages, server-side permissions, a pure state machine, integer version
negotiation, and a fixed-window message budget.

## Scope

- One envelope and one integer protocol version.
- Pure transitions: `apply(state, envelope, actor)` returns a new state or a
  typed `Rejected`.
- Server-side permission checks before every transition.
- Duplicate-ID rejection and deterministic ordering for scene and overlay
  messages.
- A bounded 50-messages-per-second budget per participant.
- JSON round-trip for every message variant.

## Non-goals

- Transport, authentication, persistence, or rendering.
- Application/UI types.
- A global server or singleton.
- Encoding, recording, or device control.

## Message table

| Message | Payload | Permission | Ordered | State effect |
|---|---|---|---|---|
| `preview` | `SceneRef` | `control_scene` | yes | sets preview |
| `program` | `SceneRef` | `control_program` | yes | program ← scene, preview ← previous program |
| `scene_patch` | `ScenePatch` | `control_scene` | yes | updates a matching preview, keeps 64 patches |
| `overlay` | `OverlayCommand` | `control_overlay` | yes | upserts one of 32 overlays |
| `role` | `RoleChange` | `control_roles` | no | changes an existing participant's role |
| `whisper` | `WhisperMessage` | `view` | no | validated, delivered by transport |
| `teleprompter` | `TeleprompterState` | `control_teleprompter` | no | replaces teleprompter state |
| `ping` | `Ping` | `view` | no | no state change |
| `pong` | `Pong` | `view` | no | no state change |

## Permission matrix

| Role | view | control_scene | control_overlay | control_program | control_roles | control_teleprompter |
|---|---|---|---|---|---|---|
| `director` | yes | yes | yes | yes | yes | yes |
| `producer` | yes | yes | yes | yes | no | yes |
| `operator` | yes | yes | yes | no | no | yes |
| `viewer` | yes | no | no | no | no | no |

`check(role, message)` is the enforcement point. A client hiding a button is
never enforcement.

## Version rule

`v = major * 100 + minor`; the current version is `100` (1.0). Unknown majors
are rejected with `UnknownMajor`. Higher minors are accepted: minor releases
add envelope fields, and unknown envelope fields are ignored. Payload structs
use `deny_unknown_fields` so payload shape changes require a major release.

## Ordering and duplicates

- A repeated `MessageId` is rejected with `Duplicate` before any state change.
  The seen set is bounded to 512 IDs; that is the documented idempotency window.
- Scene and overlay messages older than the actor's last applied message
  timestamp are rejected with `OutOfOrder`. `role`, `whisper`, `teleprompter`,
  `ping`, and `pong` bypass the ordering rule.
- A denied, duplicate, or stale message leaves the input state untouched.

## Budget

`MessageBudget::allow(participant, now)` counts messages in a wall-clock
second and returns `RateLimited` at 50. There is no queue; the caller decides
whether to drop or retry.

## Example

```text
cargo run -p studio-protocol --example replay_log
```
