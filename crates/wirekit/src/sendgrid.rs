//! SendGrid: transactional email, dynamic templates, attachments, contact
//! lists, and signed event webhooks.
//!
//! Scope: the v3 `/mail/send` path with typed addresses, content, templates,
//! attachments, and sandbox mode; the `X-Message-Id` result; contact-list
//! models; and ECDSA P-256 verification of the Event Webhook over the exact
//! received bytes.
//!
//! Non-goals: deliverability management, marketing-campaign orchestration, and
//! inbound-parse reply handling.
//!
//! ```no_run
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! use wirekit::sendgrid::{EmailAddress, SendEmailRequest, SendGridClient, SendGridConfig};
//! use wirekit::WirekitClient;
//!
//! let client = WirekitClient::builder(wirekit::sendgrid::BASE_URL)?.build()?;
//! let sendgrid = SendGridClient::new(
//!     client,
//!     SendGridConfig::new("SG.api-key", EmailAddress::new("billing@example.com")),
//! )?;
//!
//! let request = SendEmailRequest::new(EmailAddress::new("customer@example.com"))
//!     .subject("Your invoice")
//!     .html("<p>Attached.</p>");
//! let sent = sendgrid.send_email(&request).await?;
//! println!("{:?}", sent.message_id);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use base64::Engine as _;
use ecdsa::signature::Verifier;
use p256::ecdsa::{Signature, VerifyingKey};
use p256::pkcs8::DecodePublicKey;
use serde::{Deserialize, Serialize};

