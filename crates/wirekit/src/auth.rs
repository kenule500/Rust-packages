//! The auth layer: five server-side schemes over a redacting secret wrapper.
//!
//! Scope: Bearer, Basic, API key in a header, API key in the query string,
//! HMAC request signing, and OAuth2 client credentials. Every scheme applies
//! to an outbound [`RequestSpec`] and fails closed when its credentials are
//! missing or unusable (CON-511, NFR-011).
//!
//! Non-goals: browser OAuth (Authorization Code + PKCE belongs to the
//! integration that owns the redirect, CON-212), credential storage, and
//! reading credentials from the environment (CON-519).

use std::fmt;
use std::time::{Duration, Instant};

use async_trait::async_trait;
use base64::Engine as _;
use http::{HeaderName, HeaderValue};
use percent_encoding::{utf8_percent_encode, AsciiSet, CONTROLS};
use url::Url;

use crate::error::WirekitError;
use crate::pipeline::RequestSpec;
use crate::secret::Secret;

/// Characters escaped in a query value this crate builds itself.
const QUERY_VALUE: &AsciiSet = &CONTROLS
    .add(b' ')
    .add(b'"')
    .add(b'#')
    .add(b'&')
    .add(b'+')
    .add(b'/')
    .add(b':')
    .add(b';')
    .add(b'<')
    .add(b'=')
    .add(b'>')
    .add(b'?')
    .add(b'@')
    .add(b'[')
    .add(b'\\')
    .add(b']')
    .add(b'^')
    .add(b'`')
    .add(b'{')
    .add(b'|')
    .add(b'}');

/// The shared HTTP client, handed to a scheme that needs to make its own call.
#[derive(Clone, Copy)]
pub struct AuthContext<'a> {
    http: &'a reqwest::Client,
}

impl<'a> AuthContext<'a> {
    /// Wrap the client's pooled transport.
    pub fn new(http: &'a reqwest::Client) -> Self {
        Self { http }
    }

    /// The pooled transport, for a scheme such as OAuth2 that fetches a token.
    #[must_use]
    pub fn http(&self) -> &reqwest::Client {
        self.http
    }
}

/// A scheme that prepares an outbound request.
///
/// Implementations mutate the request in place and must be idempotent: the
/// pipeline calls `apply` again on every retry attempt.
#[async_trait]
pub trait AuthScheme: Send + Sync + fmt::Debug {
    /// Apply the credential to `request`.
    async fn apply(
        &self,
        request: &mut RequestSpec,
        context: &AuthContext<'_>,
    ) -> Result<(), WirekitError>;

    /// A short name for tracing.
    fn name(&self) -> &'static str;
}

fn header_value(name: &str, value: String) -> Result<(HeaderName, HeaderValue), WirekitError> {
    let name = HeaderName::from_bytes(name.as_bytes()).map_err(|_| {
        WirekitError::invalid_config(format!("{name} is not a valid HTTP header name"))
    })?;
    let value = HeaderValue::from_str(&value).map_err(|_| {
        WirekitError::invalid_config("a credential cannot be represented as an HTTP header value")
    })?;
    Ok((name, value))
}

/// `Authorization: Bearer <token>`.
#[derive(Debug, Clone)]
pub struct BearerAuth {
    token: Secret<String>,
}

impl BearerAuth {
    /// Wrap a token. An empty token is rejected when the request is prepared.
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            token: Secret::new(token.into()),
        }
    }
}

#[async_trait]
impl AuthScheme for BearerAuth {
    async fn apply(
        &self,
        request: &mut RequestSpec,
        _context: &AuthContext<'_>,
    ) -> Result<(), WirekitError> {
        let token = self.token.expose();
        if token.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a bearer token is required but none was configured",
            ));
        }
        let (name, value) = header_value("authorization", format!("Bearer {token}"))?;
        request.insert_header(name, value);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "bearer"
    }
}

/// `Authorization: Basic base64(user:password)`.
#[derive(Debug, Clone)]
pub struct BasicAuth {
    username: Secret<String>,
    password: Secret<String>,
}

