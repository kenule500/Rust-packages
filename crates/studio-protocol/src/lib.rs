//! JSON studio-control protocol.
//!
//! `studio-protocol` defines the wire envelope, the control messages, the
//! server-side permission table, a pure state machine, version negotiation, and
//! a per-participant message budget.
//!
//! # Scope
//!
//! - JSON only: one envelope shape and one integer protocol version.
//! - Pure transitions: `apply(state, envelope, actor)` returns a new state or a
//!   typed rejection and never mutates shared state.
//! - Permissions are enforced server-side before every transition. A client
//!   disabling a button is presentation, never enforcement.
//! - Duplicate message IDs are rejected idempotently; stale scene and overlay
//!   messages are rejected deterministically.
//! - A bounded fixed-window budget of 50 messages per participant per second.
//!
//! # Non-goals
//!
//! - Transport, sockets, authentication, or persistence.
//! - Application/UI types or rendering.
//! - A global server or singleton.
//! - Encoding, recording, or device control.
//!
//! # Version rule
//!
//! [`PROTOCOL_VERSION`] is one integer: the major is `v / 100` and the minor is
//! `v % 100`. An unknown major is rejected. A higher minor is accepted because
//! minor releases are additive; unknown envelope fields are ignored.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod budget;
mod envelope;
mod error;
mod message;
mod permission;
mod state;
mod transition;
mod version;

pub use budget::MessageBudget;
pub use envelope::{Envelope, MessageId, ParticipantId, TenantId};
pub use error::{ProtocolError, Rejected};
pub use message::{
    Message, Overlay, OverlayCommand, OverlayId, Ping, Pong, RoleChange, SceneId, ScenePatch,
    SceneRef, TeleprompterState, WhisperMessage,
};
pub use permission::{check, permission_for, Permission, Role};
pub use state::{Actor, BoundedSet, Participant, StudioState};
pub use transition::apply;
pub use version::{major, minor, negotiate, Version, PROTOCOL_VERSION};
