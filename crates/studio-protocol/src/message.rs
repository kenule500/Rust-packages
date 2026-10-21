//! Message payloads and their documented bounds.

use std::fmt;

use serde::{Deserialize, Serialize};

use crate::envelope::{is_safe_identifier, ParticipantId};
use crate::error::Rejected;
use crate::permission::Role;

/// A validated scene identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SceneId(String);

impl SceneId {
    /// Validates and creates a scene identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, Rejected> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err(Rejected::InvalidPayload {
                detail: "scene id must be 1-64 ASCII alphanumerics, '-' or '_'".to_string(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SceneId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A validated overlay identifier.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OverlayId(String);

impl OverlayId {
    /// Validates and creates an overlay identifier.
    pub fn new(value: impl Into<String>) -> Result<Self, Rejected> {
        let value = value.into();
        if !is_safe_identifier(&value) {
            return Err(Rejected::InvalidPayload {
                detail: "overlay id must be 1-64 ASCII alphanumerics, '-' or '_'".to_string(),
            });
        }
        Ok(Self(value))
    }

    /// Returns the identifier as a string slice.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for OverlayId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// A scene reference used by preview and program.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SceneRef {
    /// Scene identifier.
    pub scene: SceneId,
    /// Human-readable display name.
    pub name: String,
}

impl SceneRef {
    /// Creates a scene reference.
    #[must_use]
    pub fn new(scene: SceneId, name: impl Into<String>) -> Self {
        Self {
            scene,
            name: name.into(),
        }
    }

    /// Validates the display name.
    pub fn validate(&self) -> Result<(), Rejected> {
        validate_text(&self.name, 256, "scene name")
    }
}

/// A bounded scene patch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ScenePatch {
    /// Scene the patch targets.
    pub scene: SceneRef,
    /// Monotonic revision reported by the scene editor.
    pub revision: u64,
    /// Human-readable change descriptions.
    pub changes: Vec<String>,
}

impl ScenePatch {
    /// Validates the patch bounds.
    pub fn validate(&self) -> Result<(), Rejected> {
        self.scene.validate()?;
        if self.changes.len() > 32 {
            return Err(Rejected::InvalidPayload {
                detail: "a scene patch may carry at most 32 changes".to_string(),
            });
        }
        for change in &self.changes {
            validate_text(change, 512, "patch change")?;
        }
        Ok(())
    }
}

/// An overlay command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OverlayCommand {
    /// Overlay identifier.
    pub id: OverlayId,
    /// Whether the overlay should be visible.
    pub visible: bool,
    /// Overlay text.
    pub text: String,
}

impl OverlayCommand {
    /// Validates the overlay text.
    pub fn validate(&self) -> Result<(), Rejected> {
        validate_text(&self.text, 2_000, "overlay text")
    }
}

/// An overlay retained in the studio state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Overlay {
    /// Overlay identifier.
    pub id: OverlayId,
    /// Overlay text.
    pub text: String,
    /// Whether the overlay is visible.
    pub visible: bool,
}

/// A role change for an existing participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RoleChange {
    /// Participant to change.
    pub participant: ParticipantId,
    /// New role.
    pub role: Role,
}

/// A private whisper to an existing participant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhisperMessage {
    /// Recipient.
    pub to: ParticipantId,
    /// Whisper text.
    pub text: String,
}

impl WhisperMessage {
    /// Validates the whisper text.
    pub fn validate(&self) -> Result<(), Rejected> {
        validate_text(&self.text, 2_000, "whisper text")
    }
}

/// Teleprompter control state.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TeleprompterState {
    /// Script text.
    pub text: String,
    /// Scroll speed in arbitrary units, `0.0..=10.0`.
    pub speed: f32,
    /// Whether scrolling is running.
    pub running: bool,
}

impl TeleprompterState {
    /// Validates the script and speed.
    pub fn validate(&self) -> Result<(), Rejected> {
        validate_text(&self.text, 20_000, "teleprompter text")?;
        if !self.speed.is_finite() || !(0.0..=10.0).contains(&self.speed) {
            return Err(Rejected::InvalidPayload {
                detail: "teleprompter speed must be finite and within 0.0..=10.0".to_string(),
            });
        }
        Ok(())
    }
}

impl Default for TeleprompterState {
    fn default() -> Self {
        Self {
            text: String::new(),
            speed: 1.0,
            running: false,
        }
    }
}

/// A liveness ping.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Ping {
    /// Correlation nonce.
    pub nonce: u64,
}

/// A liveness reply.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Pong {
    /// Correlation nonce from the ping.
    pub nonce: u64,
}

/// One protocol payload.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum Message {
    /// Set the preview scene.
    Preview(SceneRef),
    /// Cut a scene to program and move the previous program to preview.
    Program(SceneRef),
    /// Apply a scene patch.
    ScenePatch(ScenePatch),
    /// Show or hide an overlay.
    Overlay(OverlayCommand),
    /// Change a participant's role.
    Role(RoleChange),
    /// Send a private message.
    Whisper(WhisperMessage),
    /// Control the teleprompter.
    Teleprompter(TeleprompterState),
    /// Liveness ping.
    Ping(Ping),
    /// Liveness reply.
    Pong(Pong),
}

impl Message {
    /// Returns the stable wire name.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Preview(_) => "preview",
            Self::Program(_) => "program",
            Self::ScenePatch(_) => "scene_patch",
            Self::Overlay(_) => "overlay",
            Self::Role(_) => "role",
            Self::Whisper(_) => "whisper",
            Self::Teleprompter(_) => "teleprompter",
            Self::Ping(_) => "ping",
            Self::Pong(_) => "pong",
        }
    }

    /// Returns whether the message is exempt from the ordering rule.
    #[must_use]
    pub const fn is_ordering_exempt(&self) -> bool {
        !self.is_ordered()
    }

    /// Returns whether the message participates in the scene/overlay ordering
    /// rule.
    #[must_use]
    pub const fn is_ordered(&self) -> bool {
        matches!(
            self,
            Self::Preview(_) | Self::Program(_) | Self::ScenePatch(_) | Self::Overlay(_)
        )
    }

    /// Validates payload content bounds.
    pub fn validate(&self) -> Result<(), Rejected> {
        match self {
            Self::Preview(scene) | Self::Program(scene) => scene.validate(),
            Self::ScenePatch(patch) => patch.validate(),
            Self::Overlay(command) => command.validate(),
            Self::Role(_) | Self::Ping(_) | Self::Pong(_) => Ok(()),
            Self::Whisper(whisper) => whisper.validate(),
            Self::Teleprompter(state) => state.validate(),
        }
    }
}

fn validate_text(text: &str, max: usize, label: &str) -> Result<(), Rejected> {
    if text.chars().count() > max {
        return Err(Rejected::InvalidPayload {
            detail: format!("{label} exceeds {max} characters"),
        });
    }
    if text.chars().any(char::is_control) {
        return Err(Rejected::InvalidPayload {
            detail: format!("{label} contains control characters"),
        });
    }
    Ok(())
}
