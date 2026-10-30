//! Google OAuth 2.0: Authorization Code with PKCE.
//!
//! Scope: build an authorization URL carrying `state` and an `S256` PKCE challenge,
//! exchange a code, refresh an access token at most once, revoke a token, and hand
//! every token to a caller-supplied [`TokenStore`].
//!
//! Non-goals: persistence of any kind - the library never touches a file, database,
//! or environment variable (CON-108). The implicit flow cannot be expressed: an
//! [`AuthorizationRequest`] is only produced by a constructor that generates a
//! verifier (CON-212). Client-credentials flow lives in `crate::auth`.

use std::collections::HashMap;
use std::fmt;
use std::sync::Mutex;
use std::time::{Duration, SystemTime};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine as _;
use rand::distributions::Alphanumeric;
use rand::Rng;
use sha2::{Digest, Sha256};

use crate::pipeline::RequestSpec;
use crate::{Secret, WirekitClient, WirekitError};

/// Google's authorization endpoint.
pub const AUTH_ENDPOINT: &str = "https://oauth2.googleapis.com/o/oauth2/v2/auth";
/// Google's token endpoint.
pub const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
/// Google's revocation endpoint.
pub const REVOKE_ENDPOINT: &str = "https://oauth2.googleapis.com/revoke";
/// Default safety margin before expiry at which a token is refreshed.
pub const DEFAULT_EXPIRY_MARGIN: Duration = Duration::from_secs(60);

/// A source of the current time.
///
/// The clock is injected so a caller can stamp expiry and decide whether a token
/// has expired without depending on the wall clock (FR-004). The default is
/// [`SystemTime::now`].
pub type Clock = std::sync::Arc<dyn Fn() -> SystemTime + Send + Sync>;

/// Persistence boundary for tokens.
///
/// `wirekit` never stores a token itself; the caller supplies an implementation.
/// A durable implementation MUST encrypt refresh tokens at rest and SHOULD lock per
/// record so two processes do not refresh the same token concurrently.
pub trait TokenStore: Send + Sync {
    /// Load the record for `key`.
    fn load(&self, key: &str) -> Option<TokenResponse>;
    /// Save (or replace) the record for `key`.
    fn save(&self, key: &str, token: &TokenResponse);
    /// Delete the record for `key`.
    fn delete(&self, key: &str);
}

/// A non-durable store for tests and single-process callers.
///
/// Tokens live in memory and disappear with the process. Not suitable for
/// refresh tokens that must survive a restart.
#[derive(Default)]
pub struct MemoryTokenStore {
    entries: Mutex<HashMap<String, TokenResponse>>,
}

impl MemoryTokenStore {
    /// Create an empty store.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl TokenStore for MemoryTokenStore {
    fn load(&self, key: &str) -> Option<TokenResponse> {
        self.entries.lock().ok()?.get(key).cloned()
    }

    fn save(&self, key: &str, token: &TokenResponse) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.insert(key.to_string(), token.clone());
        }
    }

    fn delete(&self, key: &str) {
        if let Ok(mut entries) = self.entries.lock() {
            entries.remove(key);
        }
    }
}

/// The result of an authorization, code exchange, or refresh.
#[derive(Clone)]
pub struct TokenResponse {
    access_token: Secret<String>,
    refresh_token: Option<Secret<String>>,
    expires_at: SystemTime,
    scopes: Vec<String>,
}

impl TokenResponse {
    /// The bearer token to send to an API. Expose it only where the header is set.
    #[must_use]
    pub fn access_token(&self) -> &Secret<String> {
        &self.access_token
    }

    /// The refresh token, when the provider issued one.
    #[must_use]
    pub fn refresh_token(&self) -> Option<&Secret<String>> {
        self.refresh_token.as_ref()
    }

    /// The absolute expiry instant.
    #[must_use]
    pub fn expires_at(&self) -> SystemTime {
        self.expires_at
    }

    /// The scopes the provider granted.
    #[must_use]
    pub fn scopes(&self) -> &[String] {
        &self.scopes
    }

    /// Whether the token is inside `margin` of expiry.
    #[must_use]
    pub fn is_expired(&self, margin: Duration) -> bool {
        self.is_expired_at(SystemTime::now(), margin)
    }

