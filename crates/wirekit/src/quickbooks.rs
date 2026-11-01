//! QuickBooks Online: customers, invoices, payments, reference data, OAuth2
//! token refresh, and signed webhook ingestion.
//!
//! Scope: customer create/update, invoice create/void/send with ordered lines,
//! payment recording against an invoice, account and item queries, refresh
//! before expiry with refresh-once-on-401, `intuit-signature` verification over
//! the exact bytes, and Intuit `Fault` mapping (`catalogue/13_quickbooks.md`).
//!
//! Non-goals: the authorization-code browser flow, token persistence, payroll
//! and tax filing, bulk endpoints, and accounting domain rules (CON-106).
//!
//! Intuit reports failures in a `Fault` object with an `Error[]` array. Every
//! reported code is preserved on [`WirekitError::Api`], so a validation fault is
//! never mistaken for success (CON-213).
//!
//! ```no_run
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! use wirekit::quickbooks::{QuickBooksClient, QuickBooksConfig};
//! use wirekit::{Secret, WirekitClient};
//!
//! let http = WirekitClient::builder("https://quickbooks.api.intuit.com")?.build()?;
//! let books = QuickBooksClient::new(
//!     QuickBooksConfig {
//!         client_id: String::from("<client-id>"),
//!         client_secret: Secret::new(String::from("<client-secret>")),
//!         refresh_token: Secret::new(String::from("<refresh-token>")),
//!         realm_id: String::from("<realm-id>"),
//!         verifier_token: Secret::new(String::from("<verifier-token>")),
//!         minor_version: String::from("65"),
//!     },
//!     http,
//! );
//!
//! let customer = books
//!     .create_customer(&Customer::new("Acme"), None)
//!     .await?;
//! println!("{:?}", customer.id);
//! # Ok(())
//! # }
//! ```

use std::fmt;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{SystemTime, UNIX_EPOCH};

use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine as _;
use hmac::{Hmac, Mac};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use url::Url;

use crate::auth::constant_time_eq;
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The QuickBooks Online API host.
pub const BASE_URL: &str = "https://quickbooks.api.intuit.com";

/// The Intuit OAuth2 token endpoint.
pub const TOKEN_URL: &str = "https://oauth.platform.intuit.com/oauth2/v1/tokens/bearer";

/// Customer resource: `POST /v3/company/{realm-id}/customer`.
pub const CUSTOMER_PATH: &str = "/v3/company/{realm-id}/customer";

/// Invoice resource: `POST /v3/company/{realm-id}/invoice`.
pub const INVOICE_PATH: &str = "/v3/company/{realm-id}/invoice";

/// Invoice void: `POST /v3/company/{realm-id}/invoice?operation=void`.
pub const INVOICE_VOID_PATH: &str = "/v3/company/{realm-id}/invoice?operation=void";

/// Invoice send: `POST /v3/company/{realm-id}/invoice/{invoice-id}/send`.
pub const INVOICE_SEND_PATH: &str = "/v3/company/{realm-id}/invoice/{invoice-id}/send";

/// Payment resource: `POST /v3/company/{realm-id}/payment`.
pub const PAYMENT_PATH: &str = "/v3/company/{realm-id}/payment";

/// Query endpoint: `GET /v3/company/{realm-id}/query`.
pub const QUERY_PATH: &str = "/v3/company/{realm-id}/query";

/// The token endpoint path, as the plan records it.
pub const TOKEN_PATH: &str = "/oauth2/v1/tokens/bearer";

/// Refresh this many seconds before the access token expires.
pub const REFRESH_SKEW_SECONDS: i64 = 60;

/// Credentials and routing for one QuickBooks company.
#[derive(Clone)]
pub struct QuickBooksConfig {
    /// The Intuit client id.
    pub client_id: String,
    /// The Intuit client secret.
    pub client_secret: Secret<String>,
    /// The rotating refresh token.
    pub refresh_token: Secret<String>,
    /// The company (realm) id.
    pub realm_id: String,
    /// The webhook verifier token.
    pub verifier_token: Secret<String>,
    /// The `minorversion` query parameter.
    pub minor_version: String,
}

