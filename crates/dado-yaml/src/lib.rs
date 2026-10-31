//! dado-yaml — the base layer for config-driven tools.
//!
//! Five feature-gated modules, no I/O, no runtime, zero dependencies by
//! default:
//!
//! - `yaml` — strict YAML-1.2-core parser for config and spec files
//! - `diag` — diagnostics with stable codes, spans, and source rendering
//! - `graph` — dependency graph, topological order, cycle detection
//! - `emit` — indentation-aware code emitter for generators
//! - `glob` — glob compilation and matching
//!
//! The crate never opens a socket, never reads a URL, never spawns a
//! runtime, and never writes a file. Callers fetch; `dado-yaml` parses.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

#[cfg(feature = "yaml")]
pub mod yaml;

#[cfg(feature = "diag")]
pub mod diag;

#[cfg(feature = "graph")]
pub mod graph;

#[cfg(feature = "emit")]
pub mod emit;

#[cfg(feature = "glob")]
pub mod glob;

/// The crate version, for build info and diagnostics headers.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
