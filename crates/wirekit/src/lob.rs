//! Lob — direct mail: address verification, postcards, letters, tracking
//! events, and cancellation of a scheduled piece.
//!
//! # Scope
//!
//! Built from `catalogue/18_lob.md`: verify a domestic or international
//! address, create a postcard or a letter, read a mail piece, list its
//! tracking events, and cancel a piece that has not printed yet. Auth, the
//! token bucket, the retry policy with `Retry-After`, tracing, and the
//! response size limit are the shared ones.
//!
//! # Non-goals
//!
//! - **No printing or rendering.** The caller supplies the HTML for the front,
//!   back, or letter file; this module never generates artwork.
//! - **No webhook receiver.** Signature verification and deduplication of
//!   inbound events belong to the caller's HTTP surface; this crate makes
//!   outbound requests only.
//! - **No campaign logic.** Deciding who receives a piece, and reconciling a
//!   returned piece against a prospect record, belongs to the caller.
//! - **No cheque support.** The catalogue lists it as a provider capability;
//!   this feature implements only the surface in the design file.
//!
//! # Test and live keys
//!
//! Lob distinguishes test keys by the `test_` prefix. A test key must be used
//! with [`LobConfig::test_mode`] and a live key must not, so a test mail piece
//! can never be mistaken for a printed one. A mismatch is
//! [`WirekitError::InvalidConfig`] rather than a silent request.
//!
//! # Idempotency and money
//!
//! Creating a mail piece prints it and costs postage, so every create carries
//! an `Idempotency-Key` header. The retry policy therefore cannot produce two
//! pieces for one request; after a timeout the caller reconciles with
//! [`LobClient::get_mail_piece`] using the idempotency key it supplied.
//!
//! ```no_run
//! use wirekit::{LobClient, LobConfig, Secret, WirekitClient};
//!
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! let transport = WirekitClient::new(wirekit::lob::BASE_URL)?;
//! let client = LobClient::new(
//!     transport,
//!     LobConfig::new(Secret::new(String::from("test_xxxxxxxxxxxx"))).with_test_mode(),
//! )?;
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use serde::{Deserialize, Serialize};

use crate::auth::{AuthScheme, BasicAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The Lob API base URL.
pub const BASE_URL: &str = "https://api.lob.com/v1";

/// How the client is configured.
#[derive(Debug, Clone)]
pub struct LobConfig {
    /// The API key, used as the Basic-auth username with an empty password.
    pub api_key: Secret<String>,
    /// Whether the key is a `test_` key.
    pub test_mode: bool,
}

impl LobConfig {
    /// Build a live-mode configuration.
    #[must_use]
    pub fn new(api_key: Secret<String>) -> Self {
        Self {
            api_key,
            test_mode: false,
        }
    }

    /// Treat the key as a Lob test key.
    ///
    /// A key without the `test_` prefix is rejected by [`LobClient::new`].
    #[must_use]
    pub fn with_test_mode(mut self) -> Self {
        self.test_mode = true;
        self
    }
}

/// The kind of mail piece. The provider keeps postcards and letters in
/// separate collections, so the kind is part of every lookup.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MailKind {
    /// A postcard.
    Postcard,
    /// A letter.
    Letter,
}

impl MailKind {
    /// The collection segment for this kind.
    #[must_use]
    pub fn segment(self) -> &'static str {
        match self {
            Self::Postcard => "postcards",
            Self::Letter => "letters",
        }
    }
}

/// A postal address.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Address {
    /// The recipient or sender name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    /// The primary street line.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_line1: Option<String>,
    /// The secondary street line, for example an apartment number.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub address_line2: Option<String>,
    /// The city, town, or locality.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub city: Option<String>,
    /// The state or province.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<String>,
    /// The postal code.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub postal_code: Option<String>,
    /// The two-letter country code. Lob defaults to `US`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub country: Option<String>,
    /// The provider's deliverability verdict, present on a verified address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deliverability: Option<String>,
    /// The provider's identifier for a verified address.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
}

impl Address {
    /// Build an address from its required parts.
    #[must_use]
    pub fn new(
        name: impl Into<String>,
        address_line1: impl Into<String>,
        city: impl Into<String>,
        state: impl Into<String>,
        postal_code: impl Into<String>,
    ) -> Self {
        Self {
            name: Some(name.into()),
            address_line1: Some(address_line1.into()),
            address_line2: None,
            city: Some(city.into()),
            state: Some(state.into()),
            postal_code: Some(postal_code.into()),
            country: Some(String::from("US")),
            deliverability: None,
            id: None,
        }
    }

