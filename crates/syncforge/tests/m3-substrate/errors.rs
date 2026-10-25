//! AC-051: every `syncforge` error renders a stable code and implements
//! `std::error::Error`.

use syncforge::SyncError;

fn assert_stable(error: &SyncError, code: &str, label: &str) {
    assert!(
        error.to_string().starts_with(code),
        "AC-051: {label} must render the stable code {code}, got {error}"
    );
    let _: &dyn std::error::Error = error;
}

#[test]
fn ac_051_syncforge_errors_expose_stable_codes_and_implement_error() {
    assert_stable(
        &SyncError::Storage("disk full".to_owned()),
        "SF-STORAGE",
        "storage",
    );
    assert_stable(
        &SyncError::Transport("timeout".to_owned()),
        "SF-TRANSPORT",
        "transport",
    );
    assert_stable(
        &SyncError::NotFound("action".to_owned()),
        "SF-NOT-FOUND",
        "not found",
    );
    assert_stable(
        &SyncError::DuplicateAction("action".to_owned()),
        "SF-DUPLICATE-ACTION",
        "duplicate action",
    );
    assert_stable(
        &SyncError::InvalidState("sent".to_owned()),
        "SF-INVALID-STATE",
        "invalid state",
    );
    assert_stable(
        &SyncError::InvalidAction("empty entity".to_owned()),
        "SF-INVALID-ACTION",
        "invalid action",
    );
    assert_stable(
        &SyncError::Config("bad settings".to_owned()),
        "SF-CONFIG",
        "config",
    );
    assert_stable(
        &SyncError::ConflictWithoutServerState("action".to_owned()),
        "SF-CONFLICT-NO-SERVER-STATE",
        "conflict without server state",
    );
    assert_stable(
        &SyncError::InvalidConflict("missing info".to_owned()),
        "SF-INVALID-CONFLICT",
        "invalid conflict",
    );
    assert_stable(
        &SyncError::MissingResult("action".to_owned()),
        "SF-MISSING-RESULT",
        "missing result",
    );
}
