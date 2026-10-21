//! Destination shape validation with the TLS policy.
//!
//! Validation is pure and runs before any probe or connection attempt. It
//! checks the scheme, host, port, key presence, and the backup chain, and
//! rejects credentials in the URL.

use crate::destination::{Destination, Protocol};
use crate::error::PublishError;

/// Maximum number of destinations in one backup chain, including the primary.
pub const MAX_BACKUP_DEPTH: usize = 4;

const SENSITIVE_QUERY_NAMES: [&str; 8] = [
    "key",
    "token",
    "secret",
    "password",
    "passphrase",
    "stream_key",
    "streamkey",
    "auth",
];

/// Validates a destination and its complete backup chain.
pub fn validate(destination: &Destination) -> Result<(), PublishError> {
    validate_one(destination)?;

    let mut seen: Vec<&str> = vec![destination.id().as_str()];
    let mut current = destination.backup();
    let mut depth = 1_usize;
    while let Some(backup) = current {
        depth += 1;
        if depth > MAX_BACKUP_DEPTH {
            return Err(PublishError::BackupDepth {
                max: MAX_BACKUP_DEPTH,
            });
        }
        if seen.contains(&backup.id().as_str()) {
            return Err(PublishError::BackupCycle {
                id: backup.id().to_string(),
            });
        }
        seen.push(backup.id().as_str());
        validate_one(backup)?;
        current = backup.backup();
    }
    Ok(())
}

fn validate_one(destination: &Destination) -> Result<(), PublishError> {
    let label = destination.label();
    if label.is_empty() || label.chars().count() > 128 {
        return Err(PublishError::InvalidLabel {
            detail: "label must be 1-128 characters".to_string(),
        });
    }
    if label.chars().any(char::is_control) {
        return Err(PublishError::InvalidLabel {
            detail: "label contains control characters".to_string(),
        });
    }

    let scheme = destination.url().scheme();
    validate_scheme(destination.protocol(), scheme)?;

    let Some(host) = destination.url().host_str() else {
        return Err(PublishError::MissingHost);
    };
    if host.is_empty() {
        return Err(PublishError::MissingHost);
    }

    if !destination.url().username().is_empty() || destination.url().password().is_some() {
        return Err(PublishError::CredentialsInUrl);
    }
    if sensitive_query(destination.url()) {
        return Err(PublishError::CredentialsInUrl);
    }
    if effective_port(destination.url(), scheme).is_none() {
        return Err(PublishError::MissingPort);
    }

    if destination.key().expose_secret().is_empty() {
        return Err(PublishError::MissingKey);
    }
    if let Some(region) = destination.region() {
        if !is_safe_identifier(region) {
            return Err(PublishError::InvalidId {
                detail: "region must be 1-64 ASCII alphanumerics, '-' or '_'".to_string(),
            });
        }
    }
    Ok(())
}

fn validate_scheme(protocol: Protocol, scheme: &str) -> Result<(), PublishError> {
    let matches = match protocol {
        Protocol::Rtmp => scheme == "rtmp",
        Protocol::Rtmps => scheme == "rtmps",
        Protocol::Srt => scheme == "srt",
        Protocol::Whip => scheme == "https" || scheme == "whip+https",
    };
    if matches {
        return Ok(());
    }
    let supported = matches!(scheme, "rtmp" | "rtmps" | "srt" | "https" | "whip+https");
    if supported {
        Err(PublishError::SchemeMismatch {
            protocol: protocol.as_str(),
            scheme: scheme.to_string(),
        })
    } else {
        Err(PublishError::UnsupportedScheme {
            scheme: scheme.to_string(),
        })
    }
}

/// Returns the effective port, applying the HTTPS default for WHIP schemes.
pub(crate) fn effective_port(url: &url::Url, scheme: &str) -> Option<u16> {
    url.port()
        .or_else(|| url.port_or_known_default())
        .or_else(|| (scheme == "whip+https").then_some(443))
}

fn sensitive_query(url: &url::Url) -> bool {
    url.query_pairs().any(|(name, _)| {
        let name = name.to_ascii_lowercase();
        SENSITIVE_QUERY_NAMES.contains(&name.as_str())
    })
}

/// Validates a path-safe identifier.
pub(crate) fn is_safe_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        && value
            .chars()
            .next()
            .is_some_and(|c| c.is_ascii_alphanumeric())
}

/// Returns a credential-free endpoint string: scheme, host, port, and path.
pub(crate) fn redacted_endpoint(url: &url::Url) -> String {
    let scheme = url.scheme();
    let host = url.host_str().unwrap_or("<no-host>");
    let port = effective_port(url, scheme)
        .map(|port| format!(":{port}"))
        .unwrap_or_default();
    let path = url.path();
    if path.is_empty() || path == "/" {
        format!("{scheme}://{host}{port}")
    } else {
        format!("{scheme}://{host}{port}{path}")
    }
}
