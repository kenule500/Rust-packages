//! Protocol rejection and JSON error types with stable codes.

use std::fmt;

use crate::envelope::{MessageId, ParticipantId};
use crate::permission::{Permission, Role};

/// A JSON encoding or decoding failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProtocolError {
    /// The value could not be encoded.
    Encode {
        /// Human-readable detail.
        detail: String,
    },
    /// The input was not a valid envelope.
    Decode {
        /// Human-readable detail.
        detail: String,
    },
}

impl ProtocolError {
    /// Returns the stable error code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::Encode { .. } => "PRO-101",
            Self::Decode { .. } => "PRO-102",
        }
    }
}

impl fmt::Display for ProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.code())?;
        match self {
            Self::Encode { detail } => write!(f, "cannot encode envelope: {detail}"),
            Self::Decode { detail } => write!(f, "cannot decode envelope: {detail}"),
        }
    }
}

impl std::error::Error for ProtocolError {}

/// A rejected message. Rejection never changes the state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Rejected {
    /// The actor's role does not grant the permission the message requires.
    PermissionDenied {
        /// The actor's role.
        role: Role,
        /// The required permission.
        permission: Permission,
    },
    /// The envelope's major version is unknown.
    UnknownMajor {
        /// The rejected version integer.
        version: u16,
    },
    /// The message ID was already applied.
    Duplicate {
        /// The repeated ID.
        id: MessageId,
    },
    /// A scene or overlay message is older than the actor's last applied message.
    OutOfOrder {
        /// The actor that sent the stale message.
        actor: ParticipantId,
    },
    /// A role or whisper target is not a participant.
    UnknownParticipant {
        /// The missing participant.
        participant: ParticipantId,
    },
    /// The payload is malformed or exceeds a documented bound.
    InvalidPayload {
        /// Human-readable detail.
        detail: String,
    },
    /// The envelope's `from` field does not match the authenticated actor.
    ActorMismatch {
        /// The participant declared in the envelope.
        declared: ParticipantId,
        /// The authenticated actor.
        actor: ParticipantId,
    },
    /// The participant exhausted the fixed-window message budget.
    RateLimited {
        /// The limited participant.
        participant: ParticipantId,
    },
}

impl Rejected {
    /// Returns the stable rejection code.
    #[must_use]
    pub const fn code(&self) -> &'static str {
        match self {
            Self::PermissionDenied { .. } => "PRO-001",
            Self::UnknownMajor { .. } => "PRO-002",
            Self::Duplicate { .. } => "PRO-003",
            Self::OutOfOrder { .. } => "PRO-004",
            Self::UnknownParticipant { .. } => "PRO-005",
            Self::InvalidPayload { .. } => "PRO-006",
            Self::ActorMismatch { .. } => "PRO-007",
            Self::RateLimited { .. } => "PRO-008",
        }
    }
}

impl fmt::Display for Rejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "[{}] ", self.code())?;
        match self {
            Self::PermissionDenied { role, permission } => {
                write!(f, "role {role:?} lacks permission {permission:?}")
            }
            Self::UnknownMajor { version } => {
                write!(f, "unknown protocol major in version {version}")
            }
            Self::Duplicate { id } => write!(f, "duplicate message {id}"),
            Self::OutOfOrder { actor } => write!(f, "out-of-order message from {actor}"),
            Self::UnknownParticipant { participant } => {
                write!(f, "unknown participant {participant}")
            }
            Self::InvalidPayload { detail } => write!(f, "invalid payload: {detail}"),
            Self::ActorMismatch { declared, actor } => {
                write!(f, "envelope declares {declared} but actor is {actor}")
            }
            Self::RateLimited { participant } => {
                write!(f, "participant {participant} exceeded the message budget")
            }
        }
    }
}

impl std::error::Error for Rejected {}
