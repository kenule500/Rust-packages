//! The request pipeline types.
//!
//! Scope: the [`RequestSpec`] a provider builds, the [`Response`] it receives,
//! and the body-level error hook that keeps HTTP 200 from being mistaken for
//! success (CON-213).
//!
//! Non-goals: the execution loop itself (that is
//! [`WirekitClient::execute`](crate::WirekitClient::execute)), provider request
//! models, and any provider-specific error type.

use std::sync::Arc;

use http::{HeaderMap, HeaderName, HeaderValue, Method};
use serde::de::DeserializeOwned;
use serde::Serialize;
use url::Url;

use crate::auth::AuthScheme;
use crate::error::WirekitError;

/// Inspects a successful response body for an error the HTTP status does not
/// report.
///
/// Returns `Some(error)` when the body reports a failure, so the pipeline
/// never treats HTTP 200 as success on its own (CON-213, FR-022).
pub type BodyErrorHook = fn(status: u16, body: &[u8]) -> Option<WirekitError>;

/// One outbound request, described by a provider and executed by the shared
/// pipeline.
///
/// A provider supplies the method, URL, headers, body, and — when the API
/// reports failures in a 200 body — a [`BodyErrorHook`]. Auth, rate limiting,
/// retry, tracing, and decoding are the pipeline's job (FR-021).
///
/// Decoding is deliberately not a field here: the plan's `response_type` hook
/// would add a generic parameter to every provider type, so
/// [`WirekitClient::execute`](crate::WirekitClient::execute) applies `T`'s
/// `Deserialize` implementation instead, in one place for every provider.
pub struct RequestSpec {
    method: Method,
    url: Url,
    headers: HeaderMap,
    body: Option<Vec<u8>>,
    auth: Option<Arc<dyn AuthScheme>>,
    rate_limit_key: Option<String>,
    non_retryable: Vec<String>,
    body_error_hook: Option<BodyErrorHook>,
    max_response_bytes: Option<usize>,
}

impl RequestSpec {
    /// Build a request for `method` and an absolute `url`.
    pub fn new(method: Method, url: Url) -> Self {
        Self {
            method,
            url,
            headers: HeaderMap::new(),
            body: None,
            auth: None,
            rate_limit_key: None,
            non_retryable: Vec::new(),
            body_error_hook: None,
            max_response_bytes: None,
        }
    }

    /// Build a `GET` request.
    pub fn get(url: Url) -> Self {
        Self::new(Method::GET, url)
    }

    /// Build a `POST` request.
    pub fn post(url: Url) -> Self {
        Self::new(Method::POST, url)
    }

    /// Build a `DELETE` request.
    pub fn delete(url: Url) -> Self {
        Self::new(Method::DELETE, url)
    }

    /// Attach the scheme that prepares this request.
    #[must_use]
    pub fn auth(mut self, auth: Arc<dyn AuthScheme>) -> Self {
        self.auth = Some(auth);
        self
    }

    /// Charge this request to a rate-limit bucket.
    ///
    /// Keys are matched exactly first, then by the segment before `:` so that
    /// `twilio:AC123` inherits the `twilio` override.
    #[must_use]
    pub fn rate_limit_key(mut self, key: impl Into<String>) -> Self {
        self.rate_limit_key = Some(key.into());
        self
    }

    /// Declare body-level error codes that must never be retried.
    #[must_use]
    pub fn non_retryable(mut self, codes: &[&str]) -> Self {
        self.non_retryable = codes.iter().map(|code| (*code).to_string()).collect();
        self
    }

    /// Install a hook that inspects a 2xx body for a reported error.
    #[must_use]
    pub fn body_error_hook(mut self, hook: BodyErrorHook) -> Self {
        self.body_error_hook = Some(hook);
        self
    }

    /// Override the client's response size bound for this request.
    #[must_use]
    pub fn max_response_bytes(mut self, limit: usize) -> Self {
        self.max_response_bytes = Some(limit);
        self
    }

    /// Add a header.
    pub fn header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.headers.insert(name, value);
        self
    }

    /// Add a header from string slices, rejecting unusable values.
    pub fn try_header(mut self, name: &str, value: &str) -> Result<Self, WirekitError> {
        let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
            WirekitError::invalid_config(format!("{name} is not a valid HTTP header name"))
        })?;
        let value = HeaderValue::from_str(value).map_err(|_| {
            WirekitError::invalid_config(format!(
                "the value for {name} is not a valid HTTP header value"
            ))
        })?;
        self.headers.insert(name, value);
        Ok(self)
    }

    /// Set a raw byte body.
    #[must_use]
    pub fn bytes_body(mut self, body: Vec<u8>) -> Self {
        self.body = Some(body);
        self
    }

    /// Serialize `value` as JSON and use it as the body.
    pub fn json_body<T: Serialize>(mut self, value: &T) -> Result<Self, WirekitError> {
        let encoded = serde_json::to_vec(value).map_err(|error| {
            WirekitError::invalid_config(format!("the request body could not be encoded: {error}"))
        })?;
        self.headers.insert(
            http::header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        );
        self.body = Some(encoded);
        Ok(self)
    }

    /// The HTTP method.
    #[must_use]
    pub fn method(&self) -> &Method {
        &self.method
    }

    /// The absolute URL.
    #[must_use]
    pub fn url(&self) -> &Url {
        &self.url
    }

    /// Mutable access to the URL, for a scheme that adds a query parameter.
    pub fn url_mut(&mut self) -> &mut Url {
        &mut self.url
    }

    /// The raw body bytes exactly as they will be sent.
    #[must_use]
    pub fn body(&self) -> Option<&[u8]> {
        self.body.as_deref()
    }

    /// The headers applied so far.
    #[must_use]
    pub fn headers(&self) -> &HeaderMap {
        &self.headers
    }

    /// Insert or replace a header.
    pub fn insert_header(&mut self, name: HeaderName, value: HeaderValue) {
        self.headers.insert(name, value);
    }

    pub(crate) fn auth_scheme(&self) -> Option<&Arc<dyn AuthScheme>> {
        self.auth.as_ref()
    }

    pub(crate) fn rate_limit_key_ref(&self) -> Option<&str> {
        self.rate_limit_key.as_deref()
    }

    pub(crate) fn non_retryable_ref(&self) -> &[String] {
        &self.non_retryable
    }

    pub(crate) fn hook(&self) -> Option<BodyErrorHook> {
        self.body_error_hook
    }

    pub(crate) fn response_limit(&self) -> Option<usize> {
        self.max_response_bytes
    }
}

/// A decoded provider response.
#[derive(Debug)]
pub struct Response<T> {
    /// The HTTP status.
    pub status: u16,
    /// The response headers, retained for values such as `X-Message-Id`.
    pub headers: HeaderMap,
    /// The decoded body.
    pub value: T,
}

impl<T> Response<T> {
    /// The first value of `name` as UTF-8, if present.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }

    /// Borrow the decoded value.
    #[must_use]
    pub fn value(&self) -> &T {
        &self.value
    }

    /// Consume the response and return the decoded value.
    #[must_use]
    pub fn into_value(self) -> T {
        self.value
    }
}

/// Decode `bytes` into `T`, mapping a mismatch to [`WirekitError::Decode`].
pub(crate) fn decode_body<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, WirekitError> {
    let payload: &[u8] = if bytes.is_empty() { b"null" } else { bytes };
    serde_json::from_slice(payload).map_err(|error| {
        WirekitError::decode_with("the response body did not match the expected type", error)
    })
}
