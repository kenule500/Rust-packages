use std::fmt;
use std::sync::Arc;

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::timestamp::{self, TimestampDecision};
use crate::{Action, SyncError};

/// The side or value selected when a conflict is resolved.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    /// Keep the client payload.
    Client,
    /// Keep the server state unchanged.
    Server,
    /// Use a caller-supplied merged value.
    Merged(Value),
}

impl Resolution {
    /// Returns the stable lowercase name.
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Client => "client",
            Self::Server => "server",
            Self::Merged(_) => "merged",
        }
    }
}

/// Audit data for one resolved conflict.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConflictInfo {
    /// The client's timestamp at action creation.
    pub client_timestamp: DateTime<Utc>,
    /// The server's timestamp when it detected the conflict.
    pub server_timestamp: DateTime<Utc>,
    /// The resolution selected by the configured strategy.
    pub resolution: Resolution,
    /// The server's value at the time of the conflict.
    pub server_state: Option<Value>,
}

impl ConflictInfo {
    /// Builds a conflict record with a provisional server-side resolution.
    ///
    /// A transport uses this to report a conflict. The engine replaces
    /// `resolution` with the configured strategy's decision.
    pub fn new(
        client_timestamp: DateTime<Utc>,
        server_timestamp: DateTime<Utc>,
        server_state: Option<Value>,
    ) -> Self {
        Self {
            client_timestamp,
            server_timestamp,
            resolution: Resolution::Server,
            server_state,
        }
    }

    /// Replaces the recorded resolution.
    pub fn with_resolution(mut self, resolution: Resolution) -> Self {
        self.resolution = resolution;
        self
    }
}

/// A caller-supplied conflict merger.
///
/// The merger receives the original action and the server state reported by the
/// transport. Returning [`Resolution::Merged`] records the merged value in
/// [`ConflictInfo::resolution`].
pub trait CustomMerger: Send + Sync {
    /// Resolves a conflict.
    ///
    /// # Errors
    ///
    /// Return a typed [`SyncError`] when the merger cannot decide.
    fn resolve(
        &self,
        action: &Action,
        server_state: Option<&Value>,
    ) -> Result<Resolution, SyncError>;
}

/// The conflict strategy applied by a [`crate::SyncEngine`].
#[derive(Clone, Default)]
pub enum ConflictStrategy {
    /// The later of the client and server timestamps wins.
    #[default]
    LastWriteWins,
    /// The server state is kept unchanged.
    ServerWins,
    /// The client payload is kept.
    ClientWins,
    /// The caller's merger decides.
    Custom(Arc<dyn CustomMerger>),
}

impl ConflictStrategy {
    /// Wraps a custom merger.
    pub fn custom(merger: impl CustomMerger + 'static) -> Self {
        Self::Custom(Arc::new(merger))
    }

    /// Applies the strategy to a reported conflict.
    ///
    /// The transport must supply `server_state`; a conflict without it returns
    /// [`SyncError::ConflictWithoutServerState`]. The tolerance is used only for
    /// the audit decision; `LastWriteWins` always picks the later timestamp.
    ///
    /// # Errors
    ///
    /// Returns a typed error when the server state is absent or a custom merger
    /// rejects the conflict.
    pub fn resolve(
        &self,
        action: &Action,
        conflict: &mut ConflictInfo,
        tolerance: Duration,
    ) -> Result<TimestampDecision, SyncError> {
        let server_state = conflict
            .server_state
            .as_ref()
            .ok_or_else(|| SyncError::ConflictWithoutServerState(action.id.to_string()))?;
        let decision = timestamp::decide(
            action.client_timestamp,
            conflict.server_timestamp,
            tolerance,
        );

        let resolution = match self {
            Self::LastWriteWins => decision.later(),
            Self::ServerWins => Resolution::Server,
            Self::ClientWins => Resolution::Client,
            Self::Custom(merger) => merger.resolve(action, Some(server_state))?,
        };

        conflict.resolution = resolution;
        Ok(decision)
    }
}

impl fmt::Debug for ConflictStrategy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LastWriteWins => f.write_str("LastWriteWins"),
            Self::ServerWins => f.write_str("ServerWins"),
            Self::ClientWins => f.write_str("ClientWins"),
            Self::Custom(_) => f.write_str("Custom(<merger>)"),
        }
    }
}
