//! wirekit — one HTTP client for many providers.
//!
//! wirekit exists so that every provider integration in a Rust application
//! shares one error type, one retry policy, one rate limiter, one HTTP
//! connection pool, one auth layer, one tracing story, and one request
//! pipeline. A provider module supplies a URL, a method, a body, and a
//! response type; everything else already exists.
//!
//! # What is always compiled
//!
//! The core is **not** feature-gated. `default = []` means "no provider
//! module enabled", not "no core":
//!
//! - [`WirekitError`] — the single error type, with stable `W####` codes
//! - [`Secret`] — a wrapper whose `Debug`/`Display` render `***`
//! - [`WirekitClient`] — the shared pooled transport, pipeline, and decoder
//! - [`RetryPolicy`] — exponential backoff with jitter and `Retry-After`
//! - [`RateLimitConfig`] — the per-provider token bucket
//! - [`AuthScheme`] — Bearer, Basic, API key (header and query), HMAC, and
//!   OAuth2 client credentials
//!
//! # Provider modules
//!
//! | Feature | Module | Auth | Base URL |
//! |---|---|---|---|
//! | `noaa` | `wirekit::noaa` | none (descriptive `User-Agent`) | `https://api.weather.gov` |
//! | `twilio` | `wirekit::twilio` | Basic or API key | `https://api.twilio.com/2010-04-01` |
//! | `sendgrid` | `wirekit::sendgrid` | Bearer API key | `https://api.sendgrid.com/v3` |
//! | `slack` | `wirekit::slack` | Bot token | `https://slack.com/api` |
//!
//! Features are additive (CON-204): enabling one provider never enables or
//! disables another. Enabling all four costs one crate.
//!
//! # I/O boundary
//!
//! The crate performs outbound HTTPS requests and nothing else. It never opens
//! a listening socket, never reads a `.env` file or an environment variable,
//! never touches a database, and never persists state. In-memory state — the
//! connection pool, the token buckets, the OAuth2 token cache, and the
//! optional NOAA cache — belongs to the caller's process and is dropped with
//! it.
//!
//! # Example
//!
//! ```no_run
//! use wirekit::{RetryPolicy, WirekitClient};
//!
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! let client = WirekitClient::builder("https://api.example.com")?
//!     .retry(RetryPolicy::default())
//!     .build()?;
//!
//! let response = client.get_json::<serde_json::Value>("/v1/status").await?;
//! println!("{}", response.status);
//! # Ok(())
//! # }
//! ```

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod auth;
pub mod client;
pub mod error;
pub mod pipeline;
pub mod rate_limit;
pub mod retry;
pub mod secret;
pub mod trace;

#[cfg(feature = "angi")]
pub mod angi;

#[cfg(feature = "google-lsa")]
pub mod google_lsa;

#[cfg(feature = "noaa")]
pub mod noaa;

#[cfg(feature = "sage-intacct")]
pub mod sage_intacct;

#[cfg(feature = "sendgrid")]
pub mod sendgrid;

#[cfg(feature = "slack")]
pub mod slack;

#[cfg(feature = "twilio")]
pub mod twilio;

pub use auth::{
    ApiKeyHeader, ApiKeyQuery, AuthScheme, BasicAuth, BearerAuth, HmacAuth, OAuth2ClientCredentials,
};
pub use client::{RawResponse, WirekitClient, WirekitClientBuilder};
pub use error::WirekitError;
pub use pipeline::{BodyErrorHook, RequestSpec, Response};
pub use rate_limit::RateLimitConfig;
pub use retry::{JitterBounds, RetryPolicy};
pub use secret::Secret;

/// The crate version, for build info and `User-Agent` strings.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");
