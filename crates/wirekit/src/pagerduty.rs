//! PagerDuty — incidents through the Events API v2 and management through the
//! REST API.
//!
//! # Scope
//!
//! Built from `catalogue/22_pagerduty.md`: trigger, acknowledge, and resolve an
//! incident through the Events API, list incidents and read on-calls through the
//! REST API, and verify an inbound webhook signature. Auth, the token bucket,
//! the retry policy with `Retry-After`, tracing, and the response size limit are
//! the shared ones.
//!
//! # Non-goals
//!
//! - **No escalation-policy management.** Reading the policy a call belongs to is
//!   part of an on-call record; creating or editing policies and schedules is
//!   the provider's console.
//! - **No user or team administration.** Seats, roles, and notification rules
//!   belong to the account owner.
//! - **No webhook receiver.** [`PagerDutyClient::verify_webhook`] checks the
//!   signature over bytes the caller already received; this crate opens no
//!   listening socket and stores no dedupe ledger.
//! - **No PII handling.** The catalogue makes responders' view of an incident
//!   payload the caller's responsibility; this module sends what it is given.
//!
//! # Two hosts
//!
//! The Events API lives at [`BASE_URL`] and the REST API at [`REST_BASE_URL`],
//! so one [`WirekitClient`] built against either host serves both: every request
//! uses an absolute HTTPS URL and the shared pipeline reads only the configured
//! retry, rate-limit, and tracing settings.
//!
//! # Credentials
//!
//! Triggering needs only the routing key, because it travels in the event body.
//! Listing incidents and reading on-calls need the REST API key, sent in the
//! `Authorization` header as `Token token=<key>`. Both are held in
//! [`Secret`]s and render as `***` in `Debug`.
//!
//! ```no_run
//! use wirekit::pagerduty::{IncidentEvent, PagerDutyClient, PagerDutyConfig, Severity};
//! use wirekit::{Secret, WirekitClient};
//!
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! let transport = WirekitClient::new("https://events.pagerduty.com/v2")?;
//! let client = PagerDutyClient::new(
//!     transport,
//!     PagerDutyConfig::new(
//!         Secret::new(String::from("<api-key>")),
//!         Secret::new(String::from("<routing-key>")),
//!     ),
//! )?;
//!
//! let event = IncidentEvent::new("Roof crew hazard reported", "storm-blast")
//!     .with_severity(Severity::Critical)
//!     .with_dedup_key("escalation-42");
//! client.trigger(&event).await?;
//! # Ok(())
//! # }
//! ```

use std::fmt::Write as _;
use std::sync::Arc;

use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;

use crate::auth::{constant_time_eq, ApiKeyHeader, AuthScheme};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The PagerDuty Events API v2 base URL.
pub const BASE_URL: &str = "https://events.pagerduty.com/v2";

/// The PagerDuty REST API base URL.
pub const REST_BASE_URL: &str = "https://api.pagerduty.com";

/// The media type the REST API expects on every request.
pub const ACCEPT_HEADER: &str = "application/vnd.pagerduty+json;version=2";

/// How urgent an incident is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    /// A person must act now.
    Critical,
    /// A failure that needs attention.
    Error,
    /// A condition that may become a failure.
    Warning,
    /// Informational only.
    Info,
}

impl Severity {
    /// The provider's wire value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Error => "error",
            Self::Warning => "warning",
            Self::Info => "info",
        }
    }
}

/// How urgent a REST incident is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Urgency {
    /// Paged immediately.
    High,
    /// Queued for the next response.
    Low,
}

impl Urgency {
    /// The provider's wire value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::High => "high",
            Self::Low => "low",
        }
    }
}

/// The lifecycle state the REST API reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IncidentStatus {
    /// Paging the on-call responder.
    Triggered,
    /// A responder has taken ownership.
    Acknowledged,
    /// The incident is closed.
    Resolved,
}

impl IncidentStatus {
    /// The provider's wire value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Triggered => "triggered",
            Self::Acknowledged => "acknowledged",
            Self::Resolved => "resolved",
        }
    }
}

/// How the client is configured.
#[derive(Debug, Clone)]
pub struct PagerDutyConfig {
    /// The REST API key, sent as `Authorization: Token token=<key>`.
    pub api_key: Option<Secret<String>>,
    /// The Events API routing key, sent in the event body.
    pub routing_key: Option<Secret<String>>,
    /// The account email PagerDuty records as the caller, sent as `From`.
    pub from_email: Option<String>,
    /// The secret an inbound webhook is signed with.
    pub webhook_secret: Option<Secret<String>>,
}

