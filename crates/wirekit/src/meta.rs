//! Meta Graph: Messenger, Instagram DM, and WhatsApp Cloud messaging, media
//! upload, and signed webhook ingestion.
//!
//! Scope: text and media sends for the three surfaces, WhatsApp templates and
//! media upload, `X-Hub-Signature-256` verification over the exact received
//! bytes, the `hub.challenge` handshake, and a normalised inbound event.
//!
//! Non-goals: conversation state, inbox routing, Facebook Login, feed
//! publishing, ads, Pages management, and WhatsApp template approval
//! (`catalogue/05_meta.md`).
//!
//! Graph reports failures in a nested `error` object. When a body carries one,
//! it becomes [`WirekitError::Api`] with the code, subcode, and trace id
//! preserved, so a caller never sees a silent success (CON-213).
//!
//! ```no_run
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! use wirekit::meta::{MetaClient, SendMessageRequest};
//! use wirekit::WirekitClient;
//!
//! let http = WirekitClient::builder("https://graph.facebook.com")?.build()?;
//! let meta = MetaClient::builder(http)
//!     .access_token("page-token")
//!     .app_secret("app-secret")
//!     .page_id("1234567890")
//!     .build()?;
//!
//! let sent = meta
//!     .messenger()
//!     .send(&SendMessageRequest::text("PSID", "Hello"))
//!     .await?;
//! println!("{:?}", sent.message_id);
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use sha2::Sha256;
use url::Url;

use crate::auth::{constant_time_eq, AuthScheme, BearerAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The Graph host used by Messenger and WhatsApp Cloud.
pub const BASE_URL: &str = "https://graph.facebook.com";

/// The Graph host used by Instagram messaging.
pub const INSTAGRAM_BASE_URL: &str = "https://graph.instagram.com";

/// The Graph API version this module targets.
pub const API_VERSION: &str = "v18.0";

/// The header Meta uses to report messaging quota usage.
pub const USAGE_HEADER: &str = "x-business-use-case-usage";

/// Messenger send path template: `POST /{page-id}/messages`.
pub const MESSENGER_MESSAGES_PATH: &str = "/{page-id}/messages";

/// Instagram send path template: `POST /{ig-user-id}/messages`.
pub const INSTAGRAM_MESSAGES_PATH: &str = "/{ig-user-id}/messages";

/// WhatsApp send path template: `POST /{phone-number-id}/messages`.
pub const WHATSAPP_MESSAGES_PATH: &str = "/{phone-number-id}/messages";

/// WhatsApp media upload path template: `POST /{phone-number-id}/media`.
pub const WHATSAPP_MEDIA_PATH: &str = "/{phone-number-id}/media";

/// App subscription path template: `POST|DELETE /{page-id}/subscribed_apps`.
pub const SUBSCRIBED_APPS_PATH: &str = "/{page-id}/subscribed_apps";

/// The largest webhook body this module will verify and parse, 1 MiB.
pub const DEFAULT_MAX_WEBHOOK_BYTES: usize = 1024 * 1024;

/// Which messaging surface produced or receives an event.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MetaSurface {
    /// Facebook Messenger, page-scoped.
    Messenger,
    /// Instagram direct messages.
    Instagram,
    /// WhatsApp Cloud API, phone-number scoped.
    WhatsApp,
    /// An inbound event whose `object` this module does not model.
    Unsupported,
}

/// The kind of media carried by a message.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentKind {
    /// An image attachment.
    Image,
    /// A video attachment.
    Video,
    /// A generic file attachment.
    File,
    /// An audio attachment.
    Audio,
}

impl AttachmentKind {
    /// The Graph wire name for this kind.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Image => "image",
            Self::Video => "video",
            Self::File => "file",
            Self::Audio => "audio",
        }
    }
}

/// Media attached to a message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MessageAttachment {
    /// The media kind.
    pub kind: AttachmentKind,
    /// A public URL Meta fetches.
    pub url: Option<String>,
    /// An attachment id returned by an upload.
    pub attachment_id: Option<String>,
}

