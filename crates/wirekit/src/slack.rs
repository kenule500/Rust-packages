//! Slack: chat messages, channel and user lookups, file upload, and request
//! signature verification.
//!
//! Scope: `chat.postMessage`, `conversations.list`, `conversations.info`,
//! `users.info`, `files.upload`, a short channel cache, and HMAC-SHA256
//! verification of the `X-Slack-Signature` header over the exact received
//! bytes.
//!
//! Non-goals: OAuth installation, Events API subscriptions, Socket Mode, and
//! streaming uploads.
//!
//! Slack answers HTTP 200 for both success and failure. Every response is
//! inspected for `ok`, so a body-reported failure becomes
//! [`WirekitError::Api`] rather than a silent success (CON-213).
//!
//! ```no_run
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! use wirekit::slack::{SlackClient, SlackMessage};
//! use wirekit::WirekitClient;
//!
//! let client = WirekitClient::builder(wirekit::slack::BASE_URL)?.build()?;
//! let slack = SlackClient::builder(client).bot_token("xoxb-...").build()?;
//! let sent = slack.post_message(&SlackMessage::text("C01234567", "Deploy done")).await?;
//! println!("{:?}", sent.ts);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::auth::{constant_time_eq, AuthScheme, BearerAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The Slack Web API base URL.
pub const BASE_URL: &str = "https://slack.com/api";

/// How old a signed request may be before it is rejected.
pub const DEFAULT_MAX_CLOCK_SKEW: Duration = Duration::from_secs(300);

/// The largest file this crate will upload, 1 GiB.
pub const MAX_FILE_BYTES: usize = 1024 * 1024 * 1024;

/// The default channel cache lifetime.
pub const DEFAULT_CHANNEL_CACHE_TTL: Duration = Duration::from_secs(300);

/// Body-reported errors that must never be retried.
const NON_RETRYABLE: &[&str] = &["channel_not_found", "not_in_channel"];

struct SlackInner {
    client: WirekitClient,
    auth: Arc<dyn AuthScheme>,
    signing_secret: Option<Secret<String>>,
    max_clock_skew: Duration,
    channel_cache_ttl: Option<Duration>,
    channels: Mutex<HashMap<String, CachedChannel>>,
}

struct CachedChannel {
    expires_at: Instant,
    channel: SlackChannel,
}

/// The Slack client. Clone it freely; the transport and cache are shared.
#[derive(Clone)]
pub struct SlackClient {
    inner: Arc<SlackInner>,
}

/// The builder for [`SlackClient`].
pub struct SlackClientBuilder {
    client: WirekitClient,
    token: Option<String>,
    signing_secret: Option<String>,
    max_clock_skew: Duration,
    channel_cache_ttl: Option<Duration>,
}

impl SlackClientBuilder {
    /// Set the bot token (`xoxb-...`).
    #[must_use]
    pub fn bot_token(mut self, token: impl Into<String>) -> Self {
        self.token = Some(token.into());
        self
    }

    /// Set the app signing secret, required to verify inbound requests.
    #[must_use]
    pub fn signing_secret(mut self, secret: impl Into<String>) -> Self {
        self.signing_secret = Some(secret.into());
        self
    }

    /// Change how old a signed request may be.
    #[must_use]
    pub fn max_clock_skew(mut self, skew: Duration) -> Self {
        self.max_clock_skew = skew;
        self
    }

    /// Change the channel cache lifetime.
    #[must_use]
    pub fn channel_cache_ttl(mut self, ttl: Duration) -> Self {
        self.channel_cache_ttl = Some(ttl);
        self
    }

    /// Turn the channel cache off.
    #[must_use]
    pub fn disable_channel_cache(mut self) -> Self {
        self.channel_cache_ttl = None;
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<SlackClient, WirekitError> {
        let token = self
            .token
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| WirekitError::invalid_config("a Slack bot token must be configured"))?;

        Ok(SlackClient {
            inner: Arc::new(SlackInner {
                client: self.client,
                auth: Arc::new(BearerAuth::new(token)),
                signing_secret: self.signing_secret.map(Secret::new),
                max_clock_skew: self.max_clock_skew,
                channel_cache_ttl: self.channel_cache_ttl,
                channels: Mutex::new(HashMap::new()),
            }),
        })
    }
}