impl PagerDutyConfig {
    /// Build a configuration with both credentials.
    #[must_use]
    pub fn new(api_key: Secret<String>, routing_key: Secret<String>) -> Self {
        Self {
            api_key: Some(api_key),
            routing_key: Some(routing_key),
            from_email: None,
            webhook_secret: None,
        }
    }

    /// Build a configuration for the Events API only.
    #[must_use]
    pub fn events_only(routing_key: Secret<String>) -> Self {
        Self {
            api_key: None,
            routing_key: Some(routing_key),
            from_email: None,
            webhook_secret: None,
        }
    }

    /// Build a configuration for the REST API only.
    #[must_use]
    pub fn rest_only(api_key: Secret<String>) -> Self {
        Self {
            api_key: Some(api_key),
            routing_key: None,
            from_email: None,
            webhook_secret: None,
        }
    }

    /// Record the account email the REST API attributes a call to.
    #[must_use]
    pub fn with_from_email(mut self, email: impl Into<String>) -> Self {
        self.from_email = Some(email.into());
        self
    }

    /// Accept inbound webhooks signed with `secret`.
    #[must_use]
    pub fn with_webhook_secret(mut self, secret: Secret<String>) -> Self {
        self.webhook_secret = Some(secret);
        self
    }
}

/// The payload that triggers an incident.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IncidentEvent {
    /// A short, human-readable summary. Required.
    pub summary: String,
    /// The service or system the event came from. Required.
    pub source: String,
    /// How urgent the event is.
    pub severity: Severity,
    /// Reuse this key to acknowledge, resolve, or deduplicate the same incident.
    pub dedup_key: Option<String>,
    /// An ISO 8601 timestamp; the provider stamps the event when absent.
    pub timestamp: Option<String>,
    /// The part of the system that failed.
    pub component: Option<String>,
    /// The logical grouping, for example a cluster or team.
    pub group: Option<String>,
    /// The class of the event, for example `deploy` or `latency`.
    pub class: Option<String>,
    /// Typed details the responder sees. Must not contain personal data.
    pub custom_details: Option<serde_json::Value>,
}

impl IncidentEvent {
    /// Build a trigger payload with the two required fields.
    #[must_use]
    pub fn new(summary: impl Into<String>, source: impl Into<String>) -> Self {
        Self {
            summary: summary.into(),
            source: source.into(),
            severity: Severity::Error,
            dedup_key: None,
            timestamp: None,
            component: None,
            group: None,
            class: None,
            custom_details: None,
        }
    }

    /// Set the severity.
    #[must_use]
    pub fn with_severity(mut self, severity: Severity) -> Self {
        self.severity = severity;
        self
    }

    /// Set the deduplication key.
    #[must_use]
    pub fn with_dedup_key(mut self, key: impl Into<String>) -> Self {
        self.dedup_key = Some(key.into());
        self
    }

    /// Set the timestamp.
    #[must_use]
    pub fn with_timestamp(mut self, timestamp: impl Into<String>) -> Self {
        self.timestamp = Some(timestamp.into());
        self
    }

    /// Set the component.
    #[must_use]
    pub fn with_component(mut self, component: impl Into<String>) -> Self {
        self.component = Some(component.into());
        self
    }

    /// Set the group.
    #[must_use]
    pub fn with_group(mut self, group: impl Into<String>) -> Self {
        self.group = Some(group.into());
        self
    }

    /// Set the class.
    #[must_use]
    pub fn with_class(mut self, class: impl Into<String>) -> Self {
        self.class = Some(class.into());
        self
    }

    /// Attach typed details for the responder.
    #[must_use]
    pub fn with_custom_details(mut self, details: serde_json::Value) -> Self {
        self.custom_details = Some(details);
        self
    }
}

/// The Events API's acceptance of a trigger, acknowledgement, or resolution.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct EventAck {
    /// `success` when the provider accepted the event.
    #[serde(default)]
    pub status: String,
    /// The provider's human-readable note.
    #[serde(default)]
    pub message: Option<String>,
    /// The key the provider deduplicated the event on.
    #[serde(default)]
    pub dedup_key: Option<String>,
}