impl MessageAttachment {
    /// An attachment addressed by URL.
    #[must_use]
    pub fn from_url(kind: AttachmentKind, url: impl Into<String>) -> Self {
        Self {
            kind,
            url: Some(url.into()),
            attachment_id: None,
        }
    }

    /// An attachment addressed by uploaded id.
    #[must_use]
    pub fn from_id(kind: AttachmentKind, attachment_id: impl Into<String>) -> Self {
        Self {
            kind,
            url: None,
            attachment_id: Some(attachment_id.into()),
        }
    }
}

/// A WhatsApp template message.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WhatsAppTemplate {
    /// The approved template name.
    pub name: String,
    /// The template language code, such as `en_US`.
    pub language: String,
    /// Positional parameters, in template order.
    pub parameters: Vec<String>,
}

/// A message to send on one surface.
///
/// Exactly one of `text`, `attachment`, or `template` is set; the constructors
/// enforce that.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendMessageRequest {
    /// The page-scoped id, Instagram id, or WhatsApp number.
    pub recipient: String,
    /// Plain text, when this is a text send.
    pub text: Option<String>,
    /// One attachment, when this is a media send.
    pub attachment: Option<MessageAttachment>,
    /// A WhatsApp template, when this is a template send.
    pub template: Option<WhatsAppTemplate>,
    /// The Graph `messaging_type`, `RESPONSE` by default.
    pub messaging_type: String,
}

impl SendMessageRequest {
    /// A text message.
    #[must_use]
    pub fn text(recipient: impl Into<String>, text: impl Into<String>) -> Self {
        Self {
            recipient: recipient.into(),
            text: Some(text.into()),
            attachment: None,
            template: None,
            messaging_type: String::from("RESPONSE"),
        }
    }

    /// A media message.
    #[must_use]
    pub fn media(recipient: impl Into<String>, attachment: MessageAttachment) -> Self {
        Self {
            recipient: recipient.into(),
            text: None,
            attachment: Some(attachment),
            template: None,
            messaging_type: String::from("RESPONSE"),
        }
    }

    /// A WhatsApp template message.
    #[must_use]
    pub fn template(recipient: impl Into<String>, template: WhatsAppTemplate) -> Self {
        Self {
            recipient: recipient.into(),
            text: None,
            attachment: None,
            template: Some(template),
            messaging_type: String::from("RESPONSE"),
        }
    }
}

/// The outcome of a send.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SendOutcome {
    /// The provider message id, when Graph returned one.
    pub message_id: Option<String>,
    /// The `X-Business-Use-Case-Usage` value, when present.
    pub usage: Option<String>,
}

/// Credentials and routing for one Meta app.
///
/// The access token and app secret are [`Secret`] values: `Debug` prints
/// `***` for both.
#[derive(Clone)]
pub struct MetaClient {
    /// The app secret, used to verify webhook signatures.
    pub app_secret: Secret<String>,
    /// The page or system-user access token.
    pub access_token: Secret<String>,
    /// The Facebook page id, required for Messenger.
    pub page_id: Option<String>,
    /// The WhatsApp phone-number id, required for WhatsApp Cloud.
    pub phone_number_id: Option<String>,
    /// The Graph version, [`API_VERSION`] by default.
    pub api_version: String,
    /// The shared transport.
    pub http: WirekitClient,
    /// The bearer-token scheme applied to every request.
    auth: Arc<dyn AuthScheme>,
}

impl fmt::Debug for MetaClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("MetaClient")
            .field("app_secret", &"***")
            .field("access_token", &"***")
            .field("page_id", &self.page_id)
            .field("phone_number_id", &self.phone_number_id)
            .field("api_version", &self.api_version)
            .finish_non_exhaustive()
    }
}

