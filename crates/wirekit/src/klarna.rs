//! Klarna — consumer financing: checkout sessions, authorizations, orders,
//! captures, and refunds.
//!
//! # Scope
//!
//! Built from `catalogue/25_klarna.md`: create and read a checkout session,
//! turn an authorization into an order, read an order and its lines, capture a
//! payment, and refund it, against the merchant's region. Auth, the token
//! bucket, the retry policy with `Retry-After`, tracing, and the response size
//! limit are the shared ones.
//!
//! # Non-goals
//!
//! - **No card data.** The homeowner completes Klarna's hosted flow; this
//!   module never sees, sends, or stores a card number. That is the point of the
//!   redirect.
//! - **No webhook receiver or signature verifier.** The catalogue's webhook
//!   method does not name a signature scheme, and this crate does not guess one.
//!   The caller owns the HTTP surface, verifies the callback per its Klarna
//!   agreement, deduplicates by event id, and reconciles with
//!   [`KlarnaClient::get_order`].
//! - **No capture retry after a timeout.** A capture may have succeeded even
//!   when its response was lost, so the module sends it once and leaves
//!   reconciliation to [`KlarnaClient::get_order`] and its `captured_amount`.
//! - **No financing decisions.** A declined application is a business outcome
//!   the caller reports; it is not retried and not modelled as a transport
//!   failure.
//!
//! # Region
//!
//! Klarna serves separate hosts per region, so [`KlarnaConfig`] carries a
//! [`KlarnaRegion`] and every request is built from that host. The credential is
//! a Basic-auth merchant pair, held in [`Secret`]s and rendered `***`.
//!
//! ```no_run
//! use wirekit::klarna::{CheckoutSessionRequest, KlarnaClient, KlarnaConfig, KlarnaRegion, OrderLine};
//! use wirekit::{Secret, WirekitClient};
//!
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! let transport = WirekitClient::new("https://api-na.klarna.com")?;
//! let client = KlarnaClient::new(
//!     transport,
//!     KlarnaConfig::new(
//!         Secret::new(String::from("<merchant-id>")),
//!         Secret::new(String::from("<merchant-password>")),
//!         KlarnaRegion::NorthAmerica,
//!     ),
//! )?;
//!
//! let request = CheckoutSessionRequest::new("US", "USD", "en-US", 120_000_i64)
//!     .with_order_line(OrderLine::new("Roof replacement", 1, 120_000_i64, 120_000_i64));
//! let session = client.create_session(&request).await?;
//! println!("{}", session.session_id);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use http::Method;
use serde::{Deserialize, Serialize};

use crate::auth::{AuthScheme, BasicAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The European Payments API base URL.
pub const BASE_URL: &str = "https://api.klarna.com";

/// The North American Payments API base URL.
pub const NORTH_AMERICA_BASE_URL: &str = "https://api-na.klarna.com";

/// The Oceania Payments API base URL.
pub const OCEANIA_BASE_URL: &str = "https://api-oc.klarna.com";

/// The merchant's Klarna region.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KlarnaRegion {
    /// Europe.
    Europe,
    /// North America.
    NorthAmerica,
    /// Oceania.
    Oceania,
}

impl KlarnaRegion {
    /// The API host for this region.
    #[must_use]
    pub fn base_url(self) -> &'static str {
        match self {
            Self::Europe => BASE_URL,
            Self::NorthAmerica => NORTH_AMERICA_BASE_URL,
            Self::Oceania => OCEANIA_BASE_URL,
        }
    }
}

/// How the client is configured.
#[derive(Debug, Clone)]
pub struct KlarnaConfig {
    /// The merchant id, used as the Basic-auth username.
    pub username: Secret<String>,
    /// The merchant password, used as the Basic-auth password.
    pub password: Secret<String>,
    /// The region whose host serves the merchant.
    pub region: KlarnaRegion,
}

impl KlarnaConfig {
    /// Build a configuration for a region.
    #[must_use]
    pub fn new(username: Secret<String>, password: Secret<String>, region: KlarnaRegion) -> Self {
        Self {
            username,
            password,
            region,
        }
    }
}

