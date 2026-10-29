//! The shared client: one transport, one retry policy, one set of buckets,
//! one pipeline.
//!
//! Scope: the pooled `reqwest::Client` (CON-210), the builder that configures
//! it, the per-provider rate limiter, the retry loop, bounded body reads, and
//! typed decoding.
//!
//! Non-goals: provider request models, auth-scheme storage (providers attach
//! their own scheme per request), and a global client singleton (CON-109). One
//! `WirekitClient` is cloned cheaply and handed to every provider the caller
//! builds, which is how they share one connection pool.

use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use http::{HeaderMap, HeaderName, HeaderValue, Method};
use serde::de::DeserializeOwned;
use url::Url;

use crate::auth::AuthContext;
use crate::error::WirekitError;
use crate::pipeline::{decode_body, RequestSpec, Response};
use crate::rate_limit::{RateLimitConfig, RateLimiter};
use crate::retry::{parse_retry_after, should_retry, RetryPolicy};
use crate::trace;

/// The default per-request timeout.
pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);
/// The default connect timeout.
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
/// The default idle connections kept per host.
pub const DEFAULT_POOL_MAX_IDLE_PER_HOST: usize = 8;
/// The default response size bound, 8 MiB.
pub const DEFAULT_MAX_RESPONSE_BYTES: usize = 8 * 1024 * 1024;

struct ClientInner {
    http: reqwest::Client,
    base_url: Url,
    retry: RetryPolicy,
    rate_limiter: RateLimiter,
    max_response_bytes: usize,
    allow_insecure_localhost: bool,
}

/// The shared transport and pipeline.
///
/// Cloning is cheap: every clone points at the same connection pool, retry
/// policy, and token buckets, so two providers built from one client share one
/// pool (AC-016).
#[derive(Clone)]
pub struct WirekitClient {
    inner: Arc<ClientInner>,
}

impl std::fmt::Debug for WirekitClient {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WirekitClient")
            .field("base_url", &self.inner.base_url)
            .field("retry", &self.inner.retry)
            .field("max_response_bytes", &self.inner.max_response_bytes)
            .finish_non_exhaustive()
    }
}

impl WirekitClient {
    /// Start building a client for `base_url`.
    ///
    /// The base URL must use HTTPS (NFR-015).
    pub fn builder(base_url: impl AsRef<str>) -> Result<WirekitClientBuilder, WirekitError> {
        let base_url = Url::parse(base_url.as_ref()).map_err(|error| {
            WirekitError::invalid_config(format!("the base URL is not a URL: {error}"))
        })?;
        Ok(WirekitClientBuilder {
            base_url,
            injected: None,
            timeout: Some(DEFAULT_TIMEOUT),
            connect_timeout: Some(DEFAULT_CONNECT_TIMEOUT),
            pool_max_idle_per_host: Some(DEFAULT_POOL_MAX_IDLE_PER_HOST),
            proxy: None,
            default_headers: HeaderMap::new(),
            retry: RetryPolicy::default(),
            rate_limit_default: None,
            rate_limit_overrides: HashMap::new(),
            max_response_bytes: DEFAULT_MAX_RESPONSE_BYTES,
            allow_insecure_localhost: false,
        })
    }

    /// Build a client with the default configuration.
    pub fn new(base_url: impl AsRef<str>) -> Result<Self, WirekitError> {
        Self::builder(base_url)?.build()
    }

    /// The pooled transport, for a scheme or provider that must make its own
    /// call.
    #[must_use]
    pub fn http(&self) -> &reqwest::Client {
        &self.inner.http
    }

    /// The configured base URL.
    #[must_use]
    pub fn base_url(&self) -> &Url {
        &self.inner.base_url
    }

    /// The retry policy in force.
    #[must_use]
    pub fn retry_policy(&self) -> &RetryPolicy {
        &self.inner.retry
    }

    /// Join `path` to the base URL and validate the result.
    ///
    /// A path starting with `/` replaces the base path; an absolute `https://`
    /// URL is accepted as-is so a provider can target another host.
    pub fn url(&self, path: &str) -> Result<Url, WirekitError> {
        let url = self.inner.base_url.join(path).map_err(|error| {
            WirekitError::invalid_config(format!("the request path could not be joined: {error}"))
        })?;
        self.validate_request_url(&url)?;
        Ok(url)
    }

