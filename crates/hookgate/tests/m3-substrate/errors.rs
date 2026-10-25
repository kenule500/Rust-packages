//! AC-051: every `hookgate` error renders a stable code and implements
//! `std::error::Error`.

use hookgate::{HookgateError, OutboxError, StoreError, VerifyError};

fn assert_stable_error(error: &HookgateError, code: &str, label: &str) {
    assert!(
        error.to_string().starts_with(code),
        "AC-051: {label} must render the stable code {code}, got {error}"
    );
    let _: &dyn std::error::Error = error;
}

#[test]
fn ac_051_hookgate_errors_expose_stable_codes_and_implement_error() {
    assert_stable_error(
        &HookgateError::Verify(VerifyError::InvalidSignature),
        "HG-VERIFY-SIGNATURE",
        "verification",
    );
    assert_stable_error(
        &HookgateError::Verify(VerifyError::MissingHeader("x-signature".to_owned())),
        "HG-VERIFY-MISSING-HEADER",
        "missing header",
    );
    assert_stable_error(
        &HookgateError::Store(StoreError::NotFound("delivery".to_owned())),
        "HG-STORE-NOT-FOUND",
        "store not found",
    );
    assert_stable_error(
        &HookgateError::Store(StoreError::Backend("database unavailable".to_owned())),
        "HG-STORE-BACKEND",
        "store backend",
    );
    assert_stable_error(
        &HookgateError::Outbox(OutboxError::Closed),
        "HG-OUTBOX-CLOSED",
        "outbox closed",
    );

    fn assert_error<E: std::error::Error>(error: &E) {
        let _: &dyn std::error::Error = error;
    }
    assert_error(&VerifyError::InvalidSignature);
    assert_error(&StoreError::Config("invalid".to_owned()));
    assert_error(&OutboxError::Backend("unavailable".to_owned()));
}