/// One line of an order or a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct OrderLine {
    /// The line's display name.
    pub name: String,
    /// How many units.
    #[serde(default)]
    pub quantity: u32,
    /// The price of one unit, in minor units of the currency.
    #[serde(default)]
    pub unit_price: i64,
    /// The line total, in minor units of the currency.
    #[serde(default)]
    pub total_amount: i64,
    /// The tax rate in basis points, for example `2500` for 25%.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tax_rate: Option<i64>,
    /// The caller's reference, for example an invoice line id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
    /// A product image URL.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub image_url: Option<String>,
}

impl OrderLine {
    /// Build a line from its name, quantity, unit price, and total.
    #[must_use]
    pub fn new(name: impl Into<String>, quantity: u32, unit_price: i64, total_amount: i64) -> Self {
        Self {
            name: name.into(),
            quantity,
            unit_price,
            total_amount,
            tax_rate: None,
            reference: None,
            image_url: None,
        }
    }

    /// Set the tax rate in basis points.
    #[must_use]
    pub fn with_tax_rate(mut self, tax_rate: i64) -> Self {
        self.tax_rate = Some(tax_rate);
        self
    }

    /// Set the caller's reference.
    #[must_use]
    pub fn with_reference(mut self, reference: impl Into<String>) -> Self {
        self.reference = Some(reference.into());
        self
    }
}

/// The URLs Klarna returns the homeowner to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct MerchantUrls {
    /// The page shown after a completed purchase.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub confirmation: Option<String>,
    /// The caller's webhook endpoint.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub notification: Option<String>,
    /// The page shown when the flow is cancelled.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub push: Option<String>,
}

impl MerchantUrls {
    /// Build the three URLs.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the confirmation page.
    #[must_use]
    pub fn with_confirmation(mut self, url: impl Into<String>) -> Self {
        self.confirmation = Some(url.into());
        self
    }

    /// Set the notification endpoint.
    #[must_use]
    pub fn with_notification(mut self, url: impl Into<String>) -> Self {
        self.notification = Some(url.into());
        self
    }

    /// Set the cancellation page.
    #[must_use]
    pub fn with_push(mut self, url: impl Into<String>) -> Self {
        self.push = Some(url.into());
        self
    }
}

/// A request to create a checkout session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CheckoutSessionRequest {
    /// The two-letter purchase country, for example `US`.
    pub purchase_country: String,
    /// The three-letter purchase currency, for example `USD`.
    pub purchase_currency: String,
    /// The locale, for example `en-US`.
    pub locale: String,
    /// The total, in minor units of the currency.
    pub order_amount: i64,
    /// The tax portion of the total, in minor units.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_tax_amount: Option<i64>,
    /// The lines that make up the total.
    pub order_lines: Vec<OrderLine>,
    /// Where Klarna returns the homeowner.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub merchant_urls: Option<MerchantUrls>,
}

impl CheckoutSessionRequest {
    /// Build a session request with the four required fields.
    #[must_use]
    pub fn new(
        purchase_country: impl Into<String>,
        purchase_currency: impl Into<String>,
        locale: impl Into<String>,
        order_amount: i64,
    ) -> Self {
        Self {
            purchase_country: purchase_country.into(),
            purchase_currency: purchase_currency.into(),
            locale: locale.into(),
            order_amount,
            order_tax_amount: None,
            order_lines: Vec::new(),
            merchant_urls: None,
        }
    }

    /// Add an order line.
    #[must_use]
    pub fn with_order_line(mut self, line: OrderLine) -> Self {
        self.order_lines.push(line);
        self
    }

    /// Set the tax portion.
    #[must_use]
    pub fn with_order_tax_amount(mut self, amount: i64) -> Self {
        self.order_tax_amount = Some(amount);
        self
    }

    /// Set the return URLs.
    #[must_use]
    pub fn with_merchant_urls(mut self, urls: MerchantUrls) -> Self {
        self.merchant_urls = Some(urls);
        self
    }
}

/// A payment method Klarna offers for a session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct PaymentMethodCategory {
    /// The provider's identifier, for example `pay_later`.
    #[serde(default)]
    pub identifier: Option<String>,
    /// The display name.
    #[serde(default)]
    pub name: Option<String>,
    /// The artwork URLs, kept as the provider sent them.
    #[serde(default)]
    pub asset_urls: Option<serde_json::Value>,
}