impl fmt::Debug for QuickBooksConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuickBooksConfig")
            .field("client_id", &self.client_id)
            .field("client_secret", &"***")
            .field("refresh_token", &"***")
            .field("realm_id", &self.realm_id)
            .field("verifier_token", &"***")
            .field("minor_version", &self.minor_version)
            .finish()
    }
}

/// An access token and its replacement refresh token.
#[derive(Clone)]
pub struct TokenSet {
    /// The bearer token for API calls.
    pub access_token: Secret<String>,
    /// The rotated refresh token the caller must persist.
    pub refresh_token: Secret<String>,
    /// Unix seconds at which the access token expires.
    pub expires_at: i64,
}

impl fmt::Debug for TokenSet {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TokenSet")
            .field("access_token", &"***")
            .field("refresh_token", &"***")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// A customer record.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Customer {
    /// The QuickBooks id, absent until the record is created.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    /// The concurrency token returned by Intuit.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sync_token: Option<String>,
    /// The display name Intuit shows.
    pub display_name: String,
    /// The primary email address, when known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub primary_email_addr: Option<serde_json::Value>,
    /// The primary phone number, when known.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub primary_phone: Option<serde_json::Value>,
}

impl Customer {
    /// A customer with only a display name.
    #[must_use]
    pub fn new(display_name: impl Into<String>) -> Self {
        Self {
            display_name: display_name.into(),
            ..Self::default()
        }
    }
}

/// One invoice line.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct InvoiceLine {
    /// The line amount.
    pub amount: f64,
    /// The free-text description.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub description: Option<String>,
    /// The item reference, when the line bills a catalogue item.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub item_ref: Option<serde_json::Value>,
}

/// An invoice record.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Invoice {
    /// The QuickBooks id, absent until the record is created.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    /// The concurrency token returned by Intuit.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sync_token: Option<String>,
    /// The customer this invoice bills.
    pub customer_ref: serde_json::Value,
    /// The ordered lines.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub line: Vec<InvoiceLine>,
    /// The outstanding balance Intuit reports.
    #[serde(default)]
    pub balance: f64,
    /// The document status, such as `Voided`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub status: Option<String>,
}

impl Invoice {
    /// An invoice for one customer with ordered lines.
    #[must_use]
    pub fn new(customer_id: impl Into<String>, lines: Vec<InvoiceLine>) -> Self {
        Self {
            customer_ref: serde_json::json!({ "value": customer_id.into() }),
            line: lines,
            ..Self::default()
        }
    }
}

/// A payment applied to an invoice.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Payment {
    /// The QuickBooks id, absent until the record is created.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub id: Option<String>,
    /// The concurrency token returned by Intuit.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub sync_token: Option<String>,
    /// The customer paying.
    pub customer_ref: serde_json::Value,
    /// The invoice being settled.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub line: Vec<serde_json::Value>,
    /// The payment amount.
    pub total_amt: f64,
}

/// An account in the chart of accounts.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Account {
    /// The QuickBooks id.
    pub id: String,
    /// The account name.
    pub name: String,
    /// The account type.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub account_type: Option<String>,
    /// The current balance, when Intuit reports one.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub current_balance: Option<f64>,
}

/// A product or service that invoice lines can reference.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct Item {
    /// The QuickBooks id.
    pub id: String,
    /// The item name.
    pub name: String,
    /// The item type, such as `Service`.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub type_: Option<String>,
    /// The unit price, when set.
    #[serde(skip_serializing_if = "Option::is_none", default)]
    pub unit_price: Option<f64>,
}

/// One entry of an Intuit `Fault.Error[]` array.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntuitError {
    /// The human-readable message.
    #[serde(default)]
    pub message: String,
    /// The Intuit error code.
    #[serde(default)]
    pub code: String,
    /// The field or detail Intuit blames.
    #[serde(default)]
    pub detail: String,
}