impl BasicAuth {
    /// Wrap a username and password pair.
    pub fn new(username: impl Into<String>, password: impl Into<String>) -> Self {
        Self {
            username: Secret::new(username.into()),
            password: Secret::new(password.into()),
        }
    }
}

#[async_trait]
impl AuthScheme for BasicAuth {
    async fn apply(
        &self,
        request: &mut RequestSpec,
        _context: &AuthContext<'_>,
    ) -> Result<(), WirekitError> {
        if self.username.expose().trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "basic authentication requires a username",
            ));
        }
        let pair = format!("{}:{}", self.username.expose(), self.password.expose());
        let encoded = base64::engine::general_purpose::STANDARD.encode(pair.as_bytes());
        let (name, value) = header_value("authorization", format!("Basic {encoded}"))?;
        request.insert_header(name, value);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "basic"
    }
}

/// An API key carried in a configured request header.
#[derive(Debug, Clone)]
pub struct ApiKeyHeader {
    header: HeaderName,
    key: Secret<String>,
}

impl ApiKeyHeader {
    /// Build the scheme, rejecting an invalid header name.
    pub fn new(header: impl AsRef<str>, key: impl Into<String>) -> Result<Self, WirekitError> {
        let header = HeaderName::from_bytes(header.as_ref().as_bytes()).map_err(|_| {
            WirekitError::invalid_config(format!(
                "{} is not a valid HTTP header name",
                header.as_ref()
            ))
        })?;
        Ok(Self {
            header,
            key: Secret::new(key.into()),
        })
    }
}

#[async_trait]
impl AuthScheme for ApiKeyHeader {
    async fn apply(
        &self,
        request: &mut RequestSpec,
        _context: &AuthContext<'_>,
    ) -> Result<(), WirekitError> {
        let key = self.key.expose();
        if key.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "an API key is required but none was configured",
            ));
        }
        let value = HeaderValue::from_str(key).map_err(|_| {
            WirekitError::invalid_config("the API key cannot be represented as a header value")
        })?;
        request.insert_header(self.header.clone(), value);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "api-key-header"
    }
}

/// An API key carried as a query parameter.
#[derive(Debug, Clone)]
pub struct ApiKeyQuery {
    param: String,
    key: Secret<String>,
}

impl ApiKeyQuery {
    /// Build the scheme with the parameter name the provider expects.
    pub fn new(param: impl Into<String>, key: impl Into<String>) -> Self {
        Self {
            param: param.into(),
            key: Secret::new(key.into()),
        }
    }
}

#[async_trait]
impl AuthScheme for ApiKeyQuery {
    async fn apply(
        &self,
        request: &mut RequestSpec,
        _context: &AuthContext<'_>,
    ) -> Result<(), WirekitError> {
        let key = self.key.expose();
        if key.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "an API key is required but none was configured",
            ));
        }

        // Existing parameters are preserved; the key is percent-encoded so a
        // value containing `&`, `=`, or a space cannot alter the query.
        let mut pairs: Vec<(String, String)> = request
            .url()
            .query_pairs()
            .filter(|(name, _)| name != self.param.as_str())
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect();
        pairs.push((self.param.clone(), key.clone()));

        let query = pairs
            .iter()
            .map(|(name, value)| {
                format!(
                    "{}={}",
                    utf8_percent_encode(name, QUERY_VALUE),
                    utf8_percent_encode(value, QUERY_VALUE)
                )
            })
            .collect::<Vec<_>>()
            .join("&");

        request.url_mut().set_query(Some(&query));
        Ok(())
    }

    fn name(&self) -> &'static str {
        "api-key-query"
    }
}

/// The digest used by an HMAC request signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HmacAlgorithm {
    /// HMAC-SHA256.
    Sha256,
}

