//! Tracing events for the request lifecycle.
//!
//! Scope: started, finished (with duration), retry, rate-limit hit, and error
//! events, each carrying only redacted data (CON-211, NFR-007).
//!
//! Non-goals: subscriber setup, span propagation across application code, and
//! exporting metrics. The caller installs a `tracing` subscriber once.

use std::time::Duration;

use http::Method;
use url::Url;

use crate::error::WirekitError;

const TARGET: &str = "wirekit::request";

/// Render a URL with the query string reduced to its parameter names.
///
/// An API key in the query string must never reach a log, so values are
/// replaced with `***` and the fragment is dropped (CON-214).
///
/// ```
/// # use url::Url;
/// let url = Url::parse("https://api.example.com/v1/items?api_key=secret").unwrap();
/// assert_eq!(wirekit::trace::redact_url(&url), "https://api.example.com/v1/items?api_key=***");
/// ```
#[must_use]
pub fn redact_url(url: &Url) -> String {
    let mut rendered = format!("{}://{}", url.scheme(), url.host_str().unwrap_or_default());
    if let Some(port) = url.port() {
        rendered.push(':');
        rendered.push_str(&port.to_string());
    }
    rendered.push_str(url.path());

    let mut pairs = url.query_pairs().peekable();
    if pairs.peek().is_some() {
        rendered.push('?');
        let names = pairs
            .map(|(name, _)| format!("{name}=***"))
            .collect::<Vec<_>>()
            .join("&");
        rendered.push_str(&names);
    }
    rendered
}

pub(crate) fn request_started(method: &Method, url: &Url) {
    tracing::debug!(
        target: TARGET,
        method = %method,
        url = %redact_url(url),
        "request started"
    );
}

pub(crate) fn request_finished(method: &Method, status: u16, duration: Duration) {
    tracing::debug!(
        target: TARGET,
        method = %method,
        status,
        duration_ms = duration.as_millis() as u64,
        "request finished"
    );
}

pub(crate) fn retry(attempt: u32, delay: Duration, error: &WirekitError) {
    tracing::warn!(
        target: TARGET,
        attempt,
        delay_ms = delay.as_millis() as u64,
        code = error.code(),
        "retrying request"
    );
}

pub(crate) fn request_failed(error: &WirekitError) {
    tracing::warn!(
        target: TARGET,
        code = error.code(),
        "request failed"
    );
}
