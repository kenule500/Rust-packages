//! Roles, permissions, and the single server-side permission table.

use serde::{Deserialize, Serialize};

use crate::error::Rejected;
use crate::message::Message;

/// A capability required by a message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Permission {
    /// Read studio state.
    View,
    /// Change the preview or patch a scene.
    ControlScene,
    /// Show or hide overlays.
    ControlOverlay,
    /// Cut a scene to program.
    ControlProgram,
    /// Change participant roles.
    ControlRoles,
    /// Control the teleprompter.
    ControlTeleprompter,
}

/// A studio role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// Full control, including roles.
    Director,
    /// Program, scene, overlay, and teleprompter control.
    Producer,
    /// Scene, overlay, and teleprompter control.
    Operator,
    /// Read-only.
    Viewer,
}

const DIRECTOR: [Permission; 6] = [
    Permission::View,
    Permission::ControlScene,
    Permission::ControlOverlay,
    Permission::ControlProgram,
    Permission::ControlRoles,
    Permission::ControlTeleprompter,
];
const PRODUCER: [Permission; 5] = [
    Permission::View,
    Permission::ControlScene,
    Permission::ControlOverlay,
    Permission::ControlProgram,
    Permission::ControlTeleprompter,
];
const OPERATOR: [Permission; 4] = [
    Permission::View,
    Permission::ControlScene,
    Permission::ControlOverlay,
    Permission::ControlTeleprompter,
];
const VIEWER: [Permission; 1] = [Permission::View];

impl Role {
    /// Returns the permissions granted by the role.
    #[must_use]
    pub const fn permissions(self) -> &'static [Permission] {
        match self {
            Self::Director => &DIRECTOR,
            Self::Producer => &PRODUCER,
            Self::Operator => &OPERATOR,
            Self::Viewer => &VIEWER,
        }
    }

    /// Returns whether the role grants the permission.
    #[must_use]
    pub fn allows(self, permission: Permission) -> bool {
        self.permissions().contains(&permission)
    }
}

/// Returns the permission a message requires.
#[must_use]
pub const fn permission_for(message: &Message) -> Permission {
    match message {
        Message::Preview(_) | Message::ScenePatch(_) => Permission::ControlScene,
        Message::Program(_) => Permission::ControlProgram,
        Message::Overlay(_) => Permission::ControlOverlay,
        Message::Role(_) => Permission::ControlRoles,
        Message::Teleprompter(_) => Permission::ControlTeleprompter,
        Message::Whisper(_) | Message::Ping(_) | Message::Pong(_) => Permission::View,
    }
}

/// Checks a message against a role before any transition.
///
/// This is the server-side enforcement point. UI button state is never trusted.
pub fn check(role: Role, message: &Message) -> Result<(), Rejected> {
    let permission = permission_for(message);
    if role.allows(permission) {
        Ok(())
    } else {
        Err(Rejected::PermissionDenied { role, permission })
    }
}
