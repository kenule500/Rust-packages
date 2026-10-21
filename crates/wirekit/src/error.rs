//! The single error type every provider maps into.
//!
//! Scope: one `#[non_exhaustive]` enum with stable `W####` codes, a redacted
//! `Debug`, and helpers for retry classification.
//!
//! Non-goals: provider-specific error enums (CON-207), raw response bodies in
//! the payload, and credentials anywhere in `Debug` or `Display` (CON-214).

use std::error::Error;
use std::fmt;
use std::time::Duration;

/// Stable error reason, `WirekitError::Api` (an HTTP status and a provider
/// code).
pub const CODE_API: &str = "W2001";
/// Stable error reason, `WirekitError::Auth` (a rejected credential).
pub const CODE_AUTH: &str = "W2002";
/// Stable error reason, `WirekitError::RateLimit` (the bucket or the provider
/// refused the request).
pub const CODE_RATE_LIMIT: &str = "W2003";
/// Stable error reason, `WirekitError::Timeout` (the request timed out).
pub const CODE_TIMEOUT: &str = "W2004";
/// Stable error reason, `WirekitError::Network` (the transport failed).
pub const CODE_NETWORK: &str = "W2005";
/// Stable error reason, `WirekitError::InvalidConfig` (the caller's
/// configuration is unusable).
pub const CODE_INVALID_CONFIG: &str = "W2006";
/// Stable error reason, `WirekitError::Decode` (the response body did not
/// match the expected type).
pub const CODE_DECODE: &str = "W2007";

/// The error returned by every wirekit operation.
///
/// The enum is `#[non_exhaustive]`: a new variant in a later 0.x release is
/// additive, so consumers match with a wildcard arm. `Debug` and `Display`
/// never render a credential, a token, or a raw request or response body.
#[derive(thiserror::Error)]
#[non_exhaustive]
pub enum WirekitError {
    /// The provider answered with a non-success status, or reported an error
    /// in a successful response body.
    ///
    /// `status` is the HTTP status when one exists (200 for body-reported
    /// errors). `code` is the provider's error code when it provides one, and
    /// `message` is a short human-readable description. `details` retains the
    /// provider's structured error payload — SendGrid's `errors[]`, for
    /// example — and is never the raw response body of a success.
    #[error("W2001 API error: HTTP {status} ({code}) {message}")]
    Api {
        /// HTTP status, or 200 when the error was reported in the body.
        status: u16,
        /// Provider error code, or a wirekit-supplied fallback.
        code: String,
        /// Short human-readable description.
        message: String,
        /// The provider's structured error payload, when it supplied one.
        details: Option<serde_json::Value>,
    },

    /// Authentication was rejected, or the auth flow failed.
    #[error("W2002 authentication failed: {message}")]
    Auth {
        /// Short human-readable description of the failure.
        message: String,
    },

    /// The request was refused because a rate limit was reached and the retry
    /// policy gave up.
    #[error("W2003 rate limited: retry after {retry_after:?}")]
    RateLimit {
        /// The wait the provider asked for, when it supplied one.
        retry_after: Option<Duration>,
    },

    /// The request timed out.
    #[error("W2004 request timed out")]
    Timeout,

    /// The transport failed before a complete response was available.
    #[error("W2005 network error: {message}")]
    Network {
        /// Short, redacted description of the transport failure.
        message: String,
        /// The underlying cause, if one is available. Excluded from `Debug`.
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },

    /// The caller's configuration cannot produce a valid request.
    #[error("W2006 invalid configuration: {message}")]
    InvalidConfig {
        /// What is wrong and how to fix it.
        message: String,
    },

    /// The response body could not be decoded into the requested type.
    #[error("W2007 decode error: {message}")]
    Decode {
        /// Short description of the mismatch.
        message: String,
        /// The underlying cause, if one is available. Excluded from `Debug`.
        #[source]
        source: Option<Box<dyn Error + Send + Sync>>,
    },
}

