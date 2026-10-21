use std::fmt;

use bytes::Bytes;
use chrono::{DateTime, Utc};
use thiserror::Error;

use crate::store::{OutboxError, StoreError};
use crate::verify::VerifyError;
use crate::{DeliveryId, ProviderId, TenantId};

/// A delivery that exhausted its attempts and was moved out of the queue.
#[derive(Clone)]
pub struct DeadLetterRecord {
    /// The original delivery identity, preserved for replay.
    pub delivery: DeliveryId,
    /// The tenant that owns the connection.
    pub tenant: TenantId,
    /// The provider family that sent the request.
    pub provider: ProviderId,
    /// The exact verified bytes.
    pub raw: Bytes,
    /// The redacted handler error.
    pub error: String,
    /// The instant the delivery was dead-lettered.
    pub dead_lettered_at: DateTime<Utc>,
}

impl fmt::Debug for DeadLetterRecord {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeadLetterRecord")
            .field("delivery", &self.delivery)
            .field("tenant", &self.tenant)
            .field("provider", &self.provider)
            .field("raw_len", &self.raw.len())
            .field("error", &self.error)
            .field("dead_lettered_at", &self.dead_lettered_at)
            .finish()
    }
}

/// The umbrella error for hookgate operations.
#[derive(Debug, Error)]
#[non_exhaustive]
pub enum HookgateError {
    /// Verification failed.
    #[error(transparent)]
    Verify(#[from] VerifyError),
    /// The delivery store failed.
    #[error(transparent)]
    Store(#[from] StoreError),
    /// The outbox failed.
    #[error(transparent)]
    Outbox(#[from] OutboxError),
}

/// Removes likely secret material from an error message and caps its length.
///
/// The redactor replaces the values of `secret`, `token`, `password`,
/// `authorization`, `signature`, `api_key`, `apikey`, and `credential`
/// key-value pairs, plus the token after `Bearer`. It is best-effort defense in
/// depth; callers should still avoid putting secrets into error strings.
pub fn redact_error(error: &str) -> String {
    const SENSITIVE: &[&str] = &[
        "secret",
        "token",
        "password",
        "authorization",
        "signature",
        "api_key",
        "apikey",
        "credential",
    ];
    const MAX_CHARS: usize = 512;

    let mut parts = Vec::new();
    let mut redact_next = false;
    for token in error.split_whitespace() {
        if redact_next {
            parts.push("[REDACTED]".to_owned());
            redact_next = false;
            continue;
        }
        if token.eq_ignore_ascii_case("bearer") {
            parts.push(token.to_owned());
            redact_next = true;
            continue;
        }
        if let Some((key, _)) = token.split_once('=') {
            let lower = key.to_ascii_lowercase();
            if SENSITIVE.iter().any(|sensitive| lower.contains(sensitive)) {
                parts.push(format!("{key}=[REDACTED]"));
                continue;
            }
        }
        parts.push(token.to_owned());
    }

    parts.join(" ").chars().take(MAX_CHARS).collect()
}