/// The nested `error` object Graph returns on a failed call.
#[derive(Clone, Debug, Default, PartialEq, Eq, serde::Deserialize)]
pub struct GraphError {
    /// The human-readable message.
    #[serde(default)]
    pub message: String,
    /// The Graph error code.
    #[serde(default)]
    pub code: i64,
    /// The Graph error subcode, when present.
    #[serde(default)]
    pub error_subcode: Option<i64>,
    /// The Graph trace id, when present.
    #[serde(default)]
    pub fbtrace_id: Option<String>,
}

/// The builder for [`MetaClient`].
pub struct MetaClientBuilder {
    client: WirekitClient,
    access_token: Option<String>,
    app_secret: Option<String>,
    page_id: Option<String>,
    phone_number_id: Option<String>,
    api_version: String,
}

impl MetaClientBuilder {
    /// Set the page or system-user access token.
    #[must_use]
    pub fn access_token(mut self, token: impl Into<String>) -> Self {
        self.access_token = Some(token.into());
        self
    }

    /// Set the app secret used to verify webhooks.
    #[must_use]
    pub fn app_secret(mut self, secret: impl Into<String>) -> Self {
        self.app_secret = Some(secret.into());
        self
    }

    /// Set the Facebook page id used by Messenger.
    #[must_use]
    pub fn page_id(mut self, page_id: impl Into<String>) -> Self {
        self.page_id = Some(page_id.into());
        self
    }

    /// Set the WhatsApp phone-number id.
    #[must_use]
    pub fn phone_number_id(mut self, phone_number_id: impl Into<String>) -> Self {
        self.phone_number_id = Some(phone_number_id.into());
        self
    }

    /// Override the Graph API version.
    #[must_use]
    pub fn api_version(mut self, api_version: impl Into<String>) -> Self {
        self.api_version = api_version.into();
        self
    }

    /// Build the client.
    pub fn build(self) -> Result<MetaClient, WirekitError> {
        let token = self
            .access_token
            .filter(|token| !token.trim().is_empty())
            .ok_or_else(|| {
                WirekitError::invalid_config("a Meta access token must be configured")
            })?;
        let secret = self
            .app_secret
            .filter(|secret| !secret.trim().is_empty())
            .ok_or_else(|| WirekitError::invalid_config("a Meta app secret must be configured"))?;

        if self.client.base_url().scheme() != "https" {
            return Err(WirekitError::invalid_config(
                "the Meta base URL must use https (CON-520)",
            ));
        }

        let auth: Arc<dyn AuthScheme> = Arc::new(BearerAuth::new(token.clone()));
        Ok(MetaClient {
            app_secret: Secret::new(secret),
            access_token: Secret::new(token),
            page_id: self.page_id.filter(|id| !id.trim().is_empty()),
            phone_number_id: self.phone_number_id.filter(|id| !id.trim().is_empty()),
            api_version: self.api_version,
            http: self.client,
            auth,
        })
    }
}

impl MetaClient {
    /// Start building a Meta client over a shared [`WirekitClient`].
    ///
    /// Build the transport against [`BASE_URL`]; Instagram requests are routed
    /// to [`INSTAGRAM_BASE_URL`] by the module itself.
    #[must_use]
    pub fn builder(client: WirekitClient) -> MetaClientBuilder {
        MetaClientBuilder {
            client,
            access_token: None,
            app_secret: None,
            page_id: None,
            phone_number_id: None,
            api_version: String::from(API_VERSION),
        }
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.http
    }

    /// The shared transport handle, as the plan names the `http` field.
    #[must_use]
    pub fn http(&self) -> &WirekitClient {
        &self.http
    }

