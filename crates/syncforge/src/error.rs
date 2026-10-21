use thiserror::Error;

/// An error returned by syncforge.
///
/// Every variant carries a stable `SF-*` code so callers can branch on the
/// failure class without matching on backend text.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum SyncError {
    /// The storage backend failed.
    #[error("SF-STORAGE: {0}")]
    Storage(String),

    /// The transport failed.
    #[error("SF-TRANSPORT: {0}")]
    Transport(String),

    /// The action was not found.
    #[error("SF-NOT-FOUND: {0}")]
    NotFound(String),

    /// An action with the same identifier is already stored.
    #[error("SF-DUPLICATE-ACTION: {0}")]
    DuplicateAction(String),

    /// The action is not in a state that permits the operation.
    #[error("SF-INVALID-STATE: {0}")]
    InvalidState(String),

    /// The action is malformed.
    #[error("SF-INVALID-ACTION: {0}")]
    InvalidAction(String),

    /// The engine or a backend is misconfigured.
    #[error("SF-CONFIG: {0}")]
    Config(String),

    /// A conflict was reported without the server state needed to resolve it.
    #[error("SF-CONFLICT-NO-SERVER-STATE: {0}")]
    ConflictWithoutServerState(String),

    /// A conflict result is missing its conflict information.
    #[error("SF-INVALID-CONFLICT: {0}")]
    InvalidConflict(String),

    /// The transport did not return a result for an action.
    #[error("SF-MISSING-RESULT: {0}")]
    MissingResult(String),
}