/// A checkout session.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct CheckoutSession {
    /// The provider's identifier for the session.
    #[serde(default)]
    pub session_id: String,
    /// The token the client-side widget needs.
    #[serde(default)]
    pub client_token: Option<String>,
    /// The methods this session can offer.
    #[serde(default)]
    pub payment_method_categories: Vec<PaymentMethodCategory>,
}

/// An order created from an authorization.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct CreatedOrder {
    /// The provider's identifier for the order.
    #[serde(default)]
    pub order_id: String,
    /// The order state.
    #[serde(default)]
    pub status: Option<String>,
    /// The fraud decision, for example `ACCEPTED`.
    #[serde(default)]
    pub fraud_status: Option<String>,
    /// Where the homeowner continues, when the flow is not finished.
    #[serde(default)]
    pub redirect_url: Option<String>,
}

/// An order and its money movements.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Order {
    /// The provider's identifier for the order.
    #[serde(default)]
    pub order_id: String,
    /// The order state.
    #[serde(default)]
    pub status: Option<String>,
    /// The fraud decision, for example `ACCEPTED`.
    #[serde(default)]
    pub fraud_status: Option<String>,
    /// The amount as the order was placed, in minor units.
    #[serde(default)]
    pub order_amount: Option<i64>,
    /// The original amount before adjustments, in minor units.
    #[serde(default)]
    pub original_order_amount: Option<i64>,
    /// The amount captured so far, in minor units. Reconcile a timed-out
    /// capture against this field before trying again.
    #[serde(default)]
    pub captured_amount: Option<i64>,
    /// The amount refunded so far, in minor units.
    #[serde(default)]
    pub refunded_amount: Option<i64>,
    /// The currency, for example `USD`.
    #[serde(default)]
    pub purchase_currency: Option<String>,
    /// The order lines.
    #[serde(default)]
    pub order_lines: Vec<OrderLine>,
    /// The reference Klarna shows the homeowner.
    #[serde(default)]
    pub klarna_reference: Option<String>,
    /// When the order was created, as the provider formats it.
    #[serde(default)]
    pub created_at: Option<String>,
}

/// A request to capture a payment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct CaptureRequest {
    /// The amount to capture, in minor units.
    pub captured_amount: i64,
    /// A note stored with the capture.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The lines being captured. `None` captures the whole order.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_lines: Option<Vec<OrderLine>>,
    /// The caller's reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl CaptureRequest {
    /// Build a full capture for an amount.
    #[must_use]
    pub fn new(captured_amount: i64) -> Self {
        Self {
            captured_amount,
            description: None,
            order_lines: None,
            reference: None,
        }
    }

    /// Attach a note.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Capture only these lines.
    #[must_use]
    pub fn with_order_lines(mut self, lines: Vec<OrderLine>) -> Self {
        self.order_lines = Some(lines);
        self
    }
}

/// The provider's acceptance of a capture.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Capture {
    /// The provider's identifier for the capture.
    #[serde(default)]
    pub capture_id: String,
}

/// A request to refund a payment.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct RefundRequest {
    /// The amount to refund, in minor units.
    pub refunded_amount: i64,
    /// A note stored with the refund.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// The lines being refunded. `None` refunds the whole amount.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub order_lines: Option<Vec<OrderLine>>,
    /// The caller's reference.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

impl RefundRequest {
    /// Build a full refund for an amount.
    #[must_use]
    pub fn new(refunded_amount: i64) -> Self {
        Self {
            refunded_amount,
            description: None,
            order_lines: None,
            reference: None,
        }
    }

    /// Attach a note.
    #[must_use]
    pub fn with_description(mut self, description: impl Into<String>) -> Self {
        self.description = Some(description.into());
        self
    }

    /// Refund only these lines.
    #[must_use]
    pub fn with_order_lines(mut self, lines: Vec<OrderLine>) -> Self {
        self.order_lines = Some(lines);
        self
    }
}

/// The provider's acceptance of a refund.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Refund {
    /// The provider's identifier for the refund.
    #[serde(default)]
    pub refund_id: String,
}