impl SlackClient {
    /// Start building a Slack client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`].
    #[must_use]
    pub fn builder(client: WirekitClient) -> SlackClientBuilder {
        SlackClientBuilder {
            client,
            token: None,
            signing_secret: None,
            max_clock_skew: DEFAULT_MAX_CLOCK_SKEW,
            channel_cache_ttl: Some(DEFAULT_CHANNEL_CACHE_TTL),
        }
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Drop the cached channels.
    pub fn clear_cache(&self) {
        lock(&self.inner.channels).clear();
    }

    /// Post a message to a channel.
    pub async fn post_message(
        &self,
        message: &SlackMessage,
    ) -> Result<SlackMessageResponse, WirekitError> {
        if message.channel.trim().is_empty() {
            return Err(WirekitError::invalid_config("a Slack channel is required"));
        }
        if message.text.is_none() && message.blocks.is_none() {
            return Err(WirekitError::invalid_config(
                "a Slack message needs text or blocks",
            ));
        }

        let url = self.inner.client.url("chat.postMessage")?;
        let spec = self.decorate(RequestSpec::post(url)).json_body(message)?;
        let response: Response<SlackMessageResponse> = self.inner.client.execute(spec).await?;
        Ok(response.value)
    }

    /// List conversations visible to the token.
    pub async fn conversations_list(
        &self,
        limit: Option<u32>,
        cursor: Option<&str>,
    ) -> Result<ConversationPage, WirekitError> {
        let mut url = self.inner.client.url("conversations.list")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(limit) = limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(cursor) = cursor.filter(|cursor| !cursor.is_empty()) {
                pairs.append_pair("cursor", cursor);
            }
        }

        let spec = self.decorate(RequestSpec::get(url));
        let response: Response<ConversationPage> = self.inner.client.execute(spec).await?;
        Ok(response.value)
    }

    /// Fetch one channel, using the cache when it is warm.
    pub async fn channel_info(&self, channel: &str) -> Result<SlackChannel, WirekitError> {
        let channel = channel.trim();
        if channel.is_empty() {
            return Err(WirekitError::invalid_config("a Slack channel is required"));
        }

        if let Some(cached) = self.cache_get(channel) {
            return Ok(cached);
        }

        let mut url = self.inner.client.url("conversations.info")?;
        url.query_pairs_mut().append_pair("channel", channel);

        let spec = self.decorate(RequestSpec::get(url));
        let response: Response<ConversationEnvelope> = self.inner.client.execute(spec).await?;
        let resolved = response.value.channel;
        self.cache_put(channel, &resolved);
        Ok(resolved)
    }

    /// Fetch one user.
    pub async fn user_info(&self, user: &str) -> Result<SlackUser, WirekitError> {
        let user = user.trim();
        if user.is_empty() {
            return Err(WirekitError::invalid_config("a Slack user is required"));
        }

        let mut url = self.inner.client.url("users.info")?;
        url.query_pairs_mut().append_pair("user", user);

        let spec = self.decorate(RequestSpec::get(url));
        let response: Response<UserEnvelope> = self.inner.client.execute(spec).await?;
        Ok(response.value.user)
    }