    /// Execute a request and return the raw response.
    ///
    /// Order (AC-032): auth, then rate limit, then the retry loop around send.
    /// The body-level error hook runs on a 2xx response, so HTTP 200 is not
    /// treated as success when the body reports an error (CON-213).
    pub async fn send(&self, mut spec: RequestSpec) -> Result<RawResponse, WirekitError> {
        self.validate_request_url(spec.url())?;

        if let Some(scheme) = spec.auth_scheme().cloned() {
            let context = AuthContext::new(&self.inner.http);
            scheme.apply(&mut spec, &context).await?;
        }

        let bucket_key = spec
            .rate_limit_key_ref()
            .map(str::to_owned)
            .unwrap_or_else(|| {
                self.inner
                    .base_url
                    .host_str()
                    .unwrap_or("default")
                    .to_string()
            });
        self.inner.rate_limiter.acquire(&bucket_key).await;

        let limit = spec
            .response_limit()
            .unwrap_or(self.inner.max_response_bytes);
        let attempts = self.inner.retry.attempts;
        let mut last_error: Option<WirekitError> = None;

        for attempt in 1..=attempts {
            trace::request_started(spec.method(), spec.url());
            let started = Instant::now();

            let mut request = self
                .inner
                .http
                .request(spec.method().clone(), spec.url().clone())
                .headers(spec.headers().clone());
            if let Some(body) = spec.body() {
                request = request.body(body.to_vec());
            }

            let outcome = request.send().await;

            let response = match outcome {
                Ok(response) => response,
                Err(error) => {
                    let mapped = map_transport_error(error);
                    if should_retry(&mapped, spec.non_retryable_ref()) && attempt < attempts {
                        let delay = self.inner.retry.delay_before_retry(attempt, None);
                        trace::retry(attempt, delay, &mapped);
                        last_error = Some(mapped);
                        tokio::time::sleep(delay).await;
                        continue;
                    }
                    trace::request_failed(&mapped);
                    return Err(mapped);
                }
            };

            let elapsed = started.elapsed();
            let status = response.status().as_u16();
            let retry_after = response
                .headers()
                .get(http::header::RETRY_AFTER)
                .and_then(|value| value.to_str().ok())
                .and_then(parse_retry_after);
            let headers = response.headers().clone();
            let bytes = read_bounded(response, limit).await?;
            trace::request_finished(spec.method(), status, elapsed);

            if (200..300).contains(&status) {
                if let Some(hook) = spec.hook() {
                    if let Some(error) = hook(status, &bytes) {
                        if should_retry(&error, spec.non_retryable_ref()) && attempt < attempts {
                            let delay = self
                                .inner
                                .retry
                                .delay_before_retry(attempt, error.retry_after());
                            trace::retry(attempt, delay, &error);
                            last_error = Some(error);
                            tokio::time::sleep(delay).await;
                            continue;
                        }
                        trace::request_failed(&error);
                        return Err(error);
                    }
                }

                return Ok(RawResponse {
                    status,
                    headers,
                    body: bytes,
                });
            }

            let error = if status == 429 {
                WirekitError::RateLimit { retry_after }
            } else {
                WirekitError::api_from_body(status, &bytes)
            };

            if should_retry(&error, spec.non_retryable_ref()) && attempt < attempts {
                let delay = self.inner.retry.delay_before_retry(attempt, retry_after);
                trace::retry(attempt, delay, &error);
                last_error = Some(error);
                tokio::time::sleep(delay).await;
                continue;
            }

            trace::request_failed(&error);
            return Err(error);
        }

        Err(last_error.unwrap_or_else(|| {
            WirekitError::network("the retry loop finished without producing an outcome")
        }))
    }

    /// `GET path` and decode the body.
    pub async fn get_json<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<Response<T>, WirekitError> {
        let url = self.url(path)?;
        self.execute(RequestSpec::get(url)).await
    }

