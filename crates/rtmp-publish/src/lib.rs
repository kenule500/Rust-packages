//! Publishing destinations without a wire protocol.
//!
//! `rtmp-publish` models an RTMP, RTMPS, SRT, or WHIP destination, validates it,
//! runs a shallow health probe, plans bounded retries, breaks circuits, fails
//! over to a backup, and produces bounded bitrate hints.
//!
//! # Redaction
//!
//! The stream key is a [`SecretString`]. It has no `Serialize` implementation
//! and `Debug`/`Display` print `***`. Error values never carry the key or the
//! query string. [`RedactedDestination`] is the only serialisable view.
//!
//! # Scope
//!
//! - Destination shape validation before any connection.
//! - TCP probes for RTMP/RTMPS/SRT and an optional HTTP probe for WHIP.
//! - Bounded retry with injected jitter and a per-destination breaker.
//! - Backup promotion and cool-down demotion.
//! - Bounded bitrate hints.
//!
//! # Non-goals
//!
//! - RTMP, SRT, or WHIP wire protocols, handshakes, or publishing.
//! - Muxing, encoding, transcoding, or CDN orchestration.
//! - Persisting keys or reading `.env` files. Keys are caller-supplied and stay
//!   in memory only.
//! - HSTS. TLS is enforced for credential-bearing schemes, but HSTS is the
//!   deployment edge's responsibility.
//!
//! # Features
//!
//! - `serde`: derives `Serialize` for [`RedactedDestination`].
//! - `whip`: enables the `reqwest`-based HTTP probe for WHIP.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

mod bitrate;
mod breaker;
mod destination;
mod error;
mod failover;
mod probe;
mod retry;
mod secret;
mod validate;

pub use bitrate::BitrateHint;
pub use breaker::{BreakerConfig, BreakerSet, BreakerState, CircuitBreaker};
pub use destination::{Destination, DestinationId, Protocol, RedactedDestination};
pub use error::PublishError;
pub use failover::{Failover, FailoverDecision, FailoverPhase};
pub use probe::{probe, Connector, FailureKind, Health, ProbeConfig, TcpConnector};
pub use retry::{RetryPlan, RetryPolicy, MAX_RETRIES};
pub use secret::SecretString;
pub use validate::{validate, MAX_BACKUP_DEPTH};

#[cfg(feature = "whip")]
pub use probe::{whip_probe, HttpProbe, ReqwestProbe};
