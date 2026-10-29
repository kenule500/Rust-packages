//! Twilio: SMS, WhatsApp, voice, Verify (MFA), Lookups, and webhook
//! signature verification.
//!
//! Scope: form-encoded writes to the 2010-04-01 API, Verify v2 challenges,
//! Lookups v2 phone validation, and constant-time webhook signature checking.
//! Basic authentication carries either the account auth token or an API key
//! pair; the auth token is also the webhook signing key.
//!
//! Non-goals: TwiML generation, media upload for MMS, and Conversations. The
//! module sends requests and returns typed results; it does not build call
//! flows.
//!
//! ```no_run
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! use wirekit::twilio::{SendSmsRequest, TwilioClient};
//! use wirekit::WirekitClient;
//!
//! let client = WirekitClient::builder(wirekit::twilio::BASE_URL)?.build()?;
//! let twilio = TwilioClient::builder(client)
//!     .account_sid("AC00000000000000000000000000000000")
//!     .auth_token("your-auth-token")
//!     .build()?;
//!
//! let request = SendSmsRequest::new("+15551234567", "+15557654321", "On my way");
//! let message = twilio.send_sms(&request).await?;
//! println!("{} {}", message.sid, message.status);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use base64::Engine as _;
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha1::Sha1;

use crate::auth::{constant_time_eq, AuthScheme, BasicAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::RequestSpec;
use crate::pipeline::Response;
use crate::secret::Secret;

/// The Twilio API base URL.
pub const BASE_URL: &str = "https://api.twilio.com/2010-04-01";
/// The Twilio Verify v2 base URL.
pub const VERIFY_BASE_URL: &str = "https://verify.twilio.com/v2";
/// The Twilio Lookups v2 base URL.
pub const LOOKUP_BASE_URL: &str = "https://lookups.twilio.com/v2";

/// How the client authenticates.
#[derive(Debug, Clone)]
enum Credential {
    /// The account auth token.
    AuthToken(String),
    /// An API key SID and secret.
    ApiKey { sid: String, secret: String },
}

struct TwilioInner {
    client: WirekitClient,
    account_sid: String,
    auth: Arc<dyn AuthScheme>,
    auth_token: Option<Secret<String>>,
}

impl std::fmt::Debug for TwilioInner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("TwilioInner")
            .field("account_sid", &self.account_sid)
            .field("auth", &self.auth)
            .field("auth_token", &self.auth_token)
            .finish()
    }
}

/// The Twilio client. Clone it freely; the transport is shared.
#[derive(Clone)]
pub struct TwilioClient {
    inner: Arc<TwilioInner>,
}

/// The builder for [`TwilioClient`].
pub struct TwilioClientBuilder {
    client: WirekitClient,
    account_sid: Option<String>,
    credential: Option<Credential>,
}

impl TwilioClientBuilder {
    /// Set the account SID that owns the requests.
    #[must_use]
    pub fn account_sid(mut self, account_sid: impl Into<String>) -> Self {
        self.account_sid = Some(account_sid.into());
        self
    }

    /// Authenticate with the account auth token.
    ///
    /// This is the only credential that can verify webhook signatures.
    #[must_use]
    pub fn auth_token(mut self, auth_token: impl Into<String>) -> Self {
        self.credential = Some(Credential::AuthToken(auth_token.into()));
        self
    }

    /// Authenticate with an API key pair.
    ///
    /// API keys can send requests but cannot verify webhook signatures; add
    /// [`auth_token`](Self::auth_token) as well when receiving webhooks.
    #[must_use]
    pub fn api_key(mut self, key_sid: impl Into<String>, secret: impl Into<String>) -> Self {
        self.credential = Some(Credential::ApiKey {
            sid: key_sid.into(),
            secret: secret.into(),
        });
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<TwilioClient, WirekitError> {
        let account_sid = self
            .account_sid
            .filter(|sid| !sid.trim().is_empty())
            .ok_or_else(|| {
                WirekitError::invalid_config("a Twilio account SID must be configured")
            })?;

        let credential = self.credential.ok_or_else(|| {
            WirekitError::invalid_config(
                "Twilio requires an auth token or an API key pair; call auth_token(..) or api_key(..)",
            )
        })?;

        let (auth, auth_token): (Arc<dyn AuthScheme>, Option<Secret<String>>) = match &credential {
            Credential::AuthToken(token) => (
                Arc::new(BasicAuth::new(account_sid.clone(), token.clone())),
                Some(Secret::new(token.clone())),
            ),
            Credential::ApiKey { sid, secret } => {
                (Arc::new(BasicAuth::new(sid.clone(), secret.clone())), None)
            }
        };

        Ok(TwilioClient {
            inner: Arc::new(TwilioInner {
                client: self.client,
                account_sid,
                auth,
                auth_token,
            }),
        })
    }
}

impl TwilioClient {
    /// Start building a Twilio client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`].
    #[must_use]
    pub fn builder(client: WirekitClient) -> TwilioClientBuilder {
        TwilioClientBuilder {
            client,
            account_sid: None,
            credential: None,
        }
    }