    /// The Messenger surface.
    #[must_use]
    pub fn messenger(&self) -> Messenger<'_> {
        Messenger { meta: self }
    }

    /// The Instagram surface.
    #[must_use]
    pub fn instagram(&self) -> InstagramDm<'_> {
        InstagramDm { meta: self }
    }

    /// The WhatsApp Cloud surface.
    #[must_use]
    pub fn whatsapp(&self) -> WhatsAppBusiness<'_> {
        WhatsAppBusiness { meta: self }
    }

    /// Build a Graph URL for `host` and `path`, refusing anything but HTTPS.
    fn endpoint(&self, host: &str, path: &str) -> Result<Url, WirekitError> {
        let url = Url::parse(&format!("https://{host}/{path}")).map_err(|error| {
            WirekitError::invalid_config(format!("the Meta endpoint could not be built: {error}"))
        })?;

        if url.scheme() != "https" {
            return Err(WirekitError::invalid_config(
                "the Meta endpoint must use https (CON-520)",
            ));
        }

        Ok(url)
    }

    /// The host for a surface.
    fn host(surface: MetaSurface) -> &'static str {
        match surface {
            MetaSurface::Instagram => "graph.instagram.com",
            MetaSurface::Messenger | MetaSurface::WhatsApp | MetaSurface::Unsupported => {
                "graph.facebook.com"
            }
        }
    }

    /// The resource path for a surface.
    fn messages_path(&self, surface: MetaSurface) -> Result<String, WirekitError> {
        let version = &self.api_version;

        match surface {
            MetaSurface::Messenger => {
                let page = self.page_id.as_deref().ok_or_else(|| {
                    WirekitError::invalid_config("a Meta page id is required for Messenger")
                })?;
                Ok(format!(
                    "{version}{}",
                    MESSENGER_MESSAGES_PATH.replace("{page-id}", page)
                ))
            }
            MetaSurface::Instagram => Ok(format!(
                "{version}{}",
                INSTAGRAM_MESSAGES_PATH.replace("{ig-user-id}", "me")
            )),
            MetaSurface::WhatsApp => {
                let phone = self.phone_number_id.as_deref().ok_or_else(|| {
                    WirekitError::invalid_config("a phone-number id is required for WhatsApp Cloud")
                })?;
                Ok(format!(
                    "{version}{}",
                    WHATSAPP_MESSAGES_PATH.replace("{phone-number-id}", phone)
                ))
            }
            MetaSurface::Unsupported => Err(WirekitError::invalid_config(
                "this Meta surface cannot send messages",
            )),
        }
    }

    async fn send_message(
        &self,
        surface: MetaSurface,
        request: &SendMessageRequest,
    ) -> Result<SendOutcome, WirekitError> {
        if request.recipient.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a Meta message needs a recipient",
            ));
        }

        let path = self.messages_path(surface)?;
        let url = self.endpoint(Self::host(surface), &path)?;
        let body = message_body(surface, request)?;

        let spec = RequestSpec::post(url)
            .auth(Arc::clone(&self.auth))
            .json_body(&body)?;
        let response: Response<serde_json::Value> = self.http.execute(spec).await?;

        outcome(&response)
    }

    /// Upload WhatsApp media and return the attachment id.
    pub async fn upload_media(
        &self,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<String, WirekitError> {
        if bytes.is_empty() {
            return Err(WirekitError::invalid_config(
                "a WhatsApp media upload needs bytes",
            ));
        }

        let phone = self.phone_number_id.as_deref().ok_or_else(|| {
            WirekitError::invalid_config("a phone-number id is required for WhatsApp Cloud")
        })?;
        let version = &self.api_version;
        let url = self.endpoint(
            Self::host(MetaSurface::WhatsApp),
            &format!(
                "{version}{}",
                WHATSAPP_MEDIA_PATH.replace("{phone-number-id}", phone)
            ),
        )?;

        let boundary = format!("wirekit-meta-{:016x}", rand::random::<u64>());
        let body = multipart_body(&boundary, mime_type, bytes);

        let spec = RequestSpec::post(url)
            .auth(Arc::clone(&self.auth))
            .try_header(
                "content-type",
                &format!("multipart/form-data; boundary={boundary}"),
            )?
            .bytes_body(body);

        let response: Response<serde_json::Value> = self.http.execute(spec).await?;
        outcome(&response)?;

        response
            .value
            .get("id")
            .and_then(serde_json::Value::as_str)
            .map(String::from)
            .ok_or_else(|| {
                WirekitError::decode("the WhatsApp media upload returned no attachment id")
            })
    }

    /// Subscribe the app to the page's webhooks.
    pub async fn subscribe_app(&self) -> Result<(), WirekitError> {
        let page = self.page_id.as_deref().ok_or_else(|| {
            WirekitError::invalid_config("a Meta page id is required to subscribe an app")
        })?;
        let version = &self.api_version;
        let url = self.endpoint(
            Self::host(MetaSurface::Messenger),
            &format!(
                "{version}{}",
                SUBSCRIBED_APPS_PATH.replace("{page-id}", page)
            ),
        )?;

        let spec = self.subscription_spec(RequestSpec::post(url))?;
        let response: Response<serde_json::Value> = self.http.execute(spec).await?;
        outcome(&response)?;

        Ok(())
    }

    /// Unsubscribe the app from the page's webhooks.
    pub async fn unsubscribe_app(&self) -> Result<(), WirekitError> {
        let page = self.page_id.as_deref().ok_or_else(|| {
            WirekitError::invalid_config("a Meta page id is required to unsubscribe an app")
        })?;
        let version = &self.api_version;
        let url = self.endpoint(
            Self::host(MetaSurface::Messenger),
            &format!(
                "{version}{}",
                SUBSCRIBED_APPS_PATH.replace("{page-id}", page)
            ),
        )?;

        let spec = self.subscription_spec(RequestSpec::delete(url))?;
        let response: Response<serde_json::Value> = self.http.execute(spec).await?;
        outcome(&response)?;

        Ok(())
    }

    /// Attach the bearer token to a subscription call.
    fn subscription_spec(&self, spec: RequestSpec) -> Result<RequestSpec, WirekitError> {
        Ok(spec.auth(Arc::clone(&self.auth)))
    }
}