/// An escalation policy an incident belongs to.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct EscalationPolicy {
    /// The provider's identifier.
    #[serde(default)]
    pub id: String,
    /// The policy's name.
    #[serde(default)]
    pub summary: Option<String>,
    /// The console URL for the policy.
    #[serde(default)]
    pub html_url: Option<String>,
}

/// An alert attached to an incident.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Alert {
    /// The provider's identifier.
    #[serde(default)]
    pub id: String,
    /// The alert's state, for example `triggered`.
    #[serde(default)]
    pub status: Option<String>,
    /// The alert's summary line.
    #[serde(default)]
    pub summary: Option<String>,
    /// When the alert was created.
    #[serde(default)]
    pub created_at: Option<String>,
    /// The console URL for the alert.
    #[serde(default)]
    pub html_url: Option<String>,
}

/// A REST incident.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Incident {
    /// The provider's identifier.
    #[serde(default)]
    pub id: String,
    /// The account-scoped incident number.
    #[serde(default)]
    pub incident_number: Option<u32>,
    /// The incident title.
    #[serde(default)]
    pub title: Option<String>,
    /// The lifecycle state the provider reports.
    #[serde(default)]
    pub status: Option<String>,
    /// `high` or `low`.
    #[serde(default)]
    pub urgency: Option<String>,
    /// When the incident was created.
    #[serde(default)]
    pub created_at: Option<String>,
    /// The console URL for the incident.
    #[serde(default)]
    pub html_url: Option<String>,
    /// The policy that decides who is paged.
    #[serde(default)]
    pub escalation_policy: Option<EscalationPolicy>,
    /// The alerts the provider returned with the incident.
    #[serde(default)]
    pub alerts: Vec<Alert>,
}

/// One page of incidents.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncidentPage {
    /// The incidents on this page.
    pub incidents: Vec<Incident>,
    /// Whether another page exists. A page past the last one is empty and
    /// reports `false`, so a caller's loop terminates.
    pub more: bool,
    /// The page size the provider applied.
    pub limit: u32,
    /// The offset of this page.
    pub offset: u32,
}

/// The filters accepted by [`PagerDutyClient::list_incidents`].
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct IncidentQuery {
    /// Restrict to these lifecycle states. Empty means every state.
    pub statuses: Vec<IncidentStatus>,
    /// Restrict to these urgencies. Empty means both.
    pub urgencies: Vec<Urgency>,
    /// The page size, at most 100. `None` uses the provider default.
    pub limit: Option<u32>,
    /// The number of records to skip.
    pub offset: Option<u32>,
}

impl IncidentQuery {
    /// An unfiltered query.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Restrict to a lifecycle state.
    #[must_use]
    pub fn with_status(mut self, status: IncidentStatus) -> Self {
        self.statuses.push(status);
        self
    }

    /// Restrict to an urgency.
    #[must_use]
    pub fn with_urgency(mut self, urgency: Urgency) -> Self {
        self.urgencies.push(urgency);
        self
    }

    /// Set the page size.
    #[must_use]
    pub fn with_limit(mut self, limit: u32) -> Self {
        self.limit = Some(limit);
        self
    }

    /// Set the page offset.
    #[must_use]
    pub fn with_offset(mut self, offset: u32) -> Self {
        self.offset = Some(offset);
        self
    }
}

/// A person who is on call, and the schedule that put them there.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OnCall {
    /// The provider's identifier for the on-call entry.
    pub id: String,
    /// The escalation policy the entry belongs to.
    pub escalation_policy: Option<EscalationPolicy>,
    /// The provider's identifier for the schedule.
    pub schedule_id: Option<String>,
    /// The provider's identifier for the person on call.
    pub user_id: Option<String>,
    /// When the shift starts.
    pub start: Option<String>,
    /// When the shift ends.
    pub end: Option<String>,
}

struct PagerDutyInner {
    client: WirekitClient,
    config: PagerDutyConfig,
    rest_auth: Option<Arc<dyn AuthScheme>>,
}

/// The PagerDuty client. Clone it freely; the transport is shared.
#[derive(Clone)]
pub struct PagerDutyClient {
    inner: Arc<PagerDutyInner>,
}

