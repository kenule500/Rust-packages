//! Strict YAML-1.2-core parsing.
//!
//! Scope is deliberately narrow. Supported: block mappings and sequences,
//! single-line flow collections, plain scalars, single- and double-quoted
//! scalars, block scalars (`|`, `>` with chomping), comments, and multi-line
//! plain scalars.
//!
//! Rejected with a positioned error: tags, directives, anchors, aliases,
//! merge keys, multi-document streams, and tabs used for indentation.
//!
//! Booleans follow YAML 1.2 core: only `true` and `false`. `yes`, `no`, `on`,
//! and `off` are strings.

/// A parse or deserialization failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Error {
    /// Stable diagnostic code, e.g. `D1001`.
    pub code: &'static str,
    /// Human-readable message.
    pub message: String,
    /// 1-based line where the failure occurred.
    pub line: usize,
    /// 1-based column where the failure occurred.
    pub column: usize,
}

impl core::fmt::Display for Error {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}: {} ({}:{})", self.code, self.message, self.line, self.column)
    }
}

impl core::error::Error for Error {}

/// Parse a YAML document into `T`.
///
/// Scaffold: the parser lands in phase 1. Until then this reports
/// `D0001` rather than guessing at a partial document.
pub fn from_str<T>(_src: &str) -> Result<T, Error> {
    Err(Error {
        code: "D0001",
        message: "the YAML parser is not implemented yet".to_owned(),
        line: 1,
        column: 1,
    })
}