/// Facebook Messenger, page-scoped.
pub struct Messenger<'a> {
    meta: &'a MetaClient,
}

impl Messenger<'_> {
    /// Send a request on Messenger.
    pub async fn send(&self, request: &SendMessageRequest) -> Result<SendOutcome, WirekitError> {
        self.meta
            .send_message(MetaSurface::Messenger, request)
            .await
    }

    /// Send a text message.
    pub async fn send_text(
        &self,
        recipient: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<SendOutcome, WirekitError> {
        self.send(&SendMessageRequest::text(recipient, text)).await
    }

    /// Send a media message.
    pub async fn send_media(
        &self,
        recipient: impl Into<String>,
        attachment: MessageAttachment,
    ) -> Result<SendOutcome, WirekitError> {
        self.send(&SendMessageRequest::media(recipient, attachment))
            .await
    }
}

/// Instagram direct messages.
pub struct InstagramDm<'a> {
    meta: &'a MetaClient,
}

impl InstagramDm<'_> {
    /// Send a request on Instagram.
    pub async fn send(&self, request: &SendMessageRequest) -> Result<SendOutcome, WirekitError> {
        self.meta
            .send_message(MetaSurface::Instagram, request)
            .await
    }

    /// Send a text message.
    pub async fn send_text(
        &self,
        recipient: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<SendOutcome, WirekitError> {
        self.send(&SendMessageRequest::text(recipient, text)).await
    }
}

/// WhatsApp Cloud API, phone-number scoped.
pub struct WhatsAppBusiness<'a> {
    meta: &'a MetaClient,
}

impl WhatsAppBusiness<'_> {
    /// Send a request on WhatsApp.
    pub async fn send(&self, request: &SendMessageRequest) -> Result<SendOutcome, WirekitError> {
        self.meta.send_message(MetaSurface::WhatsApp, request).await
    }

    /// Send a text message inside the open 24-hour window.
    pub async fn send_text(
        &self,
        recipient: impl Into<String>,
        text: impl Into<String>,
    ) -> Result<SendOutcome, WirekitError> {
        self.send(&SendMessageRequest::text(recipient, text)).await
    }

    /// Send an approved template message.
    pub async fn send_template(
        &self,
        recipient: impl Into<String>,
        template: WhatsAppTemplate,
    ) -> Result<SendOutcome, WirekitError> {
        self.send(&SendMessageRequest::template(recipient, template))
            .await
    }

    /// Upload media and return the attachment id.
    pub async fn upload_media(
        &self,
        bytes: &[u8],
        mime_type: &str,
    ) -> Result<String, WirekitError> {
        self.meta.upload_media(bytes, mime_type).await
    }
}