struct KlarnaInner {
    client: WirekitClient,
    config: KlarnaConfig,
    auth: Arc<dyn AuthScheme>,
}

/// The Klarna client. Clone it freely; the transport is shared.
#[derive(Clone)]
pub struct KlarnaClient {
    inner: Arc<KlarnaInner>,
}

impl KlarnaClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// The transport may be built against any host; every request uses the host
    /// for the configured [`KlarnaRegion`]. An empty merchant id or password is
    /// [`WirekitError::InvalidConfig`].
    pub fn new(client: WirekitClient, config: KlarnaConfig) -> Result<Self, WirekitError> {
        if config.username.expose().trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a Klarna merchant id is required",
            ));
        }
        if config.password.expose().trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a Klarna merchant password is required",
            ));
        }

        let auth: Arc<dyn AuthScheme> = Arc::new(BasicAuth::new(
            config.username.expose().clone(),
            config.password.expose().clone(),
        ));
        Ok(Self {
            inner: Arc::new(KlarnaInner {
                client,
                config,
                auth,
            }),
        })
    }

    /// The client configuration.
    #[must_use]
    pub fn config(&self) -> &KlarnaConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Create a checkout session.
    pub async fn create_session(
        &self,
        request: &CheckoutSessionRequest,
    ) -> Result<CheckoutSession, WirekitError> {
        ensure_field(&request.purchase_country, "purchase country")?;
        ensure_field(&request.purchase_currency, "purchase currency")?;
        ensure_field(&request.locale, "locale")?;
        if request.order_amount < 0 {
            return Err(WirekitError::invalid_config(
                "a Klarna order amount must not be negative",
            ));
        }
        if request.order_lines.is_empty() {
            return Err(WirekitError::invalid_config(
                "a Klarna session needs at least one order line",
            ));
        }
        ensure_lines(&request.order_lines)?;

        let url = self.url("payments/v1/sessions")?;
        let spec = self.request(Method::POST, url).json_body(request)?;
        let response: Response<CheckoutSession> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_klarna_error)?;

        Ok(response.value)
    }

    /// Read a checkout session.
    pub async fn get_session(&self, session_id: &str) -> Result<CheckoutSession, WirekitError> {
        ensure_identifier(session_id, "session")?;

        let url = self.url(&format!("payments/v1/sessions/{}", session_id.trim()))?;
        let spec = self.request(Method::GET, url);
        let response: Response<CheckoutSession> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_klarna_error)?;

        Ok(response.value)
    }

    /// Turn an authorization into an order.
    ///
    /// The homeowner's approval returns an authorization token to the caller's
    /// confirmation page; this call exchanges it for an order. A decline is
    /// reported as [`WirekitError::Api`], never retried.
    pub async fn create_order(
        &self,
        authorization_token: &str,
    ) -> Result<CreatedOrder, WirekitError> {
        ensure_identifier(authorization_token, "authorization")?;

        let url = self.url(&format!(
            "payments/v1/authorizations/{}/order",
            authorization_token.trim()
        ))?;
        let spec = self
            .request(Method::POST, url)
            .json_body(&serde_json::json!({}))?;
        let response: Response<CreatedOrder> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_klarna_error)?;

        Ok(response.value)
    }

    /// Read an order and its money movements.
    pub async fn get_order(&self, order_id: &str) -> Result<Order, WirekitError> {
        ensure_identifier(order_id, "order")?;

        let url = self.url(&format!("ordermanagement/v1/orders/{}", order_id.trim()))?;
        let spec = self.request(Method::GET, url);
        let response: Response<Order> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_klarna_error)?;

        Ok(response.value)
    }

    /// Capture a payment.
    ///
    /// A capture is money, so a lost response is reconciled with
    /// [`KlarnaClient::get_order`] and its `captured_amount` rather than sent
    /// again by this module.
    pub async fn capture_order(
        &self,
        order_id: &str,
        request: &CaptureRequest,
    ) -> Result<Capture, WirekitError> {
        ensure_identifier(order_id, "order")?;
        if request.captured_amount <= 0 {
            return Err(WirekitError::invalid_config(
                "a Klarna capture must be for a positive amount",
            ));
        }
        if let Some(lines) = request.order_lines.as_deref() {
            ensure_lines(lines)?;
        }

        let url = self.url(&format!(
            "ordermanagement/v1/orders/{}/captures",
            order_id.trim()
        ))?;
        let spec = self.request(Method::POST, url).json_body(request)?;
        let response: Response<Capture> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_klarna_error)?;

        Ok(response.value)
    }

    /// Refund a payment, in full or in part.
    pub async fn refund_order(
        &self,
        order_id: &str,
        request: &RefundRequest,
    ) -> Result<Refund, WirekitError> {
        ensure_identifier(order_id, "order")?;
        if request.refunded_amount <= 0 {
            return Err(WirekitError::invalid_config(
                "a Klarna refund must be for a positive amount",
            ));
        }
        if let Some(lines) = request.order_lines.as_deref() {
            ensure_lines(lines)?;
        }

        let url = self.url(&format!(
            "ordermanagement/v1/orders/{}/refunds",
            order_id.trim()
        ))?;
        let spec = self.request(Method::POST, url).json_body(request)?;
        let response: Response<Refund> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_klarna_error)?;

        Ok(response.value)
    }

    fn url(&self, path: &str) -> Result<url::Url, WirekitError> {
        self.inner
            .client
            .url(&format!("{}/{}", self.inner.config.region.base_url(), path))
    }

    fn request(&self, method: Method, url: url::Url) -> RequestSpec {
        RequestSpec::new(method, url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("klarna")
            .body_error_hook(reject_error_envelope)
    }
}