    /// Whether the token is inside `margin` of expiry at the instant `now`.
    ///
    /// A `now` before the recorded issue time means the token is already
    /// suspect, so it counts as expired (EC-006).
    #[must_use]
    pub fn is_expired_at(&self, now: SystemTime, margin: Duration) -> bool {
        match self.expires_at.checked_sub(margin) {
            Some(threshold) => now >= threshold,
            None => true,
        }
    }
}

impl fmt::Debug for TokenResponse {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenResponse")
            .field("access_token", &"***")
            .field(
                "refresh_token",
                &if self.refresh_token.is_some() {
                    "***"
                } else {
                    "<none>"
                },
            )
            .field("expires_at", &self.expires_at)
            .field("scopes", &self.scopes)
            .finish()
    }
}

/// An authorization URL plus the secrets that must accompany it.
///
/// Holding the request is what keeps PKCE honest: the verifier exists before the
/// callback and is required to exchange the code.
#[derive(Clone)]
pub struct AuthorizationRequest {
    url: url::Url,
    state: Secret<String>,
    code_verifier: Secret<String>,
}

impl AuthorizationRequest {
    /// The URL to send the user to.
    #[must_use]
    pub fn url(&self) -> &url::Url {
        &self.url
    }

    /// The `state` to store server-side and compare on callback.
    #[must_use]
    pub fn state(&self) -> &Secret<String> {
        &self.state
    }

    /// The PKCE verifier, kept until the code is exchanged.
    #[must_use]
    pub fn code_verifier(&self) -> &Secret<String> {
        &self.code_verifier
    }
}

impl fmt::Debug for AuthorizationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthorizationRequest")
            .field("url", &redact_state(&self.url))
            .field("state", &"***")
            .field("code_verifier", &"***")
            .finish()
    }
}

/// Render `url` with the `state` query value masked.
///
/// The URL must carry `state`, so printing it verbatim would leak the value the
/// adjacent `state: ***` field exists to protect (CON-214). Every other parameter
/// stays readable, so the output is still useful when debugging a callback.
fn redact_state(url: &url::Url) -> String {
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(key, value)| {
            let value = if key == "state" {
                String::from("***")
            } else {
                value.into_owned()
            };
            (key.into_owned(), value)
        })
        .collect();

    let mut redacted = url.clone();
    redacted
        .query_pairs_mut()
        .clear()
        .extend_pairs(pairs.iter().map(|(key, value)| (key.as_str(), value.as_str())));
    redacted.to_string()
}

/// Revoke an access token or a refresh token.
#[derive(Clone)]
pub struct RevokeRequest {
    token: Secret<String>,
    hint: Option<String>,
}

impl RevokeRequest {
    /// Revoke `token`, with an optional `token_type_hint`.
    #[must_use]
    pub fn new(token: Secret<String>, hint: Option<&str>) -> Self {
        Self {
            token,
            hint: hint.map(str::to_string),
        }
    }

    /// The token to revoke.
    #[must_use]
    pub fn token(&self) -> &Secret<String> {
        &self.token
    }

    /// The provider hint, when supplied.
    #[must_use]
    pub fn hint(&self) -> Option<&str> {
        self.hint.as_deref()
    }
}

impl fmt::Debug for RevokeRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RevokeRequest")
            .field("token", &"***")
            .field("hint", &self.hint)
            .finish()
    }
}

/// A Google OAuth2 client for one application registration.
///
/// Holds no token cache and no global state (CON-109); tokens are exchanged and
/// handed to the caller's [`TokenStore`].
#[derive(Clone)]
pub struct GoogleOAuthClient {
    client: WirekitClient,
    client_id: String,
    client_secret: Secret<String>,
    redirect_uri: String,
    scopes: Vec<String>,
    auth_endpoint: url::Url,
    token_endpoint: url::Url,
    revoke_endpoint: url::Url,
    expiry_margin: Duration,
    clock: Clock,
}

impl fmt::Debug for GoogleOAuthClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("GoogleOAuthClient")
            .field("client_id", &self.client_id)
            .field("client_secret", &"***")
            .field("redirect_uri", &self.redirect_uri)
            .field("scopes", &self.scopes)
            .field("token_endpoint", &self.token_endpoint.as_str())
            .field("expiry_margin", &self.expiry_margin)
            .finish()
    }
}