    /// The account SID.
    #[must_use]
    pub fn account_sid(&self) -> &str {
        &self.inner.account_sid
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Send an SMS.
    pub async fn send_sms(
        &self,
        request: &SendSmsRequest,
    ) -> Result<MessageResponse, WirekitError> {
        if request.to.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "an SMS destination is required",
            ));
        }
        if request.from.is_none() && request.messaging_service_sid.is_none() {
            return Err(WirekitError::invalid_config(
                "an SMS needs a sender: set from or messaging_service_sid",
            ));
        }

        let mut form: Vec<(&str, String)> = vec![("To", request.to.clone())];
        if let Some(from) = request.from.as_deref() {
            form.push(("From", from.to_string()));
        }
        if let Some(service) = request.messaging_service_sid.as_deref() {
            form.push(("MessagingServiceSid", service.to_string()));
        }
        form.push(("Body", request.body.clone()));
        if let Some(callback) = request.status_callback.as_deref() {
            form.push(("StatusCallback", callback.to_string()));
        }

        let path = format!("Accounts/{}/Messages.json", self.inner.account_sid);
        self.post_form(&path, &form).await
    }

    /// Send a WhatsApp message. `whatsapp:` is added to either address when it
    /// is missing.
    pub async fn send_whatsapp(
        &self,
        request: &SendWhatsAppRequest,
    ) -> Result<MessageResponse, WirekitError> {
        let to = whatsapp_address(&request.to);
        let from = whatsapp_address(&request.from);

        let form = vec![("To", to), ("From", from), ("Body", request.body.clone())];
        let path = format!("Accounts/{}/Messages.json", self.inner.account_sid);
        self.post_form(&path, &form).await
    }

    /// Place a voice call that plays TwiML.
    pub async fn place_call(&self, request: &CallRequest) -> Result<CallStatus, WirekitError> {
        if request.to.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a call destination is required",
            ));
        }
        if request.from.trim().is_empty() {
            return Err(WirekitError::invalid_config("a call sender is required"));
        }

        let form = vec![
            ("To", request.to.clone()),
            ("From", request.from.clone()),
            ("Twiml", request.twiml.clone()),
        ];
        let path = format!("Accounts/{}/Calls.json", self.inner.account_sid);
        self.post_form(&path, &form).await
    }

    /// Fetch the status of a call by SID.
    pub async fn call_status(&self, call_sid: &str) -> Result<CallStatus, WirekitError> {
        let call_sid = call_sid.trim();
        if call_sid.is_empty() {
            return Err(WirekitError::invalid_config("a call SID is required"));
        }
        let path = format!("Accounts/{}/Calls/{call_sid}.json", self.inner.account_sid);
        self.get_json(&path).await
    }

    /// Start an MFA verification through Verify v2.
    pub async fn start_verification(
        &self,
        request: &MfaChallenge,
    ) -> Result<MfaChallengeResponse, WirekitError> {
        if request.service_sid.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a Verify service SID is required",
            ));
        }
        if request.to.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a verification destination is required",
            ));
        }

        let path = format!(
            "{VERIFY_BASE_URL}/Services/{}/Verifications",
            request.service_sid
        );
        let form = vec![
            ("To", request.to.clone()),
            ("Channel", request.channel.as_str().to_string()),
        ];
        self.post_form(&path, &form).await
    }

    /// Check an MFA verification code through Verify v2.
    pub async fn check_verification(
        &self,
        request: &MfaVerification,
    ) -> Result<MfaVerificationResponse, WirekitError> {
        if request.to.trim().is_empty() || request.code.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a verification destination and code are required",
            ));
        }

        let path = format!(
            "{VERIFY_BASE_URL}/Services/{}/VerificationCheck",
            request.service_sid
        );
        let form = vec![("To", request.to.clone()), ("Code", request.code.clone())];
        self.post_form(&path, &form).await
    }

    /// Look up a phone number through Lookups v2.
    ///
    /// Pass `&["validation"]` for validity, or `&["line_type_intelligence"]`
    /// for carrier data. An empty slice requests the default fields.
    pub async fn lookup(
        &self,
        phone_number: &str,
        fields: &[&str],
    ) -> Result<LookupResult, WirekitError> {
        let phone_number = phone_number.trim();
        if phone_number.is_empty() {
            return Err(WirekitError::invalid_config("a phone number is required"));
        }

        let mut path = format!("{LOOKUP_BASE_URL}/PhoneNumbers/{phone_number}");
        if !fields.is_empty() {
            path.push_str("?Fields=");
            path.push_str(&fields.join(","));
        }

        let response: Response<LookupWire> = self.get(&path).await?;
        Ok(response.value.into_result())
    }

    /// Verify a Twilio webhook signature over the raw request body.
    ///
    /// The canonical string is the full URL followed by every form parameter's
    /// name and value in key order. The body is parsed from the exact bytes
    /// received, never from a re-serialization (CON-215). Requires the account
    /// auth token.
    pub fn verify_webhook(
        &self,
        url: &str,
        raw_body: &[u8],
        signature: &str,
    ) -> Result<(), WirekitError> {
        let token = self.inner.auth_token.as_ref().ok_or_else(|| {
            WirekitError::invalid_config(
                "webhook verification requires the account auth token, not an API key",
            )
        })?;

        let expected = webhook_signature(token.expose(), url, raw_body)?;
        if constant_time_eq(expected.as_bytes(), signature.trim().as_bytes()) {
            Ok(())
        } else {
            Err(WirekitError::auth(
                "the Twilio webhook signature did not match",
            ))
        }
    }

    /// Compute the signature a Twilio webhook would carry.
    ///
    /// This exists for proxies and tests that must produce a valid signature;
    /// production callers only need [`verify_webhook`](Self::verify_webhook).
    pub fn compute_webhook_signature(
        &self,
        url: &str,
        raw_body: &[u8],
    ) -> Result<String, WirekitError> {
        let token = self.inner.auth_token.as_ref().ok_or_else(|| {
            WirekitError::invalid_config(
                "webhook signing requires the account auth token, not an API key",
            )
        })?;
        webhook_signature(token.expose(), url, raw_body)
    }

    async fn get_json<T: DeserializeOwned>(&self, path_or_url: &str) -> Result<T, WirekitError> {
        Ok(self.get(path_or_url).await?.value)
    }

    async fn get<T: DeserializeOwned>(
        &self,
        path_or_url: &str,
    ) -> Result<Response<T>, WirekitError> {
        let url = self.inner.client.url(path_or_url)?;
        let request = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key(self.rate_limit_key());
        self.inner
            .client
            .execute(request)
            .await
            .map_err(map_status_error)
    }

    async fn post_form<T: DeserializeOwned>(
        &self,
        path_or_url: &str,
        form: &[(&str, String)],
    ) -> Result<T, WirekitError> {
        let url = self.inner.client.url(path_or_url)?;
        let encoded = serde_urlencoded::to_string(form).map_err(|error| {
            WirekitError::invalid_config(format!("the form body could not be encoded: {error}"))
        })?;

        let request = RequestSpec::post(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key(self.rate_limit_key())
            .try_header("content-type", "application/x-www-form-urlencoded")?
            .bytes_body(encoded.into_bytes());

        let response = self
            .inner
            .client
            .execute::<T>(request)
            .await
            .map_err(map_status_error)?;
        Ok(response.value)
    }

    fn rate_limit_key(&self) -> String {
        format!("twilio:{}", self.inner.account_sid)
    }
}