impl WirekitError {
    /// The stable reason code of this error, always `W` followed by four
    /// digits.
    ///
    /// ```
    /// # use wirekit::WirekitError;
    /// let error = WirekitError::Timeout;
    /// assert_eq!(error.code(), "W2004");
    /// ```
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Api { .. } => CODE_API,
            Self::Auth { .. } => CODE_AUTH,
            Self::RateLimit { .. } => CODE_RATE_LIMIT,
            Self::Timeout => CODE_TIMEOUT,
            Self::Network { .. } => CODE_NETWORK,
            Self::InvalidConfig { .. } => CODE_INVALID_CONFIG,
            Self::Decode { .. } => CODE_DECODE,
        }
    }

    /// Whether the shared retry loop may attempt this error again.
    ///
    /// `Api` is retryable only for 429 and 5xx. Every other 4xx is final
    /// (CON-208). `RateLimit`, `Timeout`, and `Network` are retryable.
    #[must_use]
    pub fn is_retryable(&self) -> bool {
        match self {
            Self::Api { status, .. } => *status == 429 || (500..=599).contains(status),
            Self::RateLimit { .. } | Self::Timeout | Self::Network { .. } => true,
            Self::Auth { .. } | Self::InvalidConfig { .. } | Self::Decode { .. } => false,
        }
    }

    /// Whether this error means the caller's credential was rejected.
    #[must_use]
    pub fn is_auth(&self) -> bool {
        matches!(self, Self::Auth { .. })
    }

    /// The provider error code carried by an `Api` error, if any.
    #[must_use]
    pub fn api_code(&self) -> Option<&str> {
        match self {
            Self::Api { code, .. } => Some(code.as_str()),
            _ => None,
        }
    }

    /// The `Retry-After` wait carried by a `RateLimit` or `Api` error, if any.
    #[must_use]
    pub fn retry_after(&self) -> Option<Duration> {
        match self {
            Self::RateLimit { retry_after } => *retry_after,
            _ => None,
        }
    }

    /// Build an `Api` error.
    pub fn api(status: u16, code: impl Into<String>, message: impl Into<String>) -> Self {
        Self::Api {
            status,
            code: code.into(),
            message: message.into(),
            details: None,
        }
    }

    /// Build an `Api` error that retains the provider's structured payload.
    pub fn api_with_details(
        status: u16,
        code: impl Into<String>,
        message: impl Into<String>,
        details: serde_json::Value,
    ) -> Self {
        Self::Api {
            status,
            code: code.into(),
            message: message.into(),
            details: Some(details),
        }
    }

    /// The provider's structured error payload, when an `Api` error carries
    /// one.
    #[must_use]
    pub fn details(&self) -> Option<&serde_json::Value> {
        match self {
            Self::Api { details, .. } => details.as_ref(),
            _ => None,
        }
    }

    /// Build an `Auth` error.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth {
            message: message.into(),
        }
    }

    /// Build an `InvalidConfig` error.
    pub fn invalid_config(message: impl Into<String>) -> Self {
        Self::InvalidConfig {
            message: message.into(),
        }
    }

    /// Build a `Decode` error without a cause.
    pub fn decode(message: impl Into<String>) -> Self {
        Self::Decode {
            message: message.into(),
            source: None,
        }
    }

    /// Build a `Decode` error with a cause.
    pub fn decode_with(
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Decode {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Build a `Network` error with a cause.
    ///
    /// The message must already be sanitized: callers pass a summary, never a
    /// `reqwest::Error` rendered with its URL, because a URL may carry an
    /// API key in the query string (CON-214).
    pub fn network_with(
        message: impl Into<String>,
        source: impl Error + Send + Sync + 'static,
    ) -> Self {
        Self::Network {
            message: message.into(),
            source: Some(Box::new(source)),
        }
    }

    /// Build a `Network` error without a cause.
    pub fn network(message: impl Into<String>) -> Self {
        Self::Network {
            message: message.into(),
            source: None,
        }
    }

    /// Map a status plus a decoded provider body into an `Api` error.
    ///
    /// The body is inspected for the common shapes (`code` + `message`,
    /// `error`, `message`, or `errors[]`). The parsed value is retained in
    /// `details` so a provider can surface its own error list; the raw bytes
    /// are not kept. An unrecognised body still produces an `Api` error, so an
    /// unknown shape is never mistaken for success (EC-012, RISK-012).
    #[must_use]
    pub fn api_from_body(status: u16, body: &[u8]) -> Self {
        let value: serde_json::Value = match serde_json::from_slice(body) {
            Ok(value) => value,
            Err(_) => {
                return Self::api(status, format!("http_{status}"), "unparseable error body");
            }
        };

        let code = value
            .get("code")
            .map(value_to_string)
            .or_else(|| value.get("error").map(value_to_string))
            .or_else(|| {
                value
                    .get("errors")
                    .and_then(serde_json::Value::as_array)
                    .and_then(|errors| errors.first())
                    .map(value_to_string)
            })
            .unwrap_or_else(|| format!("http_{status}"));

        let message = value
            .get("message")
            .map(value_to_string)
            .or_else(|| value.get("detail").map(value_to_string))
            .or_else(|| value.get("title").map(value_to_string))
            .or_else(|| value.get("error_description").map(value_to_string))
            .unwrap_or_else(|| "provider reported a failure".to_string());

        Self::api_with_details(status, code, message, value)
    }
}

fn value_to_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::String(text) => text.clone(),
        serde_json::Value::Null => String::new(),
        other => other.to_string(),
    }
}

impl fmt::Debug for WirekitError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Wrapped causes are deliberately omitted: a transport cause can
        // render a URL, and a URL can carry a query-string API key.
        match self {
            Self::Api {
                status,
                code,
                message,
                details,
            } => formatter
                .debug_struct("Api")
                .field("status", status)
                .field("code", code)
                .field("message", message)
                .field("details", details)
                .finish(),
            Self::Auth { message } => formatter
                .debug_struct("Auth")
                .field("message", message)
                .finish(),
            Self::RateLimit { retry_after } => formatter
                .debug_struct("RateLimit")
                .field("retry_after", retry_after)
                .finish(),
            Self::Timeout => formatter.write_str("Timeout"),
            Self::Network { message, .. } => formatter
                .debug_struct("Network")
                .field("message", message)
                .finish(),
            Self::InvalidConfig { message } => formatter
                .debug_struct("InvalidConfig")
                .field("message", message)
                .finish(),
            Self::Decode { message, .. } => formatter
                .debug_struct("Decode")
                .field("message", message)
                .finish(),
        }
    }
}

/// Convenience alias for results returned by wirekit.
pub type WirekitResult<T> = Result<T, WirekitError>;
