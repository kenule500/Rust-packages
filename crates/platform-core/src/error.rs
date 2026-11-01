use std::fmt;

use crate::ProviderId;

/// A stable, machine-readable error category.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ErrorCode {
    /// The provider rejected or is missing credentials.
    Auth,
    /// The credentials do not grant the required scope.
    Scope,
    /// The provider's rate limit was reached.
    RateLimit,
    /// The tenant's quota does not allow the call.
    Quota,
    /// The provider returned an application error.
    Provider,
    /// The transport failed before a provider response was received.
    Transport,
    /// The provider does not support the requested operation.
    Unsupported,
    /// A provider identifier was registered twice.
    DuplicateProvider,
    /// The integration configuration is invalid.
    Config,
}

impl ErrorCode {
    /// Returns the stable display code for this category.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Auth => "PC-AUTH",
            Self::Scope => "PC-SCOPE",
            Self::RateLimit => "PC-RATE-LIMIT",
            Self::Quota => "PC-QUOTA",
            Self::Provider => "PC-PROVIDER",
            Self::Transport => "PC-TRANSPORT",
            Self::Unsupported => "PC-UNSUPPORTED",
            Self::DuplicateProvider => "PC-DUPLICATE-PROVIDER",
            Self::Config => "PC-CONFIG",
        }
    }
}

impl fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The error type shared by `platform-core` adapters.
///
/// Every variant maps to a stable [`ErrorCode`]. `Display` always renders
/// `CODE: message`, so logs are searchable without parsing free-form text.
#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum IntegrationError {
    /// Authentication failed.
    Auth {
        /// A non-secret description of the failure.
        message: String,
    },
    /// The credential lacks a required scope.
    Scope {
        /// A non-secret description of the missing scope.
        message: String,
    },
    /// The provider rate limit was reached.
    RateLimit {
        /// A non-secret description of the limit.
        message: String,
        /// The provider's suggested retry delay, when supplied.
        retry_after_secs: Option<u64>,
    },
    /// The tenant quota does not allow the call.
    Quota {
        /// A non-secret description of the quota decision.
        message: String,
    },
    /// The provider returned an application error.
    Provider {
        /// A non-secret description of the provider error.
        message: String,
    },
    /// The transport failed.
    Transport {
        /// A non-secret description of the transport failure.
        message: String,
    },
    /// The provider does not support the operation.
    Unsupported {
        /// A non-secret description of the unsupported operation.
        message: String,
    },
    /// A provider identifier was registered twice.
    DuplicateProvider {
        /// The duplicated provider identifier.
        provider: String,
    },
    /// The integration configuration is invalid.
    Config {
        /// A non-secret description of the configuration problem.
        message: String,
    },
}

impl IntegrationError {
    /// Returns the stable code for this error.
    pub fn code(&self) -> ErrorCode {
        match self {
            Self::Auth { .. } => ErrorCode::Auth,
            Self::Scope { .. } => ErrorCode::Scope,
            Self::RateLimit { .. } => ErrorCode::RateLimit,
            Self::Quota { .. } => ErrorCode::Quota,
            Self::Provider { .. } => ErrorCode::Provider,
            Self::Transport { .. } => ErrorCode::Transport,
            Self::Unsupported { .. } => ErrorCode::Unsupported,
            Self::DuplicateProvider { .. } => ErrorCode::DuplicateProvider,
            Self::Config { .. } => ErrorCode::Config,
        }
    }

    /// Creates an authentication error.
    pub fn auth(message: impl Into<String>) -> Self {
        Self::Auth {
            message: message.into(),
        }
    }

    /// Creates a scope error.
    pub fn scope(message: impl Into<String>) -> Self {
        Self::Scope {
            message: message.into(),
        }
    }

    /// Creates a rate-limit error.
    pub fn rate_limit(message: impl Into<String>, retry_after_secs: Option<u64>) -> Self {
        Self::RateLimit {
            message: message.into(),
            retry_after_secs,
        }
    }

    /// Creates a quota error.
    pub fn quota(message: impl Into<String>) -> Self {
        Self::Quota {
            message: message.into(),
        }
    }

    /// Creates a provider error.
    pub fn provider(message: impl Into<String>) -> Self {
        Self::Provider {
            message: message.into(),
        }
    }

    /// Creates a transport error.
    pub fn transport(message: impl Into<String>) -> Self {
        Self::Transport {
            message: message.into(),
        }
    }

    /// Creates an unsupported-operation error.
    pub fn unsupported(message: impl Into<String>) -> Self {
        Self::Unsupported {
            message: message.into(),
        }
    }

    /// Creates a duplicate-provider error for the given identifier.
    pub fn duplicate_provider(provider: ProviderId) -> Self {
        Self::DuplicateProvider {
            provider: provider.as_str().to_owned(),
        }
    }

    /// Creates a configuration error.
    pub fn config(message: impl Into<String>) -> Self {
        Self::Config {
            message: message.into(),
        }
    }
}

impl fmt::Display for IntegrationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let code = self.code().as_str();
        match self {
            Self::Auth { message }
            | Self::Scope { message }
            | Self::Quota { message }
            | Self::Provider { message }
            | Self::Transport { message }
            | Self::Unsupported { message }
            | Self::Config { message } => write!(f, "{code}: {message}"),
            Self::RateLimit { message, .. } => write!(f, "{code}: {message}"),
            Self::DuplicateProvider { provider } => {
                write!(f, "{code}: provider `{provider}` is already registered")
            }
        }
    }
}

impl std::error::Error for IntegrationError {}