impl PagerDutyClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// At least one credential is required. An empty API key or routing key is
    /// [`WirekitError::InvalidConfig`] rather than a request the provider would
    /// reject.
    pub fn new(client: WirekitClient, config: PagerDutyConfig) -> Result<Self, WirekitError> {
        let api_key = config
            .api_key
            .as_ref()
            .map(|key| key.expose().trim().to_owned());
        let routing_key = config
            .routing_key
            .as_ref()
            .map(|key| key.expose().trim().to_owned());

        if api_key.as_deref().is_none_or(str::is_empty)
            && routing_key.as_deref().is_none_or(str::is_empty)
        {
            return Err(WirekitError::invalid_config(
                "a PagerDuty API key or routing key is required",
            ));
        }
        if api_key.as_deref() == Some("") {
            return Err(WirekitError::invalid_config(
                "the PagerDuty API key must not be empty",
            ));
        }
        if routing_key.as_deref() == Some("") {
            return Err(WirekitError::invalid_config(
                "the PagerDuty routing key must not be empty",
            ));
        }
        if config
            .webhook_secret
            .as_ref()
            .is_some_and(|secret| secret.expose().trim().is_empty())
        {
            return Err(WirekitError::invalid_config(
                "the PagerDuty webhook secret must not be empty",
            ));
        }

        let rest_auth = api_key.map(|key| {
            Arc::new(
                ApiKeyHeader::new("authorization", format!("Token token={key}"))
                    .expect("authorization is a valid header name"),
            ) as Arc<dyn AuthScheme>
        });

        Ok(Self {
            inner: Arc::new(PagerDutyInner {
                client,
                config,
                rest_auth,
            }),
        })
    }

    /// The client configuration.
    #[must_use]
    pub fn config(&self) -> &PagerDutyConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Trigger an incident through the Events API.
    ///
    /// A repeat with the same `dedup_key` does not create a second incident; the
    /// provider returns the existing one. The request is never retried after the
    /// provider has accepted it.
    pub async fn trigger(&self, event: &IncidentEvent) -> Result<EventAck, WirekitError> {
        let routing_key = self.routing_key()?;
        if event.summary.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a PagerDuty event needs a summary",
            ));
        }
        if event.source.trim().is_empty() {
            return Err(WirekitError::invalid_config(
                "a PagerDuty event needs a source",
            ));
        }
        ensure_dedup_key(event.dedup_key.as_deref())?;

        let payload = WireEventPayload {
            summary: &event.summary,
            source: &event.source,
            severity: event.severity.as_str(),
            timestamp: event.timestamp.as_deref(),
            component: event.component.as_deref(),
            group: event.group.as_deref(),
            class: event.class.as_deref(),
            custom_details: event.custom_details.as_ref(),
        };
        self.send_event(WireEvent {
            routing_key,
            event_action: "trigger",
            dedup_key: event.dedup_key.as_deref(),
            payload: Some(payload),
        })
        .await
    }

    /// Acknowledge an incident by its deduplication key.
    pub async fn acknowledge(&self, dedup_key: &str) -> Result<EventAck, WirekitError> {
        ensure_dedup_key(Some(dedup_key))?;
        self.send_event(WireEvent {
            routing_key: self.routing_key()?,
            event_action: "acknowledge",
            dedup_key: Some(dedup_key),
            payload: None,
        })
        .await
    }

    /// Resolve an incident by its deduplication key.
    pub async fn resolve(&self, dedup_key: &str) -> Result<EventAck, WirekitError> {
        ensure_dedup_key(Some(dedup_key))?;
        self.send_event(WireEvent {
            routing_key: self.routing_key()?,
            event_action: "resolve",
            dedup_key: Some(dedup_key),
            payload: None,
        })
        .await
    }

    /// List incidents through the REST API.
    ///
    /// A page past the last record is an empty page with `more: false`, so a
    /// caller's loop terminates without an extra request.
    pub async fn list_incidents(
        &self,
        query: &IncidentQuery,
    ) -> Result<IncidentPage, WirekitError> {
        if query.limit == Some(0) {
            return Err(WirekitError::invalid_config(
                "an incident page needs a limit of at least one",
            ));
        }
        if query.limit.is_some_and(|limit| limit > 100) {
            return Err(WirekitError::invalid_config(
                "the PagerDuty REST API caps an incident page at 100",
            ));
        }

        let mut url = self
            .inner
            .client
            .url(&format!("{REST_BASE_URL}/incidents"))?;
        {
            let mut pairs = url.query_pairs_mut();
            for status in &query.statuses {
                pairs.append_pair("statuses[]", status.as_str());
            }
            for urgency in &query.urgencies {
                pairs.append_pair("urgencies[]", urgency.as_str());
            }
            if let Some(limit) = query.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(offset) = query.offset {
                pairs.append_pair("offset", &offset.to_string());
            }
        }

        let request = self.rest_request(url)?;
        let response: Response<WireIncidentEnvelope> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_pagerduty_error)?;

        Ok(IncidentPage {
            incidents: response.value.incidents,
            more: response.value.more,
            limit: response.value.limit,
            offset: response.value.offset,
        })
    }

    /// List the on-call entries through the REST API.
    ///
    /// Passing a schedule narrows the answer to that schedule; passing `None`
    /// asks for every schedule in the account. Only the current entry per
    /// schedule is requested, so the result is not a shift history.
    pub async fn on_calls(&self, schedule_id: Option<&str>) -> Result<Vec<OnCall>, WirekitError> {
        let mut url = self.inner.client.url(&format!("{REST_BASE_URL}/oncalls"))?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("earliest", "true");
            if let Some(schedule_id) = schedule_id {
                if schedule_id.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "a PagerDuty schedule id must not be blank",
                    ));
                }
                pairs.append_pair("schedule_ids[]", schedule_id.trim());
            }
        }

        let request = self.rest_request(url)?;
        let response: Response<WireOnCallEnvelope> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_pagerduty_error)?;

        Ok(response
            .value
            .oncalls
            .into_iter()
            .map(WireOnCall::into_on_call)
            .collect())
    }

    /// Verify an inbound webhook signature over the exact received bytes.
    ///
    /// PagerDuty v3 signs the raw body with HMAC-SHA256 and sends
    /// `X-PagerDuty-Signature: v1=<hex>`. The comparison is constant time and
    /// fails closed. The receiver still deduplicates by event id and responds
    /// within the delivery window; this crate stores no event state.
    pub fn verify_webhook(&self, raw: &[u8], signature: &str) -> Result<(), WirekitError> {
        let secret = self.inner.config.webhook_secret.as_ref().ok_or_else(|| {
            WirekitError::invalid_config(
                "verifying a PagerDuty webhook needs a configured webhook secret",
            )
        })?;

        let mut mac =
            <Hmac<Sha256> as Mac>::new_from_slice(secret.expose().as_bytes()).map_err(|_| {
                WirekitError::invalid_config("the webhook secret is not a valid HMAC key")
            })?;
        mac.update(raw);
        let expected = hex_lower(&mac.finalize().into_bytes());
        let expected = format!("v1={expected}");

        let matched = signature
            .split(',')
            .map(str::trim)
            .any(|candidate| constant_time_eq(expected.as_bytes(), candidate.as_bytes()));
        if !matched {
            return Err(WirekitError::auth(
                "the PagerDuty webhook signature did not match",
            ));
        }

        Ok(())
    }

    fn routing_key(&self) -> Result<&str, WirekitError> {
        self.inner
            .config
            .routing_key
            .as_ref()
            .map(|key| key.expose().trim())
            .ok_or_else(|| {
                WirekitError::invalid_config(
                    "the Events API needs a routing key, but none was configured",
                )
            })
    }

    fn rest_auth(&self) -> Result<Arc<dyn AuthScheme>, WirekitError> {
        self.inner.rest_auth.clone().ok_or_else(|| {
            WirekitError::invalid_config("the REST API needs an API key, but none was configured")
        })
    }

    fn rest_request(&self, url: url::Url) -> Result<RequestSpec, WirekitError> {
        let mut request = RequestSpec::get(url)
            .auth(self.rest_auth()?)
            .rate_limit_key("pagerduty")
            .body_error_hook(reject_error_envelope)
            .try_header("accept", ACCEPT_HEADER)?;
        if let Some(email) = self.inner.config.from_email.as_deref() {
            if email.trim().is_empty() {
                return Err(WirekitError::invalid_config(
                    "the PagerDuty from email must not be blank",
                ));
            }
            request = request.try_header("from", email)?;
        }
        Ok(request)
    }

    async fn send_event(&self, event: WireEvent<'_>) -> Result<EventAck, WirekitError> {
        let url = self.inner.client.url(&format!("{BASE_URL}/enqueue"))?;
        let request = RequestSpec::post(url)
            .rate_limit_key("pagerduty")
            .body_error_hook(reject_error_envelope)
            .json_body(&event)?;
        let response: Response<EventAck> = self
            .inner
            .client
            .execute(request)
            .await
            .map_err(map_pagerduty_error)?;

        Ok(response.value)
    }
}