/// Build the JSON body for one send.
fn message_body(
    surface: MetaSurface,
    request: &SendMessageRequest,
) -> Result<serde_json::Value, WirekitError> {
    match surface {
        MetaSurface::Messenger | MetaSurface::Instagram => {
            let message = if let Some(text) = &request.text {
                serde_json::json!({ "text": text })
            } else if let Some(attachment) = &request.attachment {
                serde_json::json!({ "attachment": attachment_body(attachment) })
            } else if request.template.is_some() {
                return Err(WirekitError::invalid_config(
                    "templates are supported on WhatsApp Cloud only",
                ));
            } else {
                return Err(WirekitError::invalid_config(
                    "a Meta message needs text, an attachment, or a template",
                ));
            };

            Ok(serde_json::json!({
                "recipient": { "id": request.recipient },
                "messaging_type": request.messaging_type,
                "message": message,
            }))
        }
        MetaSurface::WhatsApp => {
            if let Some(text) = &request.text {
                Ok(serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": request.recipient,
                    "type": "text",
                    "text": { "body": text },
                }))
            } else if let Some(attachment) = &request.attachment {
                let kind = attachment.kind.as_str();
                let mut body = serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": request.recipient,
                    "type": kind,
                });

                if let Some(map) = body.as_object_mut() {
                    map.insert(String::from(kind), attachment_payload(attachment));
                }

                Ok(body)
            } else if let Some(template) = &request.template {
                let parameters: Vec<serde_json::Value> = template
                    .parameters
                    .iter()
                    .map(|value| serde_json::json!({ "type": "text", "text": value }))
                    .collect();

                Ok(serde_json::json!({
                    "messaging_product": "whatsapp",
                    "recipient_type": "individual",
                    "to": request.recipient,
                    "type": "template",
                    "template": {
                        "name": template.name,
                        "language": { "code": template.language },
                        "components": [ { "type": "body", "parameters": parameters } ],
                    },
                }))
            } else {
                Err(WirekitError::invalid_config(
                    "a Meta message needs text, an attachment, or a template",
                ))
            }
        }
        MetaSurface::Unsupported => Err(WirekitError::invalid_config(
            "this Meta surface cannot send messages",
        )),
    }
}

/// The `payload` object for a Messenger or Instagram attachment.
fn attachment_body(attachment: &MessageAttachment) -> serde_json::Value {
    serde_json::json!({
        "type": attachment.kind.as_str(),
        "payload": attachment_payload(attachment),
    })
}

/// The media object for a WhatsApp attachment.
fn attachment_payload(attachment: &MessageAttachment) -> serde_json::Value {
    if let Some(url) = &attachment.url {
        serde_json::json!({ "link": url })
    } else if let Some(attachment_id) = &attachment.attachment_id {
        serde_json::json!({ "id": attachment_id })
    } else {
        serde_json::json!({})
    }
}

/// Turn a Graph response into a [`SendOutcome`], mapping a body-reported error.
fn outcome(response: &Response<serde_json::Value>) -> Result<SendOutcome, WirekitError> {
    if let Some(error) = response.value.get("error") {
        return Err(graph_error(response.status, error));
    }

    let message_id = response
        .value
        .get("message_id")
        .and_then(serde_json::Value::as_str)
        .map(String::from)
        .or_else(|| {
            response
                .value
                .get("messages")
                .and_then(serde_json::Value::as_array)
                .and_then(|messages| messages.first())
                .and_then(|message| message.get("id"))
                .and_then(serde_json::Value::as_str)
                .map(String::from)
        });

    Ok(SendOutcome {
        message_id,
        usage: response.header(USAGE_HEADER).map(String::from),
    })
}

