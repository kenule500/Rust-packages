//! Output descriptors as data, with shape validation and key redaction.

use std::fmt;

use crate::error::CompileError;

/// Where a compiled graph will be sent.
///
/// The variants describe intent only. Nothing in this module resolves a host,
/// opens a socket, or starts a process (FR-005, FR-032).
#[derive(Clone, PartialEq, Eq)]
pub enum Output {
    /// Write to a local file path.
    File {
        /// Destination path.
        path: String,
    },
    /// Publish over RTMP or RTMPS.
    Rtmp {
        /// Server URL.
        url: String,
        /// Stream key; never printed by `Debug` or `Display`.
        stream_key: String,
    },
    /// Publish over SRT.
    Srt {
        /// Caller URL.
        url: String,
    },
    /// Publish to a WHIP endpoint over HTTPS.
    Whip {
        /// Endpoint URL.
        url: String,
    },
}

impl Output {
    /// Validates the descriptor shape before any connection attempt.
    pub fn validate(&self) -> Result<(), CompileError> {
        match self {
            Self::File { path } => {
                if path.trim().is_empty() || path.contains('\0') {
                    return Err(CompileError::InvalidOutput {
                        detail: "file path must be non-empty".to_string(),
                    });
                }
            }
            Self::Rtmp { url, stream_key } => {
                validate_url(url, &["rtmp://", "rtmps://"], "rtmp url")?;
                validate_secret(stream_key, "stream key")?;
            }
            Self::Srt { url } => {
                validate_url(url, &["srt://"], "srt url")?;
            }
            Self::Whip { url } => {
                validate_url(url, &["https://", "whip+https://"], "whip url")?;
            }
        }
        Ok(())
    }

    /// Returns the FFmpeg arguments for this descriptor.
    ///
    /// Stream keys are never placed in the argument vector; the caller supplies
    /// them through its transport layer.
    #[must_use]
    pub fn args(&self) -> Vec<String> {
        match self {
            Self::File { path } => vec![path.clone()],
            Self::Rtmp { url, .. } => {
                vec!["-f".to_string(), "flv".to_string(), url.clone()]
            }
            Self::Srt { url } => vec!["-f".to_string(), "mpegts".to_string(), url.clone()],
            Self::Whip { url } => vec![url.clone()],
        }
    }
}

impl fmt::Debug for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path } => f.debug_struct("File").field("path", path).finish(),
            Self::Rtmp { url, .. } => f
                .debug_struct("Rtmp")
                .field("url", url)
                .field("stream_key", &"***")
                .finish(),
            Self::Srt { url } => f.debug_struct("Srt").field("url", url).finish(),
            Self::Whip { url } => f.debug_struct("Whip").field("url", url).finish(),
        }
    }
}

impl fmt::Display for Output {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::File { path } => write!(f, "file:{path}"),
            Self::Rtmp { url, .. } => write!(f, "rtmp:{url} key=***"),
            Self::Srt { url } => write!(f, "srt:{url}"),
            Self::Whip { url } => write!(f, "whip:{url}"),
        }
    }
}

fn validate_url(url: &str, schemes: &[&str], label: &str) -> Result<(), CompileError> {
    if url.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(CompileError::InvalidOutput {
            detail: format!("{label} must not contain whitespace or control characters"),
        });
    }
    let Some(scheme) = schemes.iter().find(|scheme| url.starts_with(**scheme)) else {
        return Err(CompileError::InvalidOutput {
            detail: format!("{label} must start with one of {}", schemes.join(", ")),
        });
    };
    let rest = &url[scheme.len()..];
    let authority = rest.split('/').next().unwrap_or_default();
    let host = authority.split(':').next().unwrap_or_default();
    if host.is_empty() {
        return Err(CompileError::InvalidOutput {
            detail: format!("{label} must include a host"),
        });
    }
    if let Some((_, port)) = authority.rsplit_once(':') {
        if !port.is_empty() && port.parse::<u16>().is_err() {
            return Err(CompileError::InvalidOutput {
                detail: format!("{label} has an invalid port"),
            });
        }
    }
    Ok(())
}

fn validate_secret(value: &str, label: &str) -> Result<(), CompileError> {
    if value.trim().is_empty() || value.chars().any(char::is_control) {
        return Err(CompileError::InvalidOutput {
            detail: format!("{label} must be non-empty and free of control characters"),
        });
    }
    Ok(())
}