    /// Upload a file to one or more channels.
    ///
    /// The multipart body is assembled here so the shared pipeline can still
    /// bound, trace, and retry the request.
    pub async fn upload_file(&self, upload: &FileUpload) -> Result<SlackFile, WirekitError> {
        if upload.channels.is_empty() {
            return Err(WirekitError::invalid_config(
                "a Slack file upload needs at least one channel",
            ));
        }
        if upload.filename.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a Slack file upload needs a filename",
            ));
        }
        if upload.content.len() > MAX_FILE_BYTES {
            return Err(WirekitError::invalid_config(format!(
                "the file is {} bytes, over the {MAX_FILE_BYTES}-byte limit",
                upload.content.len()
            )));
        }

        let boundary = format!("wirekit-boundary-{:016x}", rand::random::<u64>());
        let body = build_multipart(&boundary, upload);

        let url = self.inner.client.url("files.upload")?;
        let spec = self
            .decorate(RequestSpec::post(url))
            .try_header(
                "content-type",
                &format!("multipart/form-data; boundary={boundary}"),
            )?
            .bytes_body(body);

        let response: Response<FileEnvelope> = self.inner.client.execute(spec).await?;
        Ok(response.value.file)
    }

    /// Verify a signed inbound request.
    ///
    /// Slack signs `v0:{timestamp}:{raw body}` with the app signing secret and
    /// sends `X-Slack-Signature: v0=<hex>`. A timestamp outside the allowed
    /// skew is rejected, and a re-serialized body fails because the signature
    /// covers the exact bytes received (CON-215, AC-072).
    pub fn verify_request(
        &self,
        timestamp: &str,
        raw_body: &[u8],
        signature: &str,
    ) -> Result<(), WirekitError> {
        let expected = self.compute_signature(timestamp, raw_body)?;
        if constant_time_eq(expected.as_bytes(), signature.trim().as_bytes()) {
            Ok(())
        } else {
            Err(WirekitError::auth(
                "the Slack request signature did not match",
            ))
        }
    }

    /// Compute the `v0=` signature a Slack request would carry.
    pub fn compute_signature(
        &self,
        timestamp: &str,
        raw_body: &[u8],
    ) -> Result<String, WirekitError> {
        let secret = self.inner.signing_secret.as_ref().ok_or_else(|| {
            WirekitError::invalid_config("request verification requires the app signing secret")
        })?;

        let sent_at: i64 = timestamp.trim().parse().map_err(|_| {
            WirekitError::auth("the Slack request timestamp is not a Unix timestamp")
        })?;
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_secs() as i64);
        if (now - sent_at).unsigned_abs() > self.inner.max_clock_skew.as_secs() {
            return Err(WirekitError::auth(
                "the Slack request timestamp is outside the allowed skew",
            ));
        }

        let body = std::str::from_utf8(raw_body)
            .map_err(|_| WirekitError::auth("the Slack request body is not UTF-8"))?;
        let base = format!("v0:{timestamp}:{body}");

        let mut mac = Hmac::<Sha256>::new_from_slice(secret.expose().as_bytes()).map_err(|_| {
            WirekitError::invalid_config("the signing secret cannot be used as an HMAC key")
        })?;
        mac.update(base.as_bytes());
        Ok(format!("v0={}", hex::encode(mac.finalize().into_bytes())))
    }

    /// Parse an Events API payload.
    pub fn parse_event(raw_body: &[u8]) -> Result<SlackEvent, WirekitError> {
        serde_json::from_slice(raw_body).map_err(|error| {
            WirekitError::decode_with("the Slack event payload did not match", error)
        })
    }

    fn decorate(&self, request: RequestSpec) -> RequestSpec {
        request
            .auth(self.inner.auth.clone())
            .rate_limit_key("slack")
            .non_retryable(NON_RETRYABLE)
            .body_error_hook(slack_error_hook)
    }

    fn cache_get(&self, key: &str) -> Option<SlackChannel> {
        self.inner.channel_cache_ttl?;
        let mut cache = lock(&self.inner.channels);
        let entry = cache.get(key)?;
        if entry.expires_at <= Instant::now() {
            cache.remove(key);
            return None;
        }
        Some(entry.channel.clone())
    }

    fn cache_put(&self, key: &str, channel: &SlackChannel) {
        let Some(ttl) = self.inner.channel_cache_ttl else {
            return;
        };
        lock(&self.inner.channels).insert(
            key.to_string(),
            CachedChannel {
                expires_at: Instant::now() + ttl,
                channel: channel.clone(),
            },
        );
    }
}