/// The token endpoint's successful response.
#[derive(serde::Deserialize)]
struct WireToken {
    access_token: String,
    #[serde(default)]
    refresh_token: Option<String>,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    scope: Option<String>,
}

/// The token endpoint's error response.
#[derive(serde::Deserialize)]
struct WireError {
    #[serde(default)]
    error: String,
    #[serde(default)]
    error_description: String,
}

impl GoogleOAuthClient {
    /// Build a client for one Google OAuth application.
    ///
    /// The redirect URI is sent verbatim; it must match the registered value, and a
    /// mismatch surfaces as a provider error rather than being rewritten (EC-003).
    pub fn new(
        client: WirekitClient,
        client_id: impl Into<String>,
        client_secret: Secret<String>,
        redirect_uri: impl Into<String>,
        scopes: Vec<String>,
    ) -> Result<Self, WirekitError> {
        Ok(Self {
            client,
            client_id: client_id.into(),
            client_secret,
            redirect_uri: redirect_uri.into(),
            scopes,
            auth_endpoint: parse_endpoint(AUTH_ENDPOINT)?,
            token_endpoint: parse_endpoint(TOKEN_ENDPOINT)?,
            revoke_endpoint: parse_endpoint(REVOKE_ENDPOINT)?,
            expiry_margin: DEFAULT_EXPIRY_MARGIN,
            clock: std::sync::Arc::new(SystemTime::now),
        })
    }

    /// Point the client at different endpoints, for a sandbox or a test.
    ///
    /// A non-HTTPS endpoint is rejected unless it is loopback (CON-520). A loopback
    /// endpoint also needs a shared client built with
    /// [`WirekitClientBuilder::allow_insecure_localhost`](crate::WirekitClientBuilder::allow_insecure_localhost),
    /// because the core client validates every URL again when the request is sent.
    pub fn with_endpoints(
        mut self,
        auth_endpoint: &str,
        token_endpoint: &str,
        revoke_endpoint: &str,
    ) -> Result<Self, WirekitError> {
        self.auth_endpoint = parse_endpoint(auth_endpoint)?;
        self.token_endpoint = parse_endpoint(token_endpoint)?;
        self.revoke_endpoint = parse_endpoint(revoke_endpoint)?;
        Ok(self)
    }

    /// Override the margin before expiry at which a token counts as expired.
    #[must_use]
    pub fn with_expiry_margin(mut self, margin: Duration) -> Self {
        self.expiry_margin = margin;
        self
    }

    /// Replace the clock used to stamp expiry and decide whether a token has
    /// expired.
    ///
    /// The clock is the caller's, so a sandbox or a test can move time without
    /// waiting (FR-004).
    #[must_use]
    pub fn with_clock(mut self, clock: impl Fn() -> SystemTime + Send + Sync + 'static) -> Self {
        self.clock = std::sync::Arc::new(clock);
        self
    }

    /// The configured expiry margin.
    #[must_use]
    pub fn expiry_margin(&self) -> Duration {
        self.expiry_margin
    }

    /// Build the authorization URL and the PKCE verifier that must survive until the
    /// callback.
    ///
    /// `state` and the verifier come from a CSPRNG and differ on every call (AC-002).
    /// There is no argument that omits PKCE (AC-003).
    pub fn authorization_url(&self) -> Result<AuthorizationRequest, WirekitError> {
        let state = Secret::new(random_token(32));
        let verifier = Secret::new(random_token(64));
        let challenge = pkce_challenge(verifier.expose());

        let mut url = self.auth_endpoint.clone();
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("response_type", "code");
            pairs.append_pair("client_id", &self.client_id);
            pairs.append_pair("redirect_uri", &self.redirect_uri);
            pairs.append_pair("scope", &self.scopes.join(" "));
            pairs.append_pair("state", state.expose());
            pairs.append_pair("code_challenge", &challenge);
            pairs.append_pair("code_challenge_method", "S256");
        }

        tracing::debug!(
            provider = "google-oauth",
            operation = "authorization_url",
            "built authorization URL"
        );