fn ensure_dedup_key(key: Option<&str>) -> Result<(), WirekitError> {
    if key.is_some_and(|key| key.trim().is_empty()) {
        return Err(WirekitError::invalid_config(
            "a PagerDuty deduplication key must not be blank",
        ));
    }
    Ok(())
}

fn hex_lower(bytes: &[u8]) -> String {
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

/// Reject a 2xx response whose body reports a failure (CON-213).
fn reject_error_envelope(status: u16, body: &[u8]) -> Option<WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;

    if let Some(error) = value.get("error") {
        let message = error
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("PagerDuty reported a failure");
        let code = error
            .get("code")
            .map(|code| code.to_string())
            .unwrap_or_else(|| String::from("pagerduty_error"));
        return Some(WirekitError::api_with_details(
            status,
            code,
            message,
            value.clone(),
        ));
    }

    let event_status = value.get("status").and_then(serde_json::Value::as_str);
    if matches!(event_status, Some("invalid event" | "error")) {
        let message = value
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("PagerDuty rejected the event");
        return Some(WirekitError::api_with_details(
            status,
            "invalid_event",
            message,
            value.clone(),
        ));
    }

    None
}

/// Map a PagerDuty rejection onto the shared error type.
///
/// The REST envelope nests its text under `error.message`; the Events API puts
/// it at the top level with a non-success `status`. Both are lifted out so the
/// caller sees the provider's text. A 401 or 403 becomes an authentication
/// failure.
fn map_pagerduty_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        details: Some(details),
        ..
    } = &error
    else {
        return error;
    };

    if *status == 401 || *status == 403 {
        return WirekitError::auth("PagerDuty rejected the credential");
    }

    if let Some(nested) = details.get("error") {
        let message = nested
            .get("message")
            .and_then(serde_json::Value::as_str)
            .unwrap_or("PagerDuty reported a failure");
        let code = nested
            .get("code")
            .map(|code| code.to_string())
            .unwrap_or_else(|| String::from("pagerduty_error"));
        return WirekitError::api_with_details(*status, code, message, details.clone());
    }

    if let Some(message) = details.get("message").and_then(serde_json::Value::as_str) {
        return WirekitError::api_with_details(
            *status,
            "pagerduty_error",
            message,
            details.clone(),
        );
    }

    error
}