use crate::auth::{AuthScheme, BearerAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::RequestSpec;
use crate::secret::Secret;

/// The SendGrid API base URL.
pub const BASE_URL: &str = "https://api.sendgrid.com/v3";

/// The largest attachment this crate will send, 10 MiB decoded.
pub const MAX_ATTACHMENT_BYTES: usize = 10 * 1024 * 1024;

struct SendGridInner {
    client: WirekitClient,
    config: SendGridConfig,
    auth: Arc<dyn AuthScheme>,
}

/// How the client is configured.
#[derive(Debug, Clone)]
pub struct SendGridConfig {
    /// The API key. It is never logged.
    pub api_key: Secret<String>,
    /// The default sender, used when a request does not set one.
    pub from: EmailAddress,
    /// Validate every send without delivering mail.
    pub sandbox: bool,
}

impl SendGridConfig {
    /// Build a configuration with sandbox mode off.
    #[must_use]
    pub fn new(api_key: impl Into<String>, from: EmailAddress) -> Self {
        Self {
            api_key: Secret::new(api_key.into()),
            from,
            sandbox: false,
        }
    }

    /// Validate sends without delivering them.
    #[must_use]
    pub fn sandbox(mut self, enable: bool) -> Self {
        self.sandbox = enable;
        self
    }
}

/// The SendGrid client. Clone it freely; the transport is shared.
#[derive(Clone)]
pub struct SendGridClient {
    inner: Arc<SendGridInner>,
}

impl SendGridClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`].
    pub fn new(client: WirekitClient, config: SendGridConfig) -> Result<Self, WirekitError> {
        if config.api_key.expose().trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a SendGrid API key is required",
            ));
        }
        if config.from.email.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a SendGrid sender address is required",
            ));
        }

        let auth: Arc<dyn AuthScheme> = Arc::new(BearerAuth::new(config.api_key.expose().clone()));
        Ok(Self {
            inner: Arc::new(SendGridInner {
                client,
                config,
                auth,
            }),
        })
    }

    /// The configured sender and sandbox flag.
    #[must_use]
    pub fn config(&self) -> &SendGridConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Send an email.
    ///
    /// The request is validated before it leaves the process: an empty
    /// recipient list, a missing sender, a missing subject for non-template
    /// mail, or an oversized attachment is [`WirekitError::InvalidConfig`]
    /// (EC-031).
    pub async fn send_email(
        &self,
        request: &SendEmailRequest,
    ) -> Result<SendEmailResponse, WirekitError> {
        if request.to.is_empty() {
            return Err(WirekitError::invalid_config(
                "an email needs at least one recipient",
            ));
        }
        if request
            .to
            .iter()
            .any(|address| address.email.trim().is_empty())
        {
            return Err(WirekitError::invalid_config(
                "every recipient needs a non-empty email address",
            ));
        }

        let from = request
            .from
            .clone()
            .unwrap_or_else(|| self.inner.config.from.clone());
        if from.email.trim().is_empty() {
            return Err(WirekitError::invalid_config("an email needs a sender"));
        }

        let is_template = request
            .template_id
            .as_deref()
            .is_some_and(|id| !id.trim().is_empty());
        if !is_template {
            if request
                .subject
                .as_deref()
                .is_none_or(|subject| subject.trim().is_empty())
            {
                return Err(WirekitError::invalid_config(
                    "a non-template email needs a subject",
                ));
            }
            if request.html.is_none() && request.text.is_none() {
                return Err(WirekitError::invalid_config(
                    "a non-template email needs html or text content",
                ));
            }
        }

        for attachment in &request.attachments {
            if attachment.decoded_len() > MAX_ATTACHMENT_BYTES {
                return Err(WirekitError::invalid_config(format!(
                    "attachment {} is over the {MAX_ATTACHMENT_BYTES}-byte limit",
                    attachment.filename
                )));
            }
        }

        let sandbox = request.sandbox.unwrap_or(self.inner.config.sandbox);

        let mut content = Vec::with_capacity(2);
        if let Some(text) = request.text.as_deref() {
            content.push(WireContent {
                content_type: "text/plain",
                value: text,
            });
        }
        if let Some(html) = request.html.as_deref() {
            content.push(WireContent {
                content_type: "text/html",
                value: html,
            });
        }

        let personalization = WirePersonalization {
            to: &request.to,
            dynamic_template_data: request.dynamic_template_data.as_ref(),
        };

        let mail = WireMail {
            personalizations: vec![personalization],
            from: &from,
            subject: request.subject.as_deref(),
            content,
            template_id: request.template_id.as_deref(),
            attachments: &request.attachments,
            mail_settings: sandbox.then_some(WireMailSettings {
                sandbox_mode: WireSandbox { enable: true },
            }),
        };

        let url = self.inner.client.url("mail/send")?;
        let request_spec = RequestSpec::post(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("sendgrid")
            .json_body(&mail)?;

        let raw = self
            .inner
            .client
            .send(request_spec)
            .await
            .map_err(map_sendgrid_error)?;

        Ok(SendEmailResponse {
            status: raw.status,
            message_id: raw.header("x-message-id").map(str::to_string),
        })
    }

    /// List the account's marketing contact lists.
    pub async fn contact_lists(&self) -> Result<Vec<ContactList>, WirekitError> {
        let url = self.inner.client.url("marketing/lists")?;
        let request = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("sendgrid");
        let response: crate::pipeline::Response<ContactListsEnvelope> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_sendgrid_error)?;
        Ok(response.value.result)
    }

    /// Parse an Event Webhook payload into typed events.
    ///
    /// Every event carries `sg_event_id`, so a receiver can deduplicate
    /// deliveries with [`SendGridEvent::dedupe_key`] (AC-065).
    pub fn parse_events(payload: &[u8]) -> Result<Vec<SendGridEvent>, WirekitError> {
        serde_json::from_slice(payload).map_err(|error| {
            WirekitError::decode_with("the SendGrid event payload did not match", error)
        })
    }

    /// Verify an Event Webhook signature.
    ///
    /// SendGrid signs `timestamp || payload` with ECDSA P-256 over SHA-256 and
    /// sends the base64 DER signature. `public_key_base64` is the verification
    /// key SendGrid shows in the dashboard. Verification runs over the exact
    /// received bytes, so a re-serialized body fails (CON-215, AC-064).
    pub fn verify_event_webhook(
        public_key_base64: &str,
        payload: &[u8],
        signature_base64: &str,
        timestamp: &str,
    ) -> Result<(), WirekitError> {
        verify_event(public_key_base64, payload, signature_base64, timestamp)
    }
}

/// Verify an Event Webhook signature without a client.
pub fn verify_event(
    public_key_base64: &str,
    payload: &[u8],
    signature_base64: &str,
    timestamp: &str,
) -> Result<(), WirekitError> {
    let key_bytes = base64::engine::general_purpose::STANDARD
        .decode(public_key_base64.trim())
        .map_err(|_| WirekitError::auth("the SendGrid public key is not valid base64"))?;

    let key = VerifyingKey::from_public_key_der(&key_bytes)
        .or_else(|_| VerifyingKey::from_sec1_bytes(&key_bytes))
        .map_err(|_| WirekitError::auth("the SendGrid public key is not a P-256 public key"))?;

    let signature_bytes = base64::engine::general_purpose::STANDARD
        .decode(signature_base64.trim())
        .map_err(|_| WirekitError::auth("the SendGrid event signature is not valid base64"))?;

    let signature = Signature::from_der(&signature_bytes)
        .or_else(|_| Signature::from_slice(&signature_bytes))
        .map_err(|_| WirekitError::auth("the SendGrid event signature is not a P-256 signature"))?;

    let mut message = Vec::with_capacity(timestamp.len() + payload.len());
    message.extend_from_slice(timestamp.as_bytes());
    message.extend_from_slice(payload);

    key.verify(&message, &signature)
        .map_err(|_| WirekitError::auth("the SendGrid event signature did not match"))
}

/// One entry of SendGrid's `errors[]` array (AC-063).
///
/// SendGrid reports the failing field as `field`; it is surfaced as `code` so a
/// caller can match on it without knowing the provider's wire names.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SendGridError {
    /// The API field or error code SendGrid blamed.
    #[serde(default, alias = "field")]
    pub code: String,
    /// The human-readable message.
    #[serde(default)]
    pub message: String,
    /// Any extra detail objects returned alongside the error.
    #[serde(default)]
    pub details: Vec<serde_json::Value>,
}

/// Map a SendGrid rejection onto the shared error type.
///
/// SendGrid reports failures as an `errors[]` array. The first entry's `code`
/// becomes the error code and its `message` becomes the message; the whole
/// array is retained in `details` (AC-063).
fn map_sendgrid_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        details: Some(details),
        ..
    } = &error
    else {
        return error;
    };

    if *status == 401 || *status == 403 {
        return WirekitError::auth("SendGrid rejected the API key");
    }

    let errors: Vec<SendGridError> = match details.get("errors") {
        Some(serde_json::Value::Array(entries)) => entries
            .iter()
            .filter_map(|entry| serde_json::from_value(entry.clone()).ok())
            .collect(),
        _ => Vec::new(),
    };

    let Some(first) = errors.first() else {
        return error;
    };

    let code = if first.code.is_empty() {
        "sendgrid_error"
    } else {
        first.code.as_str()
    };
    let message = if first.message.is_empty() {
        "SendGrid rejected the request"
    } else {
        first.message.as_str()
    };

    WirekitError::api_with_details(*status, code, message, details.clone())
}

/// An email address with an optional display name.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EmailAddress {
    /// The address itself.
    pub email: String,
    /// The display name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

impl EmailAddress {
    /// Build an address without a display name.
    #[must_use]
    pub fn new(email: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: None,
        }
    }

    /// Build an address with a display name.
    #[must_use]
    pub fn with_name(email: impl Into<String>, name: impl Into<String>) -> Self {
        Self {
            email: email.into(),
            name: Some(name.into()),
        }
    }
}

/// A base64-encoded attachment.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct EmailAttachment {
    /// The base64 content.
    pub content: String,
    /// The file name shown to the recipient.
    pub filename: String,
    /// The MIME type.
    #[serde(rename = "type", default, skip_serializing_if = "Option::is_none")]
    pub content_type: Option<String>,
    /// `attachment` or `inline`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub disposition: Option<String>,
    /// The content id, for an inline attachment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_id: Option<String>,
}

impl EmailAttachment {
    /// Build an attachment from base64 content.
    #[must_use]
    pub fn new(filename: impl Into<String>, base64_content: impl Into<String>) -> Self {
        Self {
            content: base64_content.into(),
            filename: filename.into(),
            content_type: None,
            disposition: Some("attachment".to_string()),
            content_id: None,
        }
    }

    /// Set the MIME type.
    #[must_use]
    pub fn content_type(mut self, content_type: impl Into<String>) -> Self {
        self.content_type = Some(content_type.into());
        self
    }

    /// Mark the attachment inline and give it a content id.
    #[must_use]
    pub fn inline(mut self, content_id: impl Into<String>) -> Self {
        self.disposition = Some("inline".to_string());
        self.content_id = Some(content_id.into());
        self
    }

    /// The decoded size implied by the base64 length.
    #[must_use]
    pub fn decoded_len(&self) -> usize {
        let padding = self
            .content
            .chars()
            .rev()
            .take_while(|character| *character == '=')
            .count();
        (self.content.len() / 4)
            .saturating_mul(3)
            .saturating_sub(padding)
    }
}

/// A request to send one email.
#[derive(Debug, Clone, Default)]
pub struct SendEmailRequest {
    /// The recipients.
    pub to: Vec<EmailAddress>,
    /// The sender; falls back to the configured default.
    pub from: Option<EmailAddress>,
    /// The subject. Required unless a template supplies it.
    pub subject: Option<String>,
    /// The HTML body.
    pub html: Option<String>,
    /// The plain-text body.
    pub text: Option<String>,
    /// A dynamic template id.
    pub template_id: Option<String>,
    /// The handlebars data for the dynamic template.
    pub dynamic_template_data: Option<serde_json::Value>,
    /// The attachments.
    pub attachments: Vec<EmailAttachment>,
    /// Override the configured sandbox flag.
    pub sandbox: Option<bool>,
}

impl SendEmailRequest {
    /// Build a request for one recipient.
    #[must_use]
    pub fn new(to: EmailAddress) -> Self {
        Self {
            to: vec![to],
            ..Self::default()
        }
    }

    /// Add a recipient.
    #[must_use]
    pub fn with_to(mut self, to: EmailAddress) -> Self {
        self.to.push(to);
        self
    }

    /// Set the sender.
    #[must_use]
    pub fn from(mut self, from: EmailAddress) -> Self {
        self.from = Some(from);
        self
    }

    /// Set the subject.
    #[must_use]
    pub fn subject(mut self, subject: impl Into<String>) -> Self {
        self.subject = Some(subject.into());
        self
    }

    /// Set the HTML body.
    #[must_use]
    pub fn html(mut self, html: impl Into<String>) -> Self {
        self.html = Some(html.into());
        self
    }

    /// Set the plain-text body.
    #[must_use]
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Use a dynamic template with substitutions.
    #[must_use]
    pub fn template(mut self, template_id: impl Into<String>, data: serde_json::Value) -> Self {
        self.template_id = Some(template_id.into());
        self.dynamic_template_data = Some(data);
        self
    }

    /// Attach a file.
    #[must_use]
    pub fn attach(mut self, attachment: EmailAttachment) -> Self {
        self.attachments.push(attachment);
        self
    }

    /// Validate without delivering.
    #[must_use]
    pub fn sandbox(mut self, enable: bool) -> Self {
        self.sandbox = Some(enable);
        self
    }
}

/// The result of a send.
#[derive(Debug, Clone, Default)]
pub struct SendEmailResponse {
    /// The HTTP status SendGrid returned, normally 202.
    pub status: u16,
    /// The `X-Message-Id` header, for delivery tracking.
    pub message_id: Option<String>,
}

/// A marketing contact list.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct ContactList {
    /// The list id.
    #[serde(default)]
    pub id: String,
    /// The display name.
    #[serde(default)]
    pub name: String,
    /// How many contacts the list holds.
    #[serde(default)]
    pub contact_count: Option<u64>,
}

/// One Event Webhook event.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct SendGridEvent {
    /// The event id, stable across retries.
    #[serde(default)]
    pub sg_event_id: String,
    /// The message id that produced the event.
    #[serde(default)]
    pub sg_message_id: Option<String>,
    /// `delivered`, `bounce`, `open`, `click`, `dropped`, `spamreport`, and so
    /// on.
    #[serde(default)]
    pub event: String,
    /// The Unix timestamp SendGrid recorded.
    #[serde(default)]
    pub timestamp: i64,
    /// The recipient.
    #[serde(default)]
    pub email: Option<String>,
    /// Why a delivery failed.
    #[serde(default)]
    pub reason: Option<String>,
    /// The clicked URL, for a click event.
    #[serde(default)]
    pub url: Option<String>,
}

impl SendGridEvent {
    /// The value a receiver deduplicates on.
    #[must_use]
    pub fn dedupe_key(&self) -> &str {
        &self.sg_event_id
    }
}

#[derive(Deserialize)]
struct ContactListsEnvelope {
    #[serde(default)]
    result: Vec<ContactList>,
}

#[derive(Serialize)]
struct WireMail<'a> {
    personalizations: Vec<WirePersonalization<'a>>,
    from: &'a EmailAddress,
    #[serde(skip_serializing_if = "Option::is_none")]
    subject: Option<&'a str>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    content: Vec<WireContent<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    template_id: Option<&'a str>,
    #[serde(skip_serializing_if = "slice_is_empty")]
    attachments: &'a [EmailAttachment],
    #[serde(skip_serializing_if = "Option::is_none")]
    mail_settings: Option<WireMailSettings>,
}

#[derive(Serialize)]
struct WirePersonalization<'a> {
    to: &'a [EmailAddress],
    #[serde(skip_serializing_if = "Option::is_none")]
    dynamic_template_data: Option<&'a serde_json::Value>,
}

#[derive(Serialize)]
struct WireContent<'a> {
    #[serde(rename = "type")]
    content_type: &'a str,
    value: &'a str,
}

#[derive(Serialize)]
struct WireMailSettings {
    sandbox_mode: WireSandbox,
}

#[derive(Serialize)]
struct WireSandbox {
    enable: bool,
}

fn slice_is_empty<T>(slice: &[T]) -> bool {
    slice.is_empty()
}
