//! EagleView: satellite roof measurement (`catalogue/12_eagleview.md`).
//!
//! Scope: client-credentials OAuth2, an idempotent **paid** measurement order, an
//! asynchronous order lifecycle, report fetch, a bounded PDF download, and
//! address-scoped report listing.
//!
//! Non-goals: estimate calculation, pricing rules, report caching, bulk ordering,
//! and status webhooks — the catalogue says not to assume a callback exists, so
//! v1 polls (`RISK-002`).
//!
//! Measurement costs money per report. [`EagleViewClient::order_measurement`]
//! therefore refuses to call the provider unless the caller passes
//! `cost_acknowledged: true`, so an unpriced order cannot happen by accident
//! (AC-005).
//!
//! Errors are `WirekitError::Api` values whose `api_code()` carries the provider
//! code, which is how a rejected order, an unmeasurable address, and a processing
//! failure stay distinguishable (AC-011).
//!
//! ```no_run
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! use wirekit::eagleview::{EagleViewClient, EagleViewConfig, MeasurementRequest};
//! use wirekit::{Secret, WirekitClient};
//!
//! let http = WirekitClient::builder("https://api.eagleview.com")?.build()?;
//! let client = EagleViewClient::new(
//!     EagleViewConfig {
//!         client_id: String::from("<client-id>"),
//!         client_secret: Secret::new(String::from("<client-secret>")),
//!     },
//!     http,
//! );
//!
//! let request = MeasurementRequest::new("1 Main St", "Springfield", "IL", "62704");
//! let order = client
//!     .order_measurement(&request, "estimate-42", true)
//!     .await?;
//! println!("{:?}", order.id);
//! # Ok(())
//! # }
//! ```

use std::collections::HashMap;
use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use serde::{Deserialize, Serialize};
use url::Url;

use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The documented EagleView API base.
pub const BASE_URL: &str = "https://api.eagleview.com";

/// Client-credentials token endpoint.
pub const TOKEN_PATH: &str = "/oauth2/token";

/// Order collection: create a measurement order.
pub const ORDERS_PATH: &str = "/v1/orders";

/// One order: read its status.
pub const ORDER_STATUS_PATH: &str = "/v1/orders/{order-id}";

/// One order's report.
pub const REPORT_PATH: &str = "/v1/orders/{order-id}/report";

/// One order's PDF document.
pub const DOCUMENT_PATH: &str = "/v1/orders/{order-id}/report/document";

/// Reports already produced for an address.
pub const REPORTS_PATH: &str = "/v1/reports";

/// Refresh the token this many seconds before it expires.
pub const REFRESH_SKEW_SECONDS: i64 = 60;

/// Upper bound on a downloaded PDF, so a hostile or broken response cannot
/// exhaust memory (NFR-002).
pub const MAX_PDF_BYTES: usize = 8 * 1024 * 1024;

/// Credentials and routing for EagleView.
#[derive(Clone)]
pub struct EagleViewConfig {
    /// The EagleView client id.
    pub client_id: String,
    /// The EagleView client secret.
    pub client_secret: Secret<String>,
}

impl fmt::Debug for EagleViewConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EagleViewConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"***")
            .finish()
    }
}

/// A cached client-credentials token.
#[derive(Clone)]
pub struct AccessToken {
    /// The bearer token.
    pub access_token: Secret<String>,
    /// Unix seconds at which the token expires.
    pub expires_at: i64,
}

impl fmt::Debug for AccessToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AccessToken")
            .field("access_token", &"***")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

/// The lifecycle state of a measurement order.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum MeasurementStatus {
    /// The order was accepted and queued.
    #[default]
    Accepted,
    /// Measurement is in progress.
    Processing,
    /// The report is available.
    Complete,
    /// The order failed; the reason is carried on the order.
    Failed,
}

impl MeasurementStatus {
    /// Whether the report can be fetched.
    #[must_use]
    pub fn is_complete(&self) -> bool {
        matches!(self, Self::Complete)
    }

    /// Whether waiting longer cannot help.
    #[must_use]
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Complete | Self::Failed)
    }
}