    /// Whether the provider reports this address deliverable.
    #[must_use]
    pub fn is_deliverable(&self) -> bool {
        matches!(
            self.deliverability.as_deref(),
            Some("deliverable")
                | Some("deliverable_unnecessary_unit")
                | Some("deliverable_incorrect_unit")
                | Some("deliverable_missing_unit")
        )
    }
}

/// The size of a postcard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PostcardSize {
    /// 4x6 inches.
    Size4x6,
    /// 6x9 inches.
    Size6x9,
    /// 6x11 inches.
    Size6x11,
}

impl PostcardSize {
    /// The provider's wire value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Size4x6 => "4x6",
            Self::Size6x9 => "6x9",
            Self::Size6x11 => "6x11",
        }
    }
}

/// A request to create a postcard.
#[derive(Debug, Clone, Default)]
pub struct PostcardRequest {
    /// The recipient.
    pub to: Address,
    /// The sender, which must match the account's return address.
    pub from: Option<Address>,
    /// The HTML printed on the front.
    pub front_html: String,
    /// The HTML printed on the back.
    pub back_html: String,
    /// The card size.
    pub size: Option<PostcardSize>,
    /// An ISO date at which to mail the piece. `None` mails it immediately.
    pub send_date: Option<String>,
}

impl PostcardRequest {
    /// Build a request with the two HTML panels.
    #[must_use]
    pub fn new(to: Address, front_html: impl Into<String>, back_html: impl Into<String>) -> Self {
        Self {
            to,
            from: None,
            front_html: front_html.into(),
            back_html: back_html.into(),
            size: None,
            send_date: None,
        }
    }
}

/// A request to create a letter.
#[derive(Debug, Clone, Default)]
pub struct LetterRequest {
    /// The recipient.
    pub to: Address,
    /// The return address.
    pub from: Option<Address>,
    /// The HTML printed inside the envelope.
    pub file_html: String,
    /// An ISO date at which to mail the piece. `None` mails it immediately.
    pub send_date: Option<String>,
}

impl LetterRequest {
    /// Build a request with the letter body.
    #[must_use]
    pub fn new(to: Address, file_html: impl Into<String>) -> Self {
        Self {
            to,
            from: None,
            file_html: file_html.into(),
            send_date: None,
        }
    }
}

/// A mail piece and its delivery state.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct MailPiece {
    /// The provider's identifier for the piece.
    pub id: String,
    /// Whether this is a postcard or a letter.
    pub kind: Option<MailKind>,
    /// The provider's status, for example `In Transit`.
    pub status: Option<String>,
    /// The expected delivery date, as the provider reports it.
    pub expected_delivery_date: Option<String>,
    /// The tracking events the provider returned with the piece.
    pub events: Vec<MailEvent>,
}

/// One tracking event for a mail piece.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct MailEvent {
    /// The event identifier.
    #[serde(default)]
    pub id: String,
    /// The event name, for example `postcard.delivered`.
    #[serde(default)]
    pub name: String,
    /// When the event occurred.
    #[serde(default)]
    pub date_created: Option<String>,
    /// The resource the event belongs to.
    #[serde(default)]
    pub resource_id: Option<String>,
}

/// The result of cancelling a scheduled piece.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cancellation {
    /// Whether the provider accepted the cancellation.
    pub cancelled: bool,
}

struct LobInner {
    client: WirekitClient,
    config: LobConfig,
    auth: Arc<dyn AuthScheme>,
}

/// The Lob client. Clone it freely; the transport is shared.
#[derive(Clone)]
pub struct LobClient {
    inner: Arc<LobInner>,
}