    /// Execute a request and decode the body into `T`.
    pub async fn execute<T: DeserializeOwned>(
        &self,
        spec: RequestSpec,
    ) -> Result<Response<T>, WirekitError> {
        let raw = self.send(spec).await?;
        let value = decode_body(&raw.body)?;
        Ok(Response {
            status: raw.status,
            headers: raw.headers,
            value,
        })
    }

    /// Reject a request URL that is not HTTPS, unless the client explicitly
    /// allows a loopback address for local development.
    fn validate_request_url(&self, url: &Url) -> Result<(), WirekitError> {
        if url.scheme() == "https" {
            return validate_url(url);
        }
        if self.inner.allow_insecure_localhost && is_loopback(url) {
            return Ok(());
        }
        validate_url(url)
    }
}

/// A response that has not been decoded.
#[derive(Debug)]
pub struct RawResponse {
    /// The HTTP status.
    pub status: u16,
    /// The response headers.
    pub headers: HeaderMap,
    /// The raw body bytes, bounded by the configured limit.
    pub body: Vec<u8>,
}

impl RawResponse {
    /// The first value of `name` as UTF-8, if present.
    #[must_use]
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers.get(name).and_then(|value| value.to_str().ok())
    }
}

fn is_loopback(url: &Url) -> bool {
    match url.host_str() {
        Some(host) => {
            host == "localhost"
                || host == "::1"
                || host == "[::1]"
                || host.starts_with("127.")
                || host == "127.0.0.1"
        }
        None => false,
    }
}

/// The builder for [`WirekitClient`].
pub struct WirekitClientBuilder {
    base_url: Url,
    injected: Option<reqwest::Client>,
    timeout: Option<Duration>,
    connect_timeout: Option<Duration>,
    pool_max_idle_per_host: Option<usize>,
    proxy: Option<String>,
    default_headers: HeaderMap,
    retry: RetryPolicy,
    rate_limit_default: Option<RateLimitConfig>,
    rate_limit_overrides: HashMap<String, RateLimitConfig>,
    max_response_bytes: usize,
    allow_insecure_localhost: bool,
}

impl std::fmt::Debug for WirekitClientBuilder {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WirekitClientBuilder")
            .field("base_url", &self.base_url)
            .field("injected", &self.injected.is_some())
            .field("timeout", &self.timeout)
            .field("retry", &self.retry)
            .field("max_response_bytes", &self.max_response_bytes)
            .finish_non_exhaustive()
    }
}

impl WirekitClientBuilder {
    /// Use a caller-owned `reqwest::Client` verbatim (AC-017).
    ///
    /// When a client is injected, no second pool is created and the timeout,
    /// proxy, and pool settings on this builder are ignored; default headers
    /// still apply.
    #[must_use]
    pub fn injected_client(mut self, client: reqwest::Client) -> Self {
        self.injected = Some(client);
        self
    }