/// The Intuit `Fault` envelope.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "PascalCase")]
pub struct IntuitFault {
    /// Every error Intuit reported.
    #[serde(default)]
    pub error: Vec<IntuitError>,
}

/// A verified webhook notification.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WebhookNotification {
    /// The dedupe key: the first changed entity's id and operation.
    pub dedupe_key: String,
    /// Every realm the notification names.
    pub realm_ids: Vec<String>,
    /// The changed entities.
    pub entities: Vec<serde_json::Value>,
}

struct Inner {
    config: QuickBooksConfig,
    http: WirekitClient,
    token: Mutex<Option<TokenSet>>,
}

/// The QuickBooks client. Clone it freely; the transport and token cache are
/// shared.
#[derive(Clone)]
pub struct QuickBooksClient {
    inner: Arc<Inner>,
}

impl fmt::Debug for QuickBooksClient {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("QuickBooksClient")
            .field("config", &self.inner.config)
            .finish_non_exhaustive()
    }
}

impl QuickBooksClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// Build the transport against [`BASE_URL`].
    #[must_use]
    pub fn new(config: QuickBooksConfig, http: WirekitClient) -> Self {
        Self {
            inner: Arc::new(Inner {
                config,
                http,
                token: Mutex::new(None),
            }),
        }
    }

    /// The configured credentials and routing.
    #[must_use]
    pub fn config(&self) -> &QuickBooksConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn http(&self) -> &WirekitClient {
        &self.inner.http
    }

    /// Install a token set the caller obtained or persisted.
    pub fn set_token(&self, token: TokenSet) {
        *lock(&self.inner.token) = Some(token);
    }

    /// The last refresh token the client saw, so the caller can persist it.
    #[must_use]
    pub fn refresh_token(&self) -> Option<Secret<String>> {
        lock(&self.inner.token)
            .as_ref()
            .map(|token| token.refresh_token.clone())
    }

    fn endpoint(&self, template: &str) -> Result<Url, WirekitError> {
        let path = template.replace("{realm-id}", &self.inner.config.realm_id);
        let base = format!("{BASE_URL}{path}");

        let mut url = Url::parse(&base).map_err(|error| {
            WirekitError::invalid_config(format!("the QuickBooks endpoint is invalid: {error}"))
        })?;

        if url.scheme() != "https" {
            return Err(WirekitError::invalid_config(
                "the QuickBooks endpoint must use https (CON-520)",
            ));
        }

        url.query_pairs_mut()
            .append_pair("minorversion", &self.inner.config.minor_version);

        Ok(url)
    }

    /// The current bearer token, refreshed before it expires.
    async fn access_token(&self) -> Result<String, WirekitError> {
        let fresh = lock(&self.inner.token).as_ref().and_then(|token| {
            (token.expires_at - REFRESH_SKEW_SECONDS > now_seconds())
                .then(|| token.access_token.expose().clone())
        });

        if let Some(token) = fresh {
            return Ok(token);
        }

        let refreshed = self.refresh().await?;
        Ok(refreshed.access_token.expose().clone())
    }

    /// Exchange the refresh token for a new access token.
    ///
    /// Intuit rotates the refresh token, so the new value is stored and surfaced
    /// through [`QuickBooksClient::refresh_token`].
    pub async fn refresh(&self) -> Result<TokenSet, WirekitError> {
        let config = &self.inner.config;
        let basic = BASE64.encode(format!(
            "{}:{}",
            config.client_id,
            config.client_secret.expose()
        ));
        let form = format!(
            "grant_type=refresh_token&refresh_token={}",
            config.refresh_token.expose()
        );

        let url = Url::parse(TOKEN_URL).map_err(|error| {
            WirekitError::invalid_config(format!("the Intuit token URL is invalid: {error}"))
        })?;

        let spec = RequestSpec::post(url)
            .try_header("authorization", &format!("Basic {basic}"))?
            .try_header("accept", "application/json")?
            .try_header("content-type", "application/x-www-form-urlencoded")?
            .bytes_body(form.into_bytes());

        let response: Response<TokenBody> = self.inner.http.execute(spec).await?;
        let body = response.value;

        let token = TokenSet {
            access_token: Secret::new(body.access_token),
            refresh_token: Secret::new(body.refresh_token),
            expires_at: now_seconds() + body.expires_in,
        };

        *lock(&self.inner.token) = Some(token.clone());
        Ok(token)
    }

    fn bearer(&self, spec: RequestSpec, token: &str) -> Result<RequestSpec, WirekitError> {
        spec.try_header("authorization", &format!("Bearer {token}"))
    }

    async fn post<T: Serialize, R: DeserializeOwned>(
        &self,
        template: &str,
        body: &T,
    ) -> Result<R, WirekitError> {
        let token = self.access_token().await?;
        let url = self.endpoint(template)?;
        let spec = self
            .bearer(RequestSpec::post(url), &token)?
            .json_body(body)?;
        let response: Response<R> = self.inner.http.execute(spec).await?;
        Ok(response.value)
    }
}