fn ensure_field(value: &str, what: &str) -> Result<(), WirekitError> {
    if value.trim().is_empty() {
        return Err(WirekitError::invalid_config(format!(
            "a Klarna {what} is required"
        )));
    }
    Ok(())
}

fn ensure_identifier(id: &str, what: &str) -> Result<(), WirekitError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(WirekitError::invalid_config(format!(
            "a Klarna {what} id is required"
        )));
    }
    if id.contains(['/', '?', '#']) {
        return Err(WirekitError::invalid_config(format!(
            "a Klarna {what} id must not contain a path or query separator"
        )));
    }
    Ok(())
}

fn ensure_lines(lines: &[OrderLine]) -> Result<(), WirekitError> {
    for line in lines {
        if line.name.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a Klarna order line needs a name",
            ));
        }
        if line.quantity == 0 {
            return Err(WirekitError::invalid_config(
                "a Klarna order line needs a quantity of at least one",
            ));
        }
    }
    Ok(())
}

/// Reject a 2xx response whose body reports a failure (CON-213).
fn reject_error_envelope(status: u16, body: &[u8]) -> Option<WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let code = value.get("error_code")?.as_str()?;
    let message = value
        .get("error_message")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            value
                .get("error_messages")
                .and_then(serde_json::Value::as_array)
                .and_then(|messages| messages.first())
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("Klarna reported a failure");

    Some(WirekitError::api_with_details(
        status,
        code,
        message,
        value.clone(),
    ))
}

/// Map a Klarna rejection onto the shared error type.
///
/// Klarna carries a stable `error_code` and a message list, and adds a
/// `correlation_id` for support; all three are kept in `details`. A 401 or 403
/// becomes an authentication failure.
fn map_klarna_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        details: Some(details),
        ..
    } = &error
    else {
        return error;
    };

    if *status == 401 || *status == 403 {
        return WirekitError::auth("Klarna rejected the merchant credentials");
    }

    let code = details
        .get("error_code")
        .and_then(serde_json::Value::as_str);
    let message = details
        .get("error_message")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            details
                .get("error_messages")
                .and_then(serde_json::Value::as_array)
                .and_then(|messages| messages.first())
                .and_then(serde_json::Value::as_str)
        });

    match (code, message) {
        (Some(code), Some(message)) => {
            WirekitError::api_with_details(*status, code, message, details.clone())
        }
        (Some(code), None) => WirekitError::api_with_details(
            *status,
            code,
            "Klarna reported a failure",
            details.clone(),
        ),
        (None, Some(message)) => {
            WirekitError::api_with_details(*status, "klarna_error", message, details.clone())
        }
        _ => error,
    }
}