fn whatsapp_address(value: &str) -> String {
    if value.starts_with("whatsapp:") {
        value.to_string()
    } else {
        format!("whatsapp:{value}")
    }
}

/// Map a rejected credential to [`WirekitError::Auth`].
fn map_status_error(error: WirekitError) -> WirekitError {
    match &error {
        WirekitError::Api { status: 401, .. } => {
            WirekitError::auth("Twilio rejected the credential")
        }
        WirekitError::Api {
            status: 403, code, ..
        } if code == "20003" => WirekitError::auth("Twilio rejected the credential"),
        _ => error,
    }
}

/// HMAC-SHA1 over the canonical webhook string, base64-encoded.
fn webhook_signature(auth_token: &str, url: &str, raw_body: &[u8]) -> Result<String, WirekitError> {
    let params: Vec<(String, String)> =
        serde_urlencoded::from_bytes(raw_body).map_err(|error| {
            WirekitError::invalid_config(format!("the webhook body is not form-encoded: {error}"))
        })?;

    // Twilio's rule: sort by parameter name, then concatenate name and value
    // in that order, with the full URL as the prefix.
    let mut sorted = params;
    sorted.sort_by(|left, right| left.0.cmp(&right.0));

    let mut canonical = String::from(url);
    for (name, value) in &sorted {
        canonical.push_str(name);
        canonical.push_str(value);
    }

    let mut mac = Hmac::<Sha1>::new_from_slice(auth_token.as_bytes()).map_err(|_| {
        WirekitError::invalid_config("the auth token cannot be used as an HMAC key")
    })?;
    mac.update(canonical.as_bytes());
    Ok(base64::engine::general_purpose::STANDARD.encode(mac.finalize().into_bytes()))
}