/// Turn `ok: false` into an `Api` error, and a rejected token into `Auth`.
fn slack_error_hook(_status: u16, body: &[u8]) -> Option<WirekitError> {
    let probe: SlackProbe = serde_json::from_slice(body).ok()?;
    if probe.ok {
        return None;
    }

    let code = probe.error.unwrap_or_else(|| "unknown_error".to_string());
    if matches!(
        code.as_str(),
        "invalid_auth" | "not_authed" | "token_revoked" | "account_inactive"
    ) {
        return Some(WirekitError::auth(format!(
            "Slack rejected the bot token ({code})"
        )));
    }

    Some(WirekitError::api(
        200,
        code.clone(),
        format!("Slack rejected the request ({code})"),
    ))
}

fn build_multipart(boundary: &str, upload: &FileUpload) -> Vec<u8> {
    let mut body: Vec<u8> = Vec::with_capacity(upload.content.len() + 512);

    push_field(&mut body, boundary, "channels", &upload.channels.join(","));
    push_field(&mut body, boundary, "filename", &upload.filename);
    if let Some(filetype) = upload.filetype.as_deref() {
        push_field(&mut body, boundary, "filetype", filetype);
    }
    if let Some(title) = upload.title.as_deref() {
        push_field(&mut body, boundary, "title", title);
    }

    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!(
            "Content-Disposition: form-data; name=\"file\"; filename=\"{}\"\r\n",
            upload.filename
        )
        .as_bytes(),
    );
    body.extend_from_slice(b"Content-Type: application/octet-stream\r\n\r\n");
    body.extend_from_slice(&upload.content);
    body.extend_from_slice(b"\r\n");
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

    body
}

fn push_field(body: &mut Vec<u8>, boundary: &str, name: &str, value: &str) {
    body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
    body.extend_from_slice(
        format!("Content-Disposition: form-data; name=\"{name}\"\r\n\r\n").as_bytes(),
    );
    body.extend_from_slice(value.as_bytes());
    body.extend_from_slice(b"\r\n");
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// One Block Kit block.
///
/// Serializes as `{"type": ..., ...}` so it can be sent inside a message's
/// `blocks` array without hand-building JSON.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlackBlock {
    /// The block kind, such as `section` or `divider`.
    #[serde(rename = "type")]
    pub block_type: String,
    /// The block payload: text objects, fields, accessory elements, and so on.
    #[serde(default, skip_serializing_if = "serde_json::Value::is_null")]
    pub fields: serde_json::Value,
}

impl SlackBlock {
    /// Build a block from its kind and payload.
    #[must_use]
    pub fn new(block_type: impl Into<String>, fields: serde_json::Value) -> Self {
        Self {
            block_type: block_type.into(),
            fields,
        }
    }
}

/// A message to post.
#[derive(Debug, Clone, Default, Serialize)]
pub struct SlackMessage {
    /// The channel id or name.
    pub channel: String,
    /// The plain-text fallback.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
    /// Block Kit blocks.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blocks: Option<serde_json::Value>,
    /// Reply in this thread.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub thread_ts: Option<String>,
    /// Whether to render links.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unfurl_links: Option<bool>,
}

impl SlackMessage {
    /// Build a plain-text message.
    #[must_use]
    pub fn text(channel: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            channel: channel.into(),
            text: Some(text.into()),
            ..Self::default()
        }
    }

    /// Build a Block Kit message.
    #[must_use]
    pub fn blocks(channel: impl Into<String>, blocks: serde_json::Value) -> Self {
        Self {
            channel: channel.into(),
            blocks: Some(blocks),
            ..Self::default()
        }
    }

    /// Build a Block Kit message from typed blocks.
    #[must_use]
    pub fn typed_blocks(channel: impl Into<String>, blocks: Vec<SlackBlock>) -> Self {
        Self {
            channel: channel.into(),
            blocks: Some(serde_json::to_value(blocks).unwrap_or(serde_json::Value::Null)),
            ..Self::default()
        }
    }

    /// Reply in a thread.
    #[must_use]
    pub fn in_thread(mut self, thread_ts: impl Into<String>) -> Self {
        self.thread_ts = Some(thread_ts.into());
        self
    }
}

/// The result of posting a message.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SlackMessageResponse {
    /// Whether Slack accepted the message.
    #[serde(default)]
    pub ok: bool,
    /// The channel the message landed in.
    #[serde(default)]
    pub channel: Option<String>,
    /// The message timestamp, which is also its id.
    #[serde(default)]
    pub ts: Option<String>,
}