/// The kind of roof segment the report measured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum SegmentKind {
    /// A ridge line.
    Ridge,
    /// A valley.
    Valley,
    /// A hip.
    Hip,
    /// An eave.
    Eave,
    /// Any other segment the provider reports.
    #[default]
    Other,
}

/// A measurement order request.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementRequest {
    /// Street address.
    pub address_line: String,
    /// City.
    pub city: String,
    /// State or region code.
    pub state: String,
    /// Postal code.
    pub postal_code: String,
    /// The provider product, empty for the provider default.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub product: String,
    /// Extra provider options.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub options: Vec<String>,
}

impl MeasurementRequest {
    /// A request for one street address on the provider's default product.
    #[must_use]
    pub fn new(
        address_line: impl Into<String>,
        city: impl Into<String>,
        state: impl Into<String>,
        postal_code: impl Into<String>,
    ) -> Self {
        Self {
            address_line: address_line.into(),
            city: city.into(),
            state: state.into(),
            postal_code: postal_code.into(),
            product: String::new(),
            options: Vec::new(),
        }
    }

    /// The address as one comparable string, used to detect an idempotency-key
    /// collision with a different property (EC-004).
    #[must_use]
    pub fn address_fingerprint(&self) -> String {
        format!(
            "{}|{}|{}|{}|{}",
            self.address_line.trim().to_lowercase(),
            self.city.trim().to_lowercase(),
            self.state.trim().to_lowercase(),
            self.postal_code.trim().to_lowercase(),
            self.product.trim().to_lowercase()
        )
    }
}

/// An order in the measurement lifecycle.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementOrder {
    /// The provider order id.
    pub id: String,
    /// Its current state.
    pub status: MeasurementStatus,
    /// Whether the caller acknowledged the per-order cost before it was created.
    #[serde(default)]
    pub cost_acknowledged: bool,
    /// The provider's failure reason, present when the order failed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub failure_reason: Option<String>,
}

impl MeasurementOrder {
    /// The failure reason, if the order failed.
    #[must_use]
    pub fn failure_reason(&self) -> Option<&str> {
        self.failure_reason.as_deref()
    }
}

/// Total measured area.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoofArea {
    /// Roofing squares (100 sq ft each).
    pub squares: f64,
    /// Facet count.
    pub facets: u32,
    /// The provider's waste factor.
    pub waste_factor: f64,
}

/// A roof pitch.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoofPitch {
    /// The ratio the provider reports, such as `6/12`.
    pub ratio: String,
    /// The same pitch as a rise-over-run number.
    pub rise_over_run: f64,
}

/// One measured roof segment.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RoofSegment {
    /// Ridge, valley, hip, or eave.
    pub kind: SegmentKind,
    /// Segment area in roofing squares.
    pub area_squares: f64,
    /// Segment pitch.
    pub pitch: RoofPitch,
}

/// A completed measurement report.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MeasurementReport {
    /// The order it belongs to.
    pub order_id: String,
    /// Total measured area.
    pub total_area: RoofArea,
    /// Predominant pitch.
    pub pitch: RoofPitch,
    /// The segment breakdown, in provider order.
    #[serde(default)]
    pub segments: Vec<RoofSegment>,
    /// Where the PDF lives, when the provider supplies one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub document_url: Option<String>,
}

/// A downloaded report document.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PdfDocument {
    /// The validated content type.
    pub content_type: String,
    /// The bounded body.
    pub bytes: Vec<u8>,
}

struct Inner {
    config: EagleViewConfig,
    http: WirekitClient,
    token: Mutex<Option<AccessToken>>,
    orders: Mutex<HashMap<String, (String, MeasurementOrder)>>,
}

/// The EagleView client. Clone it freely; the transport, token cache, and
/// idempotency ledger are shared.
#[derive(Clone)]
pub struct EagleViewClient {
    inner: Arc<Inner>,
}

impl fmt::Debug for EagleViewClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("EagleViewClient")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Whether `url` is the crate's loopback test affinity.
fn is_loopback(url: &Url) -> bool {
    matches!(url.host_str(), Some("127.0.0.1" | "localhost" | "::1"))
}