impl LobClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`]. An empty key, or a key
    /// whose `test_` prefix disagrees with [`LobConfig::test_mode`], is
    /// [`WirekitError::InvalidConfig`].
    pub fn new(client: WirekitClient, config: LobConfig) -> Result<Self, WirekitError> {
        let key = config.api_key.expose().trim().to_owned();
        if key.is_empty() {
            return Err(WirekitError::invalid_config("a Lob API key is required"));
        }
        let looks_like_test = key.starts_with("test_");
        if looks_like_test != config.test_mode {
            return Err(WirekitError::invalid_config(if looks_like_test {
                "a Lob test key needs LobConfig::with_test_mode"
            } else {
                "a live Lob key must not be used in test mode"
            }));
        }

        let auth: Arc<dyn AuthScheme> = Arc::new(BasicAuth::new(key, String::new()));
        Ok(Self {
            inner: Arc::new(LobInner {
                client,
                config,
                auth,
            }),
        })
    }

    /// The client configuration.
    #[must_use]
    pub fn config(&self) -> &LobConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Verify an address with the provider.
    ///
    /// The returned address carries the provider's `deliverability` verdict; use
    /// [`Address::is_deliverable`] rather than matching the string by hand. Lob
    /// answers a malformed address with 422, which is not retried.
    pub async fn verify_address(&self, address: &Address) -> Result<Address, WirekitError> {
        ensure_address(address)?;

        let url = self.inner.client.url("us_verifications")?;
        let spec = RequestSpec::post(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("lob")
            .body_error_hook(reject_error_envelope)
            .json_body(&WireAddressForm::from(address))?;
        let response: Response<Address> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_lob_error)?;

        Ok(response.value)
    }

    /// Create a postcard.
    ///
    /// `idempotency_key` becomes the `Idempotency-Key` header, so a retry of the
    /// same request cannot print a second card. A piece that was accepted is
    /// never retried; reconcile with [`LobClient::get_mail_piece`] instead.
    pub async fn create_postcard(
        &self,
        request: &PostcardRequest,
        idempotency_key: &str,
    ) -> Result<MailPiece, WirekitError> {
        ensure_address(&request.to)?;
        ensure_idempotency_key(idempotency_key)?;
        if request.front_html.trim().is_empty() || request.back_html.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a postcard needs both a front and a back HTML panel",
            ));
        }

        let form = WirePostcardForm {
            to: WireAddressForm::from(&request.to),
            from: request.from.as_ref().map(WireAddressForm::from),
            front: &request.front_html,
            back: &request.back_html,
            size: request.size.map(PostcardSize::as_str),
            send_date: request.send_date.as_deref(),
        };

        let url = self.inner.client.url("postcards")?;
        let spec = RequestSpec::post(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("lob")
            .body_error_hook(reject_error_envelope)
            .try_header("Idempotency-Key", idempotency_key)?
            .json_body(&form)?;
        let response: Response<WireMailPiece> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_lob_error)?;

        Ok(response.value.into_piece(MailKind::Postcard))
    }

    /// Create a letter.
    ///
    /// Carries the same idempotency guarantee as
    /// [`LobClient::create_postcard`].
    pub async fn create_letter(
        &self,
        request: &LetterRequest,
        idempotency_key: &str,
    ) -> Result<MailPiece, WirekitError> {
        ensure_address(&request.to)?;
        ensure_idempotency_key(idempotency_key)?;
        if request.file_html.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a letter needs a file HTML body",
            ));
        }

        let form = WireLetterForm {
            to: WireAddressForm::from(&request.to),
            from: request.from.as_ref().map(WireAddressForm::from),
            file: &request.file_html,
            send_date: request.send_date.as_deref(),
        };

        let url = self.inner.client.url("letters")?;
        let spec = RequestSpec::post(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("lob")
            .body_error_hook(reject_error_envelope)
            .try_header("Idempotency-Key", idempotency_key)?
            .json_body(&form)?;
        let response: Response<WireMailPiece> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_lob_error)?;

        Ok(response.value.into_piece(MailKind::Letter))
    }

    /// Read one mail piece.
    pub async fn get_mail_piece(
        &self,
        kind: MailKind,
        id: &str,
    ) -> Result<MailPiece, WirekitError> {
        ensure_piece_id(id)?;

        let url = self.inner.client.url(&format!("{}/{id}", kind.segment()))?;
        let spec = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("lob")
            .body_error_hook(reject_error_envelope);
        let response: Response<WireMailPiece> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_lob_error)?;

        Ok(response.value.into_piece(kind))
    }

    /// List the tracking events for one mail piece.
    pub async fn list_events(&self, id: &str) -> Result<Vec<MailEvent>, WirekitError> {
        ensure_piece_id(id)?;

        let mut url = self.inner.client.url("events")?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("resource_id", id);
        }
        let spec = RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("lob")
            .body_error_hook(reject_error_envelope);
        let response: Response<WireEventEnvelope> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_lob_error)?;

        Ok(response.value.data)
    }

    /// Cancel a scheduled piece that has not printed yet.
    ///
    /// Lob answers 200 with `{"cancelled": true}`; a piece already in
    /// production comes back as an API error rather than a silent success.
    pub async fn cancel(&self, kind: MailKind, id: &str) -> Result<Cancellation, WirekitError> {
        ensure_piece_id(id)?;

        let url = self.inner.client.url(&format!("{}/{id}", kind.segment()))?;
        let spec = RequestSpec::delete(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("lob")
            .body_error_hook(reject_error_envelope);
        let response: Response<WireCancellation> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_lob_error)?;

        Ok(Cancellation {
            cancelled: response
                .value
                .cancelled
                .unwrap_or(response.value.deleted.unwrap_or(false)),
        })
    }
}

fn ensure_address(address: &Address) -> Result<(), WirekitError> {
    let present = address
        .address_line1
        .as_deref()
        .map(str::trim)
        .is_some_and(|line| !line.is_empty());
    if !present {
        return Err(WirekitError::invalid_config(
            "a Lob address needs a primary street line",
        ));
    }
    Ok(())
}

fn ensure_piece_id(id: &str) -> Result<(), WirekitError> {
    if id.trim().is_empty() {
        return Err(WirekitError::invalid_config(
            "a Lob mail piece id is required",
        ));
    }
    Ok(())
}

fn ensure_idempotency_key(key: &str) -> Result<(), WirekitError> {
    if key.trim().is_empty() {
        return Err(WirekitError::invalid_config(
            "creating a mail piece needs an idempotency key",
        ));
    }
    Ok(())
}

/// Reject a 2xx response whose body carries Lob's `error` envelope (CON-213).
fn reject_error_envelope(status: u16, body: &[u8]) -> Option<WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let error = value.get("error")?;
    let message = error
        .get("message")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Lob reported a failure");
    let code = error
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("lob_error");

    Some(WirekitError::api_with_details(
        status,
        code,
        message,
        value.clone(),
    ))
}

/// Map a Lob rejection onto the shared error type.
///
/// Lob nests the message under `error.message`; it is lifted out so the caller
/// sees the provider's text rather than a generic one. A 401 or 403 becomes an
/// authentication failure.
fn map_lob_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        details: Some(details),
        ..
    } = &error
    else {
        return error;
    };

    if *status == 401 || *status == 403 {
        return WirekitError::auth("Lob rejected the API key");
    }

    let nested = details.get("error");
    let message = nested
        .and_then(|error| error.get("message"))
        .and_then(serde_json::Value::as_str);
    let code = nested
        .and_then(|error| error.get("code"))
        .and_then(serde_json::Value::as_str);

    match (code, message) {
        (Some(code), Some(message)) => {
            WirekitError::api_with_details(*status, code, message, details.clone())
        }
        (None, Some(message)) => {
            WirekitError::api_with_details(*status, "lob_error", message, details.clone())
        }
        _ => error,
    }
}

#[derive(Debug, Serialize)]
struct WireAddressForm<'a> {
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<&'a str>,
    address_line1: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    address_line2: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    city: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    state: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    postal_code: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    country: Option<&'a str>,
}

impl<'a> From<&'a Address> for WireAddressForm<'a> {
    fn from(address: &'a Address) -> Self {
        Self {
            name: address.name.as_deref(),
            address_line1: address.address_line1.as_deref().unwrap_or_default(),
            address_line2: address.address_line2.as_deref(),
            city: address.city.as_deref(),
            state: address.state.as_deref(),
            postal_code: address.postal_code.as_deref(),
            country: address.country.as_deref(),
        }
    }
}

#[derive(Debug, Serialize)]
struct WirePostcardForm<'a> {
    to: WireAddressForm<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<WireAddressForm<'a>>,
    front: &'a str,
    back: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    size: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    send_date: Option<&'a str>,
}

#[derive(Debug, Serialize)]
struct WireLetterForm<'a> {
    to: WireAddressForm<'a>,
    #[serde(skip_serializing_if = "Option::is_none")]
    from: Option<WireAddressForm<'a>>,
    file: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    send_date: Option<&'a str>,
}

#[derive(Debug, Default, Deserialize)]
struct WireMailPiece {
    #[serde(default)]
    id: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    expected_delivery_date: Option<String>,
    #[serde(default)]
    tracking_events: Vec<MailEvent>,
}

impl WireMailPiece {
    fn into_piece(self, kind: MailKind) -> MailPiece {
        MailPiece {
            id: self.id,
            kind: Some(kind),
            status: self.status,
            expected_delivery_date: self.expected_delivery_date,
            events: self.tracking_events,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct WireEventEnvelope {
    #[serde(default)]
    data: Vec<MailEvent>,
}

#[derive(Debug, Default, Deserialize)]
struct WireCancellation {
    #[serde(default)]
    cancelled: Option<bool>,
    #[serde(default)]
    deleted: Option<bool>,
}