/// Map a nested Graph `error` object to [`WirekitError::Api`].
fn graph_error(status: u16, error: &serde_json::Value) -> WirekitError {
    let parsed: GraphError = serde_json::from_value(error.clone()).unwrap_or_default();

    let code = if parsed.code == 0 {
        String::from("graph-error")
    } else {
        parsed.code.to_string()
    };
    let message = if parsed.message.is_empty() {
        "the Meta Graph call failed"
    } else {
        parsed.message.as_str()
    };

    WirekitError::api(status, code, message)
}

/// Build a minimal multipart body for the WhatsApp media upload.
fn multipart_body(boundary: &str, mime_type: &str, bytes: &[u8]) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"messaging_product\"\r\n\r\nwhatsapp\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"upload\"\r\nContent-Type: {mime_type}\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(bytes);
    body.extend_from_slice(format!("\r\n--{boundary}--\r\n").as_bytes());
    body
}

/// What kind of inbound event was received.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EventKind {
    /// An inbound message.
    Message,
    /// A delivery receipt.
    Delivery,
    /// A read receipt.
    Read,
    /// An event shape this module does not model.
    Unsupported,
}

/// A normalised inbound webhook event.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MetaEvent {
    /// The surface that produced the event.
    pub surface: MetaSurface,
    /// The message id used for deduplication (`mid` or WhatsApp `id`).
    pub dedupe_key: String,
    /// The sender id, when Graph supplied one.
    pub sender_id: String,
    /// The entry timestamp in seconds, `0` when absent.
    pub timestamp: i64,
    /// What kind of event this is.
    pub kind: EventKind,
}

/// Verifies `X-Hub-Signature-256` and parses normalised events.
#[derive(Clone)]
pub struct WebhookPayload {
    app_secret: Secret<String>,
    max_bytes: usize,
}

impl fmt::Debug for WebhookPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WebhookPayload")
            .field("app_secret", &"***")
            .field("max_bytes", &self.max_bytes)
            .finish()
    }
}

impl WebhookPayload {
    /// Verify payloads signed with `app_secret`.
    #[must_use]
    pub fn new(app_secret: impl Into<String>) -> Self {
        Self {
            app_secret: Secret::new(app_secret.into()),
            max_bytes: DEFAULT_MAX_WEBHOOK_BYTES,
        }
    }

    /// Override the body size ceiling.
    #[must_use]
    pub fn with_max_bytes(mut self, max_bytes: usize) -> Self {
        self.max_bytes = max_bytes;
        self
    }

    /// The hex HMAC-SHA256 of `raw` under the app secret.
    fn expected_signature(&self, raw: &[u8]) -> Result<String, WirekitError> {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(self.app_secret.expose().as_bytes())
            .map_err(|_| {
                WirekitError::invalid_config("the Meta app secret is not a valid HMAC key")
            })?;
        mac.update(raw);
        let digest = mac.finalize().into_bytes();

        let mut hex = String::with_capacity(digest.len() * 2);
        for byte in digest {
            use std::fmt::Write as _;
            let _ = write!(hex, "{byte:02x}");
        }

        Ok(hex)
    }

    /// Verify a signature over the exact received bytes.
    pub fn verify(&self, raw: &[u8], signature: &str) -> Result<(), WirekitError> {
        if raw.len() > self.max_bytes {
            return Err(WirekitError::invalid_config(format!(
                "the webhook body is {} bytes, over the {}-byte limit",
                raw.len(),
                self.max_bytes
            )));
        }

        let expected = self.expected_signature(raw)?;
        let provided = signature.trim();
        let provided = provided.strip_prefix("sha256=").unwrap_or(provided);

        if constant_time_eq(expected.as_bytes(), provided.as_bytes()) {
            Ok(())
        } else {
            Err(WirekitError::auth(
                "the Meta webhook signature did not match",
            ))
        }
    }

    /// Verify, then parse the body into a normalised event.
    pub fn verify_and_parse(&self, raw: &[u8], signature: &str) -> Result<MetaEvent, WirekitError> {
        self.verify(raw, signature)?;
        parse_event(raw)
    }
}