/// A request to send an SMS.
#[derive(Debug, Clone)]
pub struct SendSmsRequest {
    /// The destination in E.164 form.
    pub to: String,
    /// The sender number, when not using a messaging service.
    pub from: Option<String>,
    /// The messaging service SID, when not using a sender number.
    pub messaging_service_sid: Option<String>,
    /// The message body.
    pub body: String,
    /// An optional status callback URL.
    pub status_callback: Option<String>,
}

impl SendSmsRequest {
    /// Build a request with a sender number.
    #[must_use]
    pub fn new(to: impl Into<String>, from: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            to: to.into(),
            from: Some(from.into()),
            messaging_service_sid: None,
            body: body.into(),
            status_callback: None,
        }
    }

    /// Send through a messaging service instead of a fixed number.
    #[must_use]
    pub fn via_messaging_service(mut self, service_sid: impl Into<String>) -> Self {
        self.messaging_service_sid = Some(service_sid.into());
        self.from = None;
        self
    }

    /// Ask Twilio to report delivery status to `url`.
    #[must_use]
    pub fn status_callback(mut self, url: impl Into<String>) -> Self {
        self.status_callback = Some(url.into());
        self
    }
}

/// A request to send a WhatsApp message.
#[derive(Debug, Clone)]
pub struct SendWhatsAppRequest {
    /// The destination; `whatsapp:` is added when missing.
    pub to: String,
    /// The sender; `whatsapp:` is added when missing.
    pub from: String,
    /// The message body.
    pub body: String,
}

impl SendWhatsAppRequest {
    /// Build a request.
    #[must_use]
    pub fn new(to: impl Into<String>, from: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            to: to.into(),
            from: from.into(),
            body: body.into(),
        }
    }
}

/// A Twilio message.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MessageResponse {
    /// The message SID.
    #[serde(default)]
    pub sid: String,
    /// `queued`, `sent`, `delivered`, `failed`, and so on.
    #[serde(default)]
    pub status: String,
    /// The destination.
    #[serde(default)]
    pub to: Option<String>,
    /// The sender.
    #[serde(default)]
    pub from: Option<String>,
    /// The body Twilio stored.
    #[serde(default)]
    pub body: Option<String>,
    /// The Twilio error code, when the message failed.
    #[serde(default)]
    pub error_code: Option<i64>,
    /// The human-readable error, when the message failed.
    #[serde(default)]
    pub error_message: Option<String>,
    /// The number of billed segments, as Twilio reports it.
    #[serde(default)]
    pub num_segments: Option<String>,
}

/// A request to place a voice call.
#[derive(Debug, Clone)]
pub struct CallRequest {
    /// The destination.
    pub to: String,
    /// The sender.
    pub from: String,
    /// TwiML the call should execute.
    pub twiml: String,
}

impl CallRequest {
    /// Build a call request.
    #[must_use]
    pub fn new(to: impl Into<String>, from: impl Into<String>, twiml: impl Into<String>) -> Self {
        Self {
            to: to.into(),
            from: from.into(),
            twiml: twiml.into(),
        }
    }
}