/// HMAC request signing over the canonical raw request bytes.
///
/// The canonical form is `METHOD \n URL \n BODY`, signed with the configured
/// key and emitted base64-encoded in the configured header. The body is the
/// exact byte sequence that will be sent, never a re-serialization (CON-215).
#[derive(Debug, Clone)]
pub struct HmacAuth {
    key: Secret<Vec<u8>>,
    header: HeaderName,
    algorithm: HmacAlgorithm,
}

impl HmacAuth {
    /// Build the scheme.
    pub fn new(
        key: impl Into<Vec<u8>>,
        header: impl AsRef<str>,
        algorithm: HmacAlgorithm,
    ) -> Result<Self, WirekitError> {
        let header = HeaderName::from_bytes(header.as_ref().as_bytes()).map_err(|_| {
            WirekitError::invalid_config(format!(
                "{} is not a valid HTTP header name",
                header.as_ref()
            ))
        })?;
        Ok(Self {
            key: Secret::new(key.into()),
            header,
            algorithm,
        })
    }
}

#[async_trait]
impl AuthScheme for HmacAuth {
    async fn apply(
        &self,
        request: &mut RequestSpec,
        _context: &AuthContext<'_>,
    ) -> Result<(), WirekitError> {
        if self.key.expose().is_empty() {
            return Err(WirekitError::invalid_config(
                "an HMAC key is required but none was configured",
            ));
        }

        let mut canonical = Vec::with_capacity(64);
        canonical.extend_from_slice(request.method().as_str().as_bytes());
        canonical.push(b'\n');
        canonical.extend_from_slice(request.url().as_str().as_bytes());
        canonical.push(b'\n');
        if let Some(body) = request.body() {
            canonical.extend_from_slice(body);
        }

        let signature = match self.algorithm {
            HmacAlgorithm::Sha256 => hmac_sha256_base64(self.key.expose(), &canonical),
        };
        let value = HeaderValue::from_str(&signature).map_err(|_| {
            WirekitError::invalid_config("the computed signature is not a valid header value")
        })?;
        request.insert_header(self.header.clone(), value);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "hmac"
    }
}

/// Constant-time byte comparison.
///
/// Used by every webhook verifier so that a comparison cannot leak the
/// expected signature through timing.
#[must_use]
pub fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

/// HMAC-SHA256 over `message`, base64-encoded.
#[must_use]
pub fn hmac_sha256_base64(key: &[u8], message: &[u8]) -> String {
    use hmac::{Hmac, Mac};
    use sha2::Sha256;

    let mut mac = Hmac::<Sha256>::new_from_slice(key).expect("HMAC accepts a key of any length");
    mac.update(message);
    base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes())
}

/// OAuth2 client-credentials flow with a cached, single-flight token.
///
/// The first request fetches a token; later requests reuse it until it is
/// within `refresh_margin` of expiry, at which point exactly one request
/// refreshes it (FR-018, EC-010, EC-020).
#[derive(Debug)]
pub struct OAuth2ClientCredentials {
    token_url: Url,
    client_id: Secret<String>,
    client_secret: Secret<String>,
    scopes: Vec<String>,
    refresh_margin: Duration,
    state: tokio::sync::Mutex<TokenState>,
}

#[derive(Debug, Default)]
struct TokenState {
    token: Option<OAuth2Token>,
}

/// A cached OAuth2 access token and the instant it expires.
///
/// The token is a [`Secret`], so neither `Debug` nor a log line can print it.
#[derive(Debug)]
struct OAuth2Token {
    access_token: Secret<String>,
    expires_at: Instant,
}

impl OAuth2ClientCredentials {
    /// Build the scheme. `token_url` must be HTTPS.
    pub fn new(
        token_url: impl AsRef<str>,
        client_id: impl Into<String>,
        client_secret: impl Into<String>,
    ) -> Result<Self, WirekitError> {
        let token_url = Url::parse(token_url.as_ref()).map_err(|error| {
            WirekitError::invalid_config(format!("the OAuth2 token URL is not a URL: {error}"))
        })?;
        if token_url.scheme() != "https" {
            return Err(WirekitError::invalid_config(
                "the OAuth2 token URL must use https",
            ));
        }
        Ok(Self {
            token_url,
            client_id: Secret::new(client_id.into()),
            client_secret: Secret::new(client_secret.into()),
            scopes: Vec::new(),
            refresh_margin: Duration::from_secs(60),
            state: tokio::sync::Mutex::new(TokenState::default()),
        })
    }