impl EagleViewClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// Build the transport against [`BASE_URL`].
    #[must_use]
    pub fn new(config: EagleViewConfig, http: WirekitClient) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                http,
                token: Mutex::new(None),
                orders: Mutex::new(HashMap::new()),
            }),
        }
    }

    /// The configured credentials.
    #[must_use]
    pub fn config(&self) -> &EagleViewConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn http(&self) -> &WirekitClient {
        &self.inner.http
    }

    /// Install a token the caller obtained or persisted.
    pub fn set_token(&self, token: AccessToken) {
        *lock(&self.inner.token) = Some(token);
    }

    /// Build a resource URL for `template`, substituting the order id.
    fn endpoint(&self, template: &str, order_id: Option<&str>) -> Result<Url, WirekitError> {
        let path = match order_id {
            Some(id) => template.replace("{order-id}", id),
            None => String::from(template),
        };

        let configured = self.inner.http.base_url();
        let loopback = is_loopback(configured);
        let base = if loopback {
            format!("{}{path}", configured.as_str().trim_end_matches('/'))
        } else {
            format!("{BASE_URL}{path}")
        };

        let url = Url::parse(&base).map_err(|error| {
            WirekitError::invalid_config(format!("the EagleView endpoint is invalid: {error}"))
        })?;

        if !loopback && url.scheme() != "https" {
            return Err(WirekitError::invalid_config(
                "the EagleView endpoint must use https (CON-520)",
            ));
        }

        Ok(url)
    }

    /// The token endpoint, honouring a loopback transport for tests.
    fn token_url(&self) -> Result<Url, WirekitError> {
        let configured = self.inner.http.base_url();
        let base = if is_loopback(configured) {
            format!("{}{TOKEN_PATH}", configured.as_str().trim_end_matches('/'))
        } else {
            format!("{BASE_URL}{TOKEN_PATH}")
        };
        Url::parse(&base).map_err(|error| {
            WirekitError::invalid_config(format!("the EagleView token URL is invalid: {error}"))
        })
    }
}

/// The token endpoint response.
#[derive(Debug, Deserialize)]
struct TokenBody {
    access_token: String,
    expires_in: i64,
}

impl EagleViewClient {
    /// Exchange the client credentials for an access token (AC-001, AC-002).
    pub async fn refresh(&self) -> Result<AccessToken, WirekitError> {
        let config = &self.inner.config;
        let basic = BASE64.encode(format!(
            "{}:{}",
            config.client_id,
            config.client_secret.expose()
        ));

        let url = self.token_url()?;
        let spec = RequestSpec::post(url)
            .try_header("authorization", &format!("Basic {basic}"))?
            .try_header("accept", "application/json")?
            .try_header("content-type", "application/x-www-form-urlencoded")?
            .bytes_body(b"grant_type=client_credentials".to_vec());

        let response: Response<TokenBody> = self.inner.http.execute(spec).await?;

        let token = AccessToken {
            access_token: Secret::new(response.value.access_token),
            expires_at: now_seconds() + response.value.expires_in,
        };
        *lock(&self.inner.token) = Some(token.clone());
        Ok(token)
    }

    /// The current bearer token, obtained once and reused until it expires.
    async fn access_token(&self) -> Result<String, WirekitError> {
        let fresh = lock(&self.inner.token).as_ref().and_then(|token| {
            (token.expires_at - REFRESH_SKEW_SECONDS > now_seconds())
                .then(|| token.access_token.expose().clone())
        });

        if let Some(token) = fresh {
            return Ok(token);
        }

        Ok(self.refresh().await?.access_token.expose().clone())
    }

    fn bearer(&self, spec: RequestSpec, token: &str) -> Result<RequestSpec, WirekitError> {
        spec.try_header("authorization", &format!("Bearer {token}"))
    }