/// A channel.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SlackChannel {
    /// The channel id.
    #[serde(default)]
    pub id: String,
    /// The channel name.
    #[serde(default)]
    pub name: Option<String>,
    /// Whether the channel is private.
    #[serde(default)]
    pub is_private: Option<bool>,
    /// How many members the channel has.
    #[serde(default)]
    pub num_members: Option<u64>,
}

/// One page of conversations.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ConversationPage {
    /// Whether Slack accepted the request.
    #[serde(default)]
    pub ok: bool,
    /// The channels on this page.
    #[serde(default)]
    pub channels: Vec<SlackChannel>,
    /// The cursor for the next page.
    #[serde(default)]
    pub response_metadata: Option<ResponseMetadata>,
}

impl ConversationPage {
    /// The cursor for the next page, when one exists.
    #[must_use]
    pub fn next_cursor(&self) -> Option<&str> {
        self.response_metadata
            .as_ref()
            .and_then(|metadata| metadata.next_cursor.as_deref())
            .filter(|cursor| !cursor.is_empty())
    }
}

/// Pagination metadata.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ResponseMetadata {
    /// The cursor for the next page.
    #[serde(default)]
    pub next_cursor: Option<String>,
}

/// A user.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SlackUser {
    /// The user id.
    #[serde(default)]
    pub id: String,
    /// The handle.
    #[serde(default)]
    pub name: Option<String>,
    /// The display name.
    #[serde(default)]
    pub real_name: Option<String>,
    /// Whether the account is a bot.
    #[serde(default)]
    pub is_bot: Option<bool>,
}

/// A file to upload.
#[derive(Debug, Clone, Default)]
pub struct FileUpload {
    /// The channels to share into.
    pub channels: Vec<String>,
    /// The file bytes.
    pub content: Vec<u8>,
    /// The file name.
    pub filename: String,
    /// The Slack filetype, such as `python`.
    pub filetype: Option<String>,
    /// A display title.
    pub title: Option<String>,
}

impl FileUpload {
    /// Build an upload.
    #[must_use]
    pub fn new(filename: impl Into<String>, content: Vec<u8>) -> Self {
        Self {
            channels: Vec::new(),
            content,
            filename: filename.into(),
            filetype: None,
            title: None,
        }
    }

    /// Share into a channel.
    #[must_use]
    pub fn channel(mut self, channel: impl Into<String>) -> Self {
        self.channels.push(channel.into());
        self
    }

    /// Set the Slack filetype.
    #[must_use]
    pub fn filetype(mut self, filetype: impl Into<String>) -> Self {
        self.filetype = Some(filetype.into());
        self
    }

    /// Set the display title.
    #[must_use]
    pub fn title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

/// The uploaded file.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SlackFile {
    /// The file id.
    #[serde(default)]
    pub id: String,
    /// The file name.
    #[serde(default)]
    pub name: Option<String>,
    /// The display title.
    #[serde(default)]
    pub title: Option<String>,
}

/// An Events API payload.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SlackEvent {
    /// `url_verification`, `event_callback`, and so on.
    #[serde(default, rename = "type")]
    pub envelope_type: String,
    /// The challenge to echo for `url_verification`.
    #[serde(default)]
    pub challenge: Option<String>,
    /// The workspace id.
    #[serde(default)]
    pub team_id: Option<String>,
    /// The inner event, for `event_callback`.
    #[serde(default)]
    pub event: Option<serde_json::Value>,
}

#[derive(Deserialize)]
struct SlackProbe {
    #[serde(default)]
    ok: bool,
    #[serde(default)]
    error: Option<String>,
}

#[derive(Deserialize)]
struct ConversationEnvelope {
    #[serde(default)]
    channel: SlackChannel,
}

#[derive(Deserialize)]
struct UserEnvelope {
    #[serde(default)]
    user: SlackUser,
}

#[derive(Deserialize)]
struct FileEnvelope {
    #[serde(default)]
    file: SlackFile,
}