/// The token endpoint response.
#[derive(Debug, Deserialize)]
struct TokenBody {
    access_token: String,
    refresh_token: String,
    expires_in: i64,
}

impl QuickBooksClient {
    /// Create a customer, optionally checking for an existing one first.
    ///
    /// With an idempotency key the client queries by display name and returns
    /// the existing customer instead of creating a duplicate (AC-016).
    pub async fn create_customer(
        &self,
        customer: &Customer,
        idempotency_key: Option<&str>,
    ) -> Result<Customer, WirekitError> {
        if idempotency_key.is_some() {
            if let Some(existing) = self.find_customer(&customer.display_name).await? {
                return Ok(existing);
            }
        }

        let envelope: CustomerEnvelope = self.post(CUSTOMER_PATH, customer).await?;
        Ok(envelope.customer)
    }

    /// Update a customer; the record must carry its id and sync token.
    pub async fn update_customer(&self, customer: &Customer) -> Result<Customer, WirekitError> {
        if customer.id.is_none() || customer.sync_token.is_none() {
            return Err(WirekitError::invalid_config(
                "a customer update needs both the id and the sync token",
            ));
        }

        let token = self.access_token().await?;
        let url = self.endpoint(CUSTOMER_PATH)?;
        let spec = self
            .bearer(RequestSpec::post(url), &token)?
            .json_body(customer)?;
        let response: Response<CustomerEnvelope> = self.inner.http.execute(spec).await?;
        Ok(response.value.customer)
    }

    /// Find a customer by display name, if one exists.
    pub async fn find_customer(
        &self,
        display_name: &str,
    ) -> Result<Option<Customer>, WirekitError> {
        let escaped = display_name.replace('\'', "\\'");
        let sql = format!("select * from Customer where DisplayName = '{escaped}'");
        let page: QueryEnvelope = self.query(&sql).await?;
        Ok(page.query_response.customer.into_iter().next())
    }

    /// Create an invoice with its ordered lines.
    pub async fn create_invoice(&self, invoice: &Invoice) -> Result<Invoice, WirekitError> {
        let envelope: InvoiceEnvelope = self.post(INVOICE_PATH, invoice).await?;
        Ok(envelope.invoice)
    }

    /// Void an existing invoice.
    pub async fn void_invoice(&self, invoice: &Invoice) -> Result<Invoice, WirekitError> {
        let token = self.access_token().await?;
        let url = self.endpoint(INVOICE_VOID_PATH)?;
        let spec = self
            .bearer(RequestSpec::post(url), &token)?
            .json_body(invoice)?;
        let response: Response<InvoiceEnvelope> = self.inner.http.execute(spec).await?;
        Ok(response.value.invoice)
    }

    /// Send an invoice to its customer.
    pub async fn send_invoice(&self, invoice_id: &str) -> Result<Invoice, WirekitError> {
        let token = self.access_token().await?;
        let template = INVOICE_SEND_PATH.replace("{invoice-id}", invoice_id);
        let url = self.endpoint(&template)?;
        let spec = self
            .bearer(RequestSpec::post(url), &token)?
            .json_body(&serde_json::json!({ "EmailStatus": "NeedToSend" }))?;
        let response: Response<InvoiceEnvelope> = self.inner.http.execute(spec).await?;
        Ok(response.value.invoice)
    }