    /// Order a measurement report.
    ///
    /// The order is a paid provider call, so it is refused locally unless
    /// `cost_acknowledged` is true (AC-005). A repeated `idempotency_key` returns
    /// the order already created for the same address and never places a second
    /// one (AC-004); reusing the key for a different address is an error (EC-004).
    pub async fn order_measurement(
        &self,
        request: &MeasurementRequest,
        idempotency_key: &str,
        cost_acknowledged: bool,
    ) -> Result<MeasurementOrder, WirekitError> {
        if !cost_acknowledged {
            return Err(WirekitError::invalid_config(
                "an EagleView order needs an explicit cost acknowledgement (AC-005)",
            ));
        }
        if idempotency_key.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "an EagleView order needs a caller idempotency key (AC-004)",
            ));
        }

        let fingerprint = request.address_fingerprint();
        if let Some((seen, order)) = lock(&self.inner.orders).get(idempotency_key).cloned() {
            if seen != fingerprint {
                return Err(WirekitError::invalid_config(
                    "the idempotency key was already used for a different address (EC-004)",
                ));
            }
            return Ok(order);
        }

        let token = self.access_token().await?;
        let url = self.endpoint(ORDERS_PATH, None)?;
        let spec = self
            .bearer(RequestSpec::post(url), &token)?
            .try_header("idempotency-key", idempotency_key)?
            .json_body(request)?;

        let response: Response<MeasurementOrder> = self.inner.http.execute(spec).await?;
        let mut order = response.value;
        order.cost_acknowledged = true;

        lock(&self.inner.orders)
            .insert(String::from(idempotency_key), (fingerprint, order.clone()));

        Ok(order)
    }

    /// Read an order's current state (AC-006).
    pub async fn order_status(&self, order_id: &str) -> Result<MeasurementOrder, WirekitError> {
        let token = self.access_token().await?;
        let url = self.endpoint(ORDER_STATUS_PATH, Some(order_id))?;
        let spec = self.bearer(RequestSpec::get(url), &token)?;
        let response: Response<MeasurementOrder> = self.inner.http.execute(spec).await?;
        Ok(response.value)
    }

    /// Fetch the completed report (AC-008).
    pub async fn fetch_report(&self, order_id: &str) -> Result<MeasurementReport, WirekitError> {
        let token = self.access_token().await?;
        let url = self.endpoint(REPORT_PATH, Some(order_id))?;
        let spec = self.bearer(RequestSpec::get(url), &token)?;
        let response: Response<MeasurementReport> = self.inner.http.execute(spec).await?;
        Ok(response.value)
    }

    /// Download the report document (AC-009).
    ///
    /// The content type is validated and the body is bounded before anything is
    /// returned, so a broken or hostile response cannot be mistaken for a PDF.
    pub async fn download_pdf(&self, order_id: &str) -> Result<PdfDocument, WirekitError> {
        let token = self.access_token().await?;
        let url = self.endpoint(DOCUMENT_PATH, Some(order_id))?;
        let spec = self
            .bearer(RequestSpec::get(url), &token)?
            .try_header("accept", "application/pdf")?;

        let raw = self.inner.http.send(spec).await?;

        if !(200..300).contains(&raw.status) {
            return Err(WirekitError::api_from_body(raw.status, &raw.body));
        }

        let content_type = raw
            .headers
            .get("content-type")
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();

        if !content_type.starts_with("application/pdf") {
            return Err(WirekitError::decode(format!(
                "the report document is not a PDF (content-type: {content_type})"
            )));
        }

        if raw.body.len() > MAX_PDF_BYTES {
            return Err(WirekitError::decode(format!(
                "the report document is larger than the {MAX_PDF_BYTES} byte limit"
            )));
        }

        Ok(PdfDocument {
            content_type,
            bytes: raw.body,
        })
    }

    /// List the reports already produced for an address (AC-010).
    pub async fn list_reports(
        &self,
        address: &str,
        limit: u32,
    ) -> Result<Vec<MeasurementReport>, WirekitError> {
        if limit == 0 {
            return Err(WirekitError::invalid_config(
                "a report listing needs a non-zero limit (AC-010)",
            ));
        }

        let token = self.access_token().await?;
        let mut url = self.endpoint(REPORTS_PATH, None)?;
        url.query_pairs_mut()
            .append_pair("address", address)
            .append_pair("limit", &limit.to_string());

        let spec = self.bearer(RequestSpec::get(url), &token)?;
        let response: Response<ReportsEnvelope> = self.inner.http.execute(spec).await?;
        Ok(response.value.reports)
    }
}

/// The report listing response.
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportsEnvelope {
    #[serde(default)]
    reports: Vec<MeasurementReport>,
}