#[derive(Debug, Serialize)]
struct WireEvent<'a> {
    routing_key: &'a str,
    event_action: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    dedup_key: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    payload: Option<WireEventPayload<'a>>,
}

#[derive(Debug, Serialize)]
struct WireEventPayload<'a> {
    summary: &'a str,
    source: &'a str,
    severity: &'a str,
    #[serde(skip_serializing_if = "Option::is_none")]
    timestamp: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    component: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    group: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    class: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    custom_details: Option<&'a serde_json::Value>,
}

#[derive(Debug, Default, Deserialize)]
struct WireIncidentEnvelope {
    #[serde(default)]
    incidents: Vec<Incident>,
    #[serde(default)]
    more: bool,
    #[serde(default)]
    limit: u32,
    #[serde(default)]
    offset: u32,
}

#[derive(Debug, Default, Deserialize)]
struct WireOnCallEnvelope {
    #[serde(default)]
    oncalls: Vec<WireOnCall>,
}

#[derive(Debug, Default, Deserialize)]
struct WireOnCall {
    #[serde(default)]
    id: String,
    #[serde(default)]
    escalation_policy: Option<EscalationPolicy>,
    #[serde(default)]
    schedule: Option<WireReference>,
    #[serde(default)]
    user: Option<WireReference>,
    #[serde(default)]
    start: Option<String>,
    #[serde(default)]
    end: Option<String>,
}

impl WireOnCall {
    fn into_on_call(self) -> OnCall {
        OnCall {
            id: self.id,
            escalation_policy: self.escalation_policy,
            schedule_id: self.schedule.map(|reference| reference.id),
            user_id: self.user.map(|reference| reference.id),
            start: self.start,
            end: self.end,
        }
    }
}

#[derive(Debug, Default, Deserialize)]
struct WireReference {
    #[serde(default)]
    id: String,
}