    /// Set the per-request timeout.
    #[must_use]
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the connect timeout.
    #[must_use]
    pub fn connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = Some(timeout);
        self
    }

    /// Set how many idle connections are kept per host.
    #[must_use]
    pub fn pool_max_idle_per_host(mut self, limit: usize) -> Self {
        self.pool_max_idle_per_host = Some(limit);
        self
    }

    /// Route requests through a proxy.
    #[must_use]
    pub fn proxy(mut self, proxy_url: impl Into<String>) -> Self {
        self.proxy = Some(proxy_url.into());
        self
    }

    /// Add a header to every request from this client.
    pub fn default_header(mut self, name: HeaderName, value: HeaderValue) -> Self {
        self.default_headers.insert(name, value);
        self
    }

    /// Replace the retry policy.
    #[must_use]
    pub fn retry(mut self, policy: RetryPolicy) -> Self {
        self.retry = policy;
        self
    }

    /// Apply a token bucket to every rate-limit key that has no override.
    #[must_use]
    pub fn rate_limit(mut self, config: RateLimitConfig) -> Self {
        self.rate_limit_default = Some(config);
        self
    }

    /// Apply a token bucket to one provider or account key.
    #[must_use]
    pub fn rate_limit_for(mut self, key: impl Into<String>, config: RateLimitConfig) -> Self {
        self.rate_limit_overrides.insert(key.into(), config);
        self
    }

    /// Bound the number of bytes read from a response body.
    #[must_use]
    pub fn max_response_bytes(mut self, limit: usize) -> Self {
        self.max_response_bytes = limit;
        self
    }

    /// Allow plain-HTTP requests to a loopback address.
    ///
    /// This exists for local mock servers and development only. The default is
    /// strict HTTPS, so a production base URL cannot be downgraded.
    #[must_use]
    pub fn allow_insecure_localhost(mut self, allow: bool) -> Self {
        self.allow_insecure_localhost = allow;
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<WirekitClient, WirekitError> {
        self.retry.validate()?;
        if !(self.allow_insecure_localhost && is_loopback(&self.base_url)) {
            validate_url(&self.base_url)?;
        }
        if self.max_response_bytes == 0 {
            return Err(WirekitError::invalid_config(
                "max_response_bytes must be at least 1",
            ));
        }

        let http = match self.injected {
            Some(client) => client,
            None => {
                let mut builder =
                    reqwest::Client::builder().default_headers(self.default_headers.clone());
                if let Some(timeout) = self.timeout {
                    builder = builder.timeout(timeout);
                }
                if let Some(connect_timeout) = self.connect_timeout {
                    builder = builder.connect_timeout(connect_timeout);
                }
                if let Some(limit) = self.pool_max_idle_per_host {
                    builder = builder.pool_max_idle_per_host(limit);
                }
                if let Some(proxy) = self.proxy.as_deref() {
                    builder =
                        builder.proxy(reqwest::Proxy::all(proxy).map_err(|_| {
                            WirekitError::invalid_config("the proxy URL is not valid")
                        })?);
                }
                builder.build().map_err(|_| {
                    WirekitError::invalid_config(
                        "the HTTP client could not be built; check the timeout, proxy, and TLS settings",
                    )
                })?
            }
        };

        Ok(WirekitClient {
            inner: Arc::new(ClientInner {
                http,
                base_url: self.base_url,
                retry: self.retry,
                rate_limiter: RateLimiter::new(self.rate_limit_default, self.rate_limit_overrides),
                max_response_bytes: self.max_response_bytes,
                allow_insecure_localhost: self.allow_insecure_localhost,
            }),
        })
    }
}

/// Reject a URL that is not HTTPS or that embeds credentials.
pub(crate) fn validate_url(url: &Url) -> Result<(), WirekitError> {
    if url.scheme() != "https" {
        return Err(WirekitError::invalid_config(format!(
            "all requests must use https; {} is not allowed",
            url.scheme()
        )));
    }
    if url.host_str().is_none() {
        return Err(WirekitError::invalid_config("the URL has no host"));
    }
    if !url.username().is_empty() || url.password().is_some() {
        return Err(WirekitError::invalid_config(
            "credentials must not be embedded in a URL; use an auth scheme",
        ));
    }
    Ok(())
}

/// Map a `reqwest` failure without rendering its URL.
fn map_transport_error(error: reqwest::Error) -> WirekitError {
    if error.is_timeout() {
        return WirekitError::Timeout;
    }
    let message = if error.is_connect() {
        "could not connect to the provider"
    } else if error.is_body() {
        "the response body could not be read"
    } else if error.is_redirect() {
        "the provider redirected a request the client will not follow"
    } else {
        "the request failed in transport"
    };
    WirekitError::network_with(message, error)
}

/// Read a response body, refusing anything past `limit` bytes (EC-017).
async fn read_bounded(
    mut response: reqwest::Response,
    limit: usize,
) -> Result<Vec<u8>, WirekitError> {
    if let Some(length) = response.content_length() {
        if length as usize > limit {
            return Err(WirekitError::decode(format!(
                "the response declared {length} bytes, over the {limit}-byte limit"
            )));
        }
    }

    let mut collected: Vec<u8> = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| WirekitError::network_with("the response body could not be read", error))?
    {
        if collected.len() + chunk.len() > limit {
            return Err(WirekitError::decode(format!(
                "the response body exceeded the {limit}-byte limit"
            )));
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected)
}

/// The HTTP method a provider passes to [`RequestSpec`].
pub type HttpMethod = Method;