/// A call's status.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct CallStatus {
    /// The call SID.
    #[serde(default)]
    pub sid: String,
    /// `queued`, `ringing`, `in-progress`, `completed`, `failed`, and so on.
    #[serde(default)]
    pub status: String,
    /// The call duration, as Twilio reports it.
    #[serde(default)]
    pub duration: Option<String>,
    /// The destination.
    #[serde(default)]
    pub to: Option<String>,
    /// The sender.
    #[serde(default)]
    pub from: Option<String>,
}

/// The channel an MFA challenge uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MfaChannel {
    /// A text message.
    Sms,
    /// A voice call.
    Call,
    /// An email message.
    Email,
    /// A WhatsApp message.
    WhatsApp,
}

impl MfaChannel {
    /// The value Twilio expects.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Sms => "sms",
            Self::Call => "call",
            Self::Email => "email",
            Self::WhatsApp => "whatsapp",
        }
    }
}

/// A request to start an MFA verification.
#[derive(Debug, Clone)]
pub struct MfaChallenge {
    /// The Verify service SID.
    pub service_sid: String,
    /// The destination (phone number or email address).
    pub to: String,
    /// The delivery channel.
    pub channel: MfaChannel,
}

impl MfaChallenge {
    /// Build a challenge request.
    #[must_use]
    pub fn new(service_sid: impl Into<String>, to: impl Into<String>, channel: MfaChannel) -> Self {
        Self {
            service_sid: service_sid.into(),
            to: to.into(),
            channel,
        }
    }
}

/// The result of starting an MFA verification.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MfaChallengeResponse {
    /// The verification SID.
    #[serde(default)]
    pub sid: String,
    /// `pending` or `approved`.
    #[serde(default)]
    pub status: String,
    /// The destination.
    #[serde(default)]
    pub to: Option<String>,
    /// The channel used.
    #[serde(default)]
    pub channel: Option<String>,
}

/// A request to check an MFA code.
#[derive(Debug, Clone)]
pub struct MfaVerification {
    /// The Verify service SID.
    pub service_sid: String,
    /// The destination.
    pub to: String,
    /// The code the recipient received.
    pub code: String,
}

impl MfaVerification {
    /// Build a verification check.
    #[must_use]
    pub fn new(
        service_sid: impl Into<String>,
        to: impl Into<String>,
        code: impl Into<String>,
    ) -> Self {
        Self {
            service_sid: service_sid.into(),
            to: to.into(),
            code: code.into(),
        }
    }
}

/// The result of checking an MFA code.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct MfaVerificationResponse {
    /// `approved` or `pending`.
    #[serde(default)]
    pub status: String,
    /// Whether the code was correct.
    #[serde(default)]
    pub valid: bool,
    /// The destination.
    #[serde(default)]
    pub to: Option<String>,
}

/// The result of a phone-number lookup.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct LookupResult {
    /// The number in E.164 form.
    pub phone_number: String,
    /// Whether Twilio considers the number valid.
    #[serde(default)]
    pub valid: Option<bool>,
    /// The ISO country code.
    #[serde(default)]
    pub country_code: Option<String>,
    /// The national formatting of the number.
    #[serde(default)]
    pub national_format: Option<String>,
    /// The line type, when requested: `mobile`, `landline`, `voip`, and so on.
    #[serde(default)]
    pub line_type: Option<String>,
}

#[derive(Deserialize)]
struct LookupWire {
    #[serde(default)]
    phone_number: String,
    #[serde(default)]
    valid: Option<bool>,
    #[serde(default)]
    country_code: Option<String>,
    #[serde(default)]
    national_format: Option<String>,
    #[serde(default)]
    line_type_intelligence: Option<LineTypeWire>,
}

#[derive(Deserialize)]
struct LineTypeWire {
    #[serde(default, rename = "type")]
    line_type: Option<String>,
}

impl LookupWire {
    fn into_result(self) -> LookupResult {
        LookupResult {
            phone_number: self.phone_number,
            valid: self.valid,
            country_code: self.country_code,
            national_format: self.national_format,
            line_type: self
                .line_type_intelligence
                .and_then(|intelligence| intelligence.line_type),
        }
    }
}