    /// Request these scopes, space-separated in the token request.
    #[must_use]
    pub fn with_scopes(mut self, scopes: impl IntoIterator<Item = impl Into<String>>) -> Self {
        self.scopes = scopes.into_iter().map(Into::into).collect();
        self
    }

    /// Refresh this long before expiry. Capped at half the token lifetime by
    /// the caller-side computation below.
    #[must_use]
    pub fn with_refresh_margin(mut self, margin: Duration) -> Self {
        self.refresh_margin = margin;
        self
    }

    async fn fetch_token(
        &self,
        context: &AuthContext<'_>,
    ) -> Result<(Secret<String>, Instant), WirekitError> {
        if self.client_id.expose().trim().is_empty() || self.client_secret.expose().is_empty() {
            return Err(WirekitError::invalid_config(
                "OAuth2 client credentials require a client id and a client secret",
            ));
        }

        let mut form: Vec<(&str, &str)> = vec![
            ("grant_type", "client_credentials"),
            ("client_id", self.client_id.expose().as_str()),
            ("client_secret", self.client_secret.expose().as_str()),
        ];
        let scope = self.scopes.join(" ");
        if !scope.is_empty() {
            form.push(("scope", scope.as_str()));
        }

        let response = context
            .http()
            .post(self.token_url.clone())
            .form(&form)
            .send()
            .await
            .map_err(|error| {
                WirekitError::network_with("the OAuth2 token request failed", error)
            })?;

        let status = response.status();
        let body = response.bytes().await.map_err(|error| {
            WirekitError::network_with("the OAuth2 token response could not be read", error)
        })?;

        if !status.is_success() {
            let status_code = status.as_u16();
            if status.is_client_error() && status_code != 429 {
                return Err(WirekitError::auth(format!(
                    "the OAuth2 token endpoint rejected the client credentials (HTTP {status_code})"
                )));
            }
            return Err(WirekitError::api(
                status_code,
                format!("oauth2_{status_code}"),
                "the OAuth2 token endpoint returned a failure",
            ));
        }

        let parsed: TokenResponse = serde_json::from_slice(&body).map_err(|error| {
            WirekitError::decode_with("the OAuth2 token response did not match", error)
        })?;

        let lifetime = Duration::from_secs(parsed.expires_in.unwrap_or(300).max(1));
        let margin = self.refresh_margin.min(lifetime / 2);
        let expires_at = Instant::now() + lifetime.saturating_sub(margin);
        Ok((Secret::new(parsed.access_token), expires_at))
    }
}

#[derive(serde::Deserialize)]
struct TokenResponse {
    access_token: String,
    #[serde(default)]
    expires_in: Option<u64>,
}

#[async_trait]
impl AuthScheme for OAuth2ClientCredentials {
    async fn apply(
        &self,
        request: &mut RequestSpec,
        context: &AuthContext<'_>,
    ) -> Result<(), WirekitError> {
        let mut state = self.state.lock().await;

        let valid = state
            .token
            .as_ref()
            .is_some_and(|token| token.expires_at > Instant::now());
        if !valid {
            let (access_token, expires_at) = self.fetch_token(context).await?;
            state.token = Some(OAuth2Token {
                access_token,
                expires_at,
            });
        }

        let token = state
            .token
            .as_ref()
            .map(|token| token.access_token.expose().clone())
            .unwrap_or_default();
        let (name, value) = header_value("authorization", format!("Bearer {token}"))?;
        request.insert_header(name, value);
        Ok(())
    }

    fn name(&self) -> &'static str {
        "oauth2-client-credentials"
    }
}