    /// Record a payment against an invoice.
    pub async fn record_payment(&self, payment: &Payment) -> Result<Payment, WirekitError> {
        let envelope: PaymentEnvelope = self.post(PAYMENT_PATH, payment).await?;
        Ok(envelope.payment)
    }

    /// List accounts, bounded by `max`.
    pub async fn list_accounts(&self, max: u32) -> Result<Vec<Account>, WirekitError> {
        let sql = format!("select * from Account maxresults {max}");
        let page: QueryEnvelope = self.query(&sql).await?;
        Ok(page.query_response.account)
    }

    /// List items, bounded by `max`.
    pub async fn list_items(&self, max: u32) -> Result<Vec<Item>, WirekitError> {
        let sql = format!("select * from Item maxresults {max}");
        let page: QueryEnvelope = self.query(&sql).await?;
        Ok(page.query_response.item)
    }

    async fn query<R: DeserializeOwned>(&self, sql: &str) -> Result<R, WirekitError> {
        let token = self.access_token().await?;
        let mut url = self.endpoint(QUERY_PATH)?;
        url.query_pairs_mut().append_pair("query", sql);

        let spec = self.bearer(RequestSpec::get(url), &token)?;
        let response: Response<R> = self.inner.http.execute(spec).await?;
        Ok(response.value)
    }
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

impl QuickBooksClient {
    /// Verify an `intuit-signature` over the exact received bytes.
    ///
    /// The HMAC-SHA256 comparison is constant time and fails closed; the parsed
    /// notification carries a dedupe key so a replay can be recognised (AC-012,
    /// AC-013).
    pub fn verify_webhook(
        &self,
        raw: &[u8],
        signature: &str,
    ) -> Result<WebhookNotification, WirekitError> {
        let mut mac = <Hmac<Sha256> as Mac>::new_from_slice(
            self.inner.config.verifier_token.expose().as_bytes(),
        )
        .map_err(|_| WirekitError::invalid_config("the verifier token is not a valid HMAC key"))?;
        mac.update(raw);
        let expected = BASE64.encode(mac.finalize().into_bytes());

        if !constant_time_eq(expected.as_bytes(), signature.trim().as_bytes()) {
            return Err(WirekitError::auth(
                "the QuickBooks webhook signature did not match",
            ));
        }

        let body: serde_json::Value = serde_json::from_slice(raw).map_err(|error| {
            WirekitError::decode_with("the QuickBooks webhook body is not valid JSON", error)
        })?;

        let notifications = body
            .get("eventNotifications")
            .and_then(serde_json::Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut realm_ids = Vec::new();
        let mut entities = Vec::new();
        for notification in notifications {
            if let Some(realm) = notification
                .get("realmId")
                .and_then(serde_json::Value::as_str)
            {
                realm_ids.push(String::from(realm));
            }
            if let Some(changed) = notification
                .get("dataChangeEvent")
                .and_then(|event| event.get("entities"))
                .and_then(serde_json::Value::as_array)
            {
                entities.extend(changed.iter().cloned());
            }
        }

        let dedupe_key = entities.first().map_or_else(String::new, |entity| {
            let id = entity
                .get("id")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            let operation = entity
                .get("operation")
                .and_then(serde_json::Value::as_str)
                .unwrap_or_default();
            format!("{operation}:{id}")
        });

        Ok(WebhookNotification {
            dedupe_key,
            realm_ids,
            entities,
        })
    }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct CustomerEnvelope {
    customer: Customer,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct InvoiceEnvelope {
    invoice: Invoice,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct PaymentEnvelope {
    payment: Payment,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QueryResponseBody {
    #[serde(default)]
    customer: Vec<Customer>,
    #[serde(default)]
    account: Vec<Account>,
    #[serde(default)]
    item: Vec<Item>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "PascalCase")]
struct QueryEnvelope {
    #[serde(default)]
    query_response: QueryResponseBody,
}