        Ok(AuthorizationRequest {
            url,
            state,
            code_verifier: verifier,
        })
    }

    /// Compare the callback `state` with the value the request carries.
    pub fn validate_state(
        &self,
        request: &AuthorizationRequest,
        state: &str,
    ) -> Result<(), WirekitError> {
        if constant_time_eq(request.state.expose().as_bytes(), state.as_bytes()) {
            Ok(())
        } else {
            Err(WirekitError::Auth {
                message: String::from(
                    "the callback state does not match the authorization request",
                ),
            })
        }
    }

    /// Exchange an authorization code, using the verifier the request carries.
    pub async fn exchange_code(
        &self,
        code: &str,
        request: &AuthorizationRequest,
    ) -> Result<TokenResponse, WirekitError> {
        if code.trim().is_empty() {
            return Err(WirekitError::Auth {
                message: String::from("the authorization code is empty"),
            });
        }

        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "authorization_code")
            .append_pair("code", code)
            .append_pair("client_id", &self.client_id)
            .append_pair("client_secret", self.client_secret.expose())
            .append_pair("redirect_uri", &self.redirect_uri)
            .append_pair("code_verifier", request.code_verifier.expose())
            .finish();

        self.post_token(body).await
    }

    /// Load a usable token for `key`, refreshing at most once.
    ///
    /// A token inside the expiry margin is refreshed through the store; a missing
    /// token is an [`WirekitError::Auth`] rather than a silent authorization attempt
    /// (AC-005, AC-009).
    pub async fn token_for(
        &self,
        store: &dyn TokenStore,
        key: &str,
    ) -> Result<TokenResponse, WirekitError> {
        let current = store.load(key).ok_or_else(|| WirekitError::Auth {
            message: format!("no token is stored for key {key}"),
        })?;

        if !current.is_expired_at((self.clock.as_ref())(), self.expiry_margin) {
            return Ok(current);
        }

        let refreshed = self.refresh(&current).await?;
        store.save(key, &refreshed);
        Ok(refreshed)
    }

    /// Refresh `expired`, once.
    ///
    /// A record without a refresh token cannot be refreshed: the caller must
    /// re-authorize (AC-006). A provider-issued refresh token replaces the stored one;
    /// otherwise the existing token is carried forward (EC-006).
    pub async fn refresh(&self, expired: &TokenResponse) -> Result<TokenResponse, WirekitError> {
        let refresh_token = expired.refresh_token().ok_or_else(|| WirekitError::Auth {
            message: String::from(
                "the token expired and carries no refresh token; re-authorize the user",
            ),
        })?;

        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "refresh_token")
            .append_pair("refresh_token", refresh_token.expose())
            .append_pair("client_id", &self.client_id)
            .append_pair("client_secret", self.client_secret.expose())
            .finish();

        let mut refreshed = self.post_token(body).await?;
        if refreshed.refresh_token.is_none() {
            refreshed.refresh_token = expired.refresh_token.clone();
        }
        if refreshed.scopes.is_empty() {
            refreshed.scopes = expired.scopes.clone();
        }
        Ok(refreshed)
    }

    /// Revoke the token stored under `key` and remove the record.
    ///
    /// Returns `Ok(false)` when nothing was stored, so a caller can treat
    /// "already gone" as success (AC-007).
    pub async fn revoke_stored(
        &self,
        store: &dyn TokenStore,
        key: &str,
        hint: Option<&str>,
    ) -> Result<bool, WirekitError> {
        let Some(token) = store.load(key) else {
            return Ok(false);
        };

        let target = match token.refresh_token() {
            Some(refresh) => refresh.clone(),
            None => token.access_token().clone(),
        };

        self.revoke(RevokeRequest::new(target, hint)).await?;
        store.delete(key);
        Ok(true)
    }

    /// Revoke a token at Google's revocation endpoint.
    pub async fn revoke(&self, request: RevokeRequest) -> Result<(), WirekitError> {
        let mut pairs = url::form_urlencoded::Serializer::new(String::new());
        pairs.append_pair("token", request.token.expose());
        if let Some(hint) = request.hint() {
            pairs.append_pair("token_type_hint", hint);
        }
        let body = pairs.finish();

        let spec = RequestSpec::post(self.revoke_endpoint.clone())
            .try_header("content-type", "application/x-www-form-urlencoded")?
            .try_header("accept", "application/json")?
            .rate_limit_key("google-oauth")
            .bytes_body(body.into_bytes());

        match self.client.send(spec).await {
            Ok(_) => {
                tracing::debug!(
                    provider = "google-oauth",
                    operation = "revoke",
                    "revoked token"
                );
                Ok(())
            }
            Err(error) => Err(map_send_error(error)),
        }
    }

    /// POST a form body to the token endpoint and decode the result.
    async fn post_token(&self, body: String) -> Result<TokenResponse, WirekitError> {
        let spec = RequestSpec::post(self.token_endpoint.clone())
            .try_header("content-type", "application/x-www-form-urlencoded")?
            .try_header("accept", "application/json")?
            .rate_limit_key("google-oauth")
            .bytes_body(body.into_bytes());

        let raw = self.client.send(spec).await.map_err(map_send_error)?;

        let wire: WireToken =
            serde_json::from_slice(&raw.body).map_err(|error| WirekitError::Decode {
                message: String::from("the token response did not match the expected shape"),
                source: Some(Box::new(error)),
            })?;

        let lifetime = Duration::from_secs(wire.expires_in);
        let now = (self.clock.as_ref())();
        let expires_at = now.checked_add(lifetime).unwrap_or(now);

        let scopes = wire
            .scope
            .map(|joined| {
                joined
                    .split_whitespace()
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();

        tracing::debug!(
            provider = "google-oauth",
            operation = "token",
            expires_in = wire.expires_in,
            "received token"
        );

        Ok(TokenResponse {
            access_token: Secret::new(wire.access_token),
            refresh_token: wire.refresh_token.map(Secret::new),
            expires_at,
            scopes,
        })
    }
}

/// The S256 PKCE challenge for a verifier.
///
/// Exposed so a caller can check its own PKCE implementation against ours.
#[must_use]
pub fn pkce_challenge(verifier: &str) -> String {
    let digest = Sha256::digest(verifier.as_bytes());
    URL_SAFE_NO_PAD.encode(digest)
}

/// Map a provider error document carried by a failed `send` to a typed error.
///
/// An `invalid_grant` or `invalid_client` is an authentication failure the caller
/// must act on; every other provider code keeps its HTTP status (AC-007).
fn map_send_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        message,
        details,
        ..
    } = &error
    else {
        return error;
    };

    let parsed = details
        .as_ref()
        .and_then(|value| serde_json::from_value::<WireError>(value.clone()).ok());

    let Some(provider) = parsed else {
        return error;
    };

    if matches!(provider.error.as_str(), "invalid_grant" | "invalid_client") {
        return WirekitError::Auth {
            message: if provider.error_description.is_empty() {
                provider.error
            } else {
                provider.error_description
            },
        };
    }

    tracing::debug!(
        provider = "google-oauth",
        provider_code = %provider.error,
        "token request rejected"
    );

    WirekitError::api(
        *status,
        provider.error,
        if provider.error_description.is_empty() {
            message.clone()
        } else {
            provider.error_description
        },
    )
}

/// A CSPRNG token of `len` alphanumeric characters.
fn random_token(len: usize) -> String {
    let mut rng = rand::thread_rng();
    (0..len)
        .map(|_| {
            let pick: u8 = rng.sample(Alphanumeric);
            char::from(pick)
        })
        .collect()
}

/// Compare two byte slices without leaking the position of the first difference.
fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut difference = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        difference |= a ^ b;
    }
    difference == 0
}

/// Parse an endpoint, requiring HTTPS or an explicitly typed loopback test server.
///
/// The default endpoints are compile-time constants and must be HTTPS (NFR-004).
fn parse_endpoint(raw: &str) -> Result<url::Url, WirekitError> {
    let parsed = url::Url::parse(raw)
        .map_err(|_| WirekitError::invalid_config(format!("{raw} is not a valid URL")))?;

    let is_https = parsed.scheme() == "https";
    let is_loopback = parsed.scheme() == "http"
        && matches!(
            parsed.host_str(),
            Some("localhost") | Some("127.0.0.1") | Some("[::1]")
        );

    if is_https || is_loopback {
        Ok(parsed)
    } else {
        Err(WirekitError::invalid_config(format!(
            "{raw} must use HTTPS or be a loopback address"
        )))
    }
}
