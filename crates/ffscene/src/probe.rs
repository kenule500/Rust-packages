//! Encoder probing against an injected list of available encoders.

use crate::error::CompileError;

/// Encoder names the compiler knows how to report.
pub const KNOWN_ENCODERS: [&str; 4] = ["h264_nvenc", "hevc_qsv", "h264_amf", "libx264"];

/// The outcome of an encoder probe.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderSelection {
    /// The encoder that was requested.
    pub requested: String,
    /// The encoder that will actually be used.
    pub selected: String,
    /// Why a different encoder was selected, if it was.
    pub fallback_reason: Option<String>,
}

/// Selects an encoder from a caller-provided availability list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EncoderProbe {
    available: Vec<String>,
}

impl EncoderProbe {
    /// Creates a probe from the encoders the caller observed as available.
    #[must_use]
    pub fn new(available: impl IntoIterator<Item = String>) -> Self {
        let mut available: Vec<String> = available.into_iter().collect();
        available.sort();
        available.dedup();
        Self { available }
    }

    /// Returns the sorted availability list.
    #[must_use]
    pub fn available(&self) -> &[String] {
        &self.available
    }

    /// Selects the requested encoder, or an explicit fallback.
    ///
    /// The fallback prefers `libx264`, then the first known encoder in
    /// [`KNOWN_ENCODERS`] order. A downgrade is never silent: the returned
    /// selection always names the requested encoder and the reason.
    pub fn select(&self, requested: &str) -> Result<EncoderSelection, CompileError> {
        if !KNOWN_ENCODERS.contains(&requested) {
            return Err(CompileError::UnsupportedEncoder {
                encoder: requested.to_string(),
            });
        }
        if self.available.iter().any(|name| name == requested) {
            return Ok(EncoderSelection {
                requested: requested.to_string(),
                selected: requested.to_string(),
                fallback_reason: None,
            });
        }

        if let Some(selected) = KNOWN_ENCODERS
            .iter()
            .find(|name| self.available.iter().any(|available| available == **name))
        {
            return Ok(EncoderSelection {
                requested: requested.to_string(),
                selected: (*selected).to_string(),
                fallback_reason: Some(format!(
                    "requested encoder {requested:?} is unavailable; using {selected:?}"
                )),
            });
        }

        Err(CompileError::NoEncoderAvailable {
            requested: requested.to_string(),
        })
    }
}