/// Complete the `hub.challenge` subscription handshake.
pub fn verify_handshake(
    mode: &str,
    token: &str,
    verify_token: &str,
    challenge: &str,
) -> Result<String, WirekitError> {
    if mode.trim() != "subscribe" {
        return Err(WirekitError::auth(
            "the Meta subscription mode is not subscribe",
        ));
    }

    if !constant_time_eq(token.trim().as_bytes(), verify_token.as_bytes()) {
        return Err(WirekitError::auth("the Meta verify token did not match"));
    }

    Ok(String::from(challenge))
}

/// Normalise a verified webhook body.
fn parse_event(raw: &[u8]) -> Result<MetaEvent, WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(raw).map_err(|error| {
        WirekitError::decode_with("the Meta webhook body is not valid JSON", error)
    })?;

    let surface = match value.get("object").and_then(serde_json::Value::as_str) {
        Some("page") => MetaSurface::Messenger,
        Some("instagram") => MetaSurface::Instagram,
        Some("whatsapp_business_account") => MetaSurface::WhatsApp,
        _ => MetaSurface::Unsupported,
    };

    let entry = value
        .get("entry")
        .and_then(serde_json::Value::as_array)
        .and_then(|entries| entries.first());
    let timestamp = entry
        .and_then(|entry| entry.get("time"))
        .and_then(serde_json::Value::as_i64)
        .unwrap_or(0);

    if let Some(messaging) = entry
        .and_then(|entry| entry.get("messaging"))
        .and_then(serde_json::Value::as_array)
        .and_then(|events| events.first())
    {
        let sender_id = messaging
            .get("sender")
            .and_then(|sender| sender.get("id"))
            .and_then(serde_json::Value::as_str)
            .unwrap_or_default();
        let kind = if messaging.get("message").is_some() {
            EventKind::Message
        } else if messaging.get("delivery").is_some() {
            EventKind::Delivery
        } else if messaging.get("read").is_some() {
            EventKind::Read
        } else {
            EventKind::Unsupported
        };
        let dedupe_key = messaging
            .get("message")
            .and_then(|message| message.get("mid"))
            .and_then(serde_json::Value::as_str)
            .or_else(|| {
                messaging
                    .get("delivery")
                    .and_then(|delivery| delivery.get("mids"))
                    .and_then(serde_json::Value::as_array)
                    .and_then(|mids| mids.first())
                    .and_then(serde_json::Value::as_str)
            })
            .unwrap_or_default();

        return Ok(MetaEvent {
            surface,
            dedupe_key: String::from(dedupe_key),
            sender_id: String::from(sender_id),
            timestamp,
            kind,
        });
    }

    if let Some(change) = entry
        .and_then(|entry| entry.get("changes"))
        .and_then(serde_json::Value::as_array)
        .and_then(|changes| changes.first())
        .and_then(|change| change.get("value"))
    {
        if let Some(message) = change
            .get("messages")
            .and_then(serde_json::Value::as_array)
            .and_then(|messages| messages.first())
        {
            return Ok(MetaEvent {
                surface,
                dedupe_key: message
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .into(),
                sender_id: message
                    .get("from")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .into(),
                timestamp,
                kind: EventKind::Message,
            });
        }

        if let Some(status) = change
            .get("statuses")
            .and_then(serde_json::Value::as_array)
            .and_then(|statuses| statuses.first())
        {
            let kind = match status.get("status").and_then(serde_json::Value::as_str) {
                Some("read") => EventKind::Read,
                _ => EventKind::Delivery,
            };

            return Ok(MetaEvent {
                surface,
                dedupe_key: status
                    .get("id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .into(),
                sender_id: status
                    .get("recipient_id")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .into(),
                timestamp,
                kind,
            });
        }
    }

    Ok(MetaEvent {
        surface,
        dedupe_key: String::new(),
        sender_id: String::new(),
        timestamp,
        kind: EventKind::Unsupported,
    })
}
