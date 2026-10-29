//! Google Local Services Ads reporting client.
//!
//! Scope: the two read-only search methods the public API actually exposes -
//! `v1/accountReports:search` and `v1/detailedLeadReports:search` - with typed
//! pages, an explicit account-not-approved error for HTTP 403, and per-page
//! pagination.
//!
//! Non-goals, and this is deliberate: changing a lead's status, reporting a
//! booking, and receiving notifications. The published `localservices:v1`
//! discovery document defines exactly two resources, `accountReports` and
//! `detailedLeadReports`, both with a single `:search` method and the single OAuth
//! scope `https://www.googleapis.com/auth/adwords`. There is no mutate endpoint to
//! wrap, so this module does not pretend to offer one (CON-704).
//!
//! Credentials: one OAuth2 access token with the `adwords` scope, supplied by the
//! caller. Token acquisition belongs to `wirekit::google_oauth`; this module never
//! mints or refreshes a token itself.

use serde::Deserialize;

use crate::pipeline::RequestSpec;
use crate::{WirekitClient, WirekitError};

/// The published Local Services Ads API host.
pub const LSA_BASE_URL: &str = "https://localservices.googleapis.com/";

/// The only OAuth scope the Local Services Ads API accepts.
pub const LSA_SCOPE: &str = "https://www.googleapis.com/auth/adwords";

/// Stable wirekit code for "the Ads account is not linked or not approved".
pub const CODE_ACCOUNT_NOT_APPROVED: &str = "LOCALSERVICES_ACCOUNT_NOT_APPROVED";

/// A calendar date as the API takes it: three separate query parameters.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ApiDate {
    /// Four-digit year, or 0 to omit a year.
    pub year: u32,
    /// Month 1-12, or 0 for a year-only date.
    pub month: u32,
    /// Day 1-31, or 0 for a month-only date.
    pub day: u32,
}

impl ApiDate {
    /// Build a full date.
    #[must_use]
    pub fn new(year: u32, month: u32, day: u32) -> Self {
        Self { year, month, day }
    }
}

/// One page of account reports.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountReportsPage {
    /// One report per linked GLS account.
    #[serde(default)]
    pub account_reports: Vec<AccountReport>,
    /// Token for the next page, when more pages exist.
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// Aggregate performance for one linked GLS account.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AccountReport {
    /// Unique identifier of the GLS account.
    #[serde(default)]
    pub account_id: Option<String>,
    /// Business name on the account.
    #[serde(default)]
    pub business_name: Option<String>,
    /// Currency of every cost field.
    #[serde(default)]
    pub currency_code: Option<String>,
    /// Charged leads in the requested period.
    #[serde(default)]
    pub current_period_charged_leads: Option<String>,
    /// Total cost in the requested period.
    #[serde(default)]
    pub current_period_total_cost: Option<f64>,
    /// Phone calls in the requested period, connected and unconnected.
    #[serde(default)]
    pub current_period_phone_calls: Option<String>,
    /// Average review rating, 1 to 5.
    #[serde(default)]
    pub average_five_star_rating: Option<f64>,
    /// Total reviews on the account up to the request date.
    #[serde(default)]
    pub total_review: Option<i32>,
}

/// One page of detailed lead reports.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedLeadReportsPage {
    /// One row per lead.
    #[serde(default)]
    pub detailed_lead_reports: Vec<DetailedLeadReport>,
    /// Token for the next page, when more pages exist.
    #[serde(default)]
    pub next_page_token: Option<String>,
}

/// A single lead as the reporting API exposes it.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DetailedLeadReport {
    /// Lead identifier.
    #[serde(default)]
    pub lead_id: Option<String>,
    /// Lead type, for example message or phone.
    #[serde(default)]
    pub lead_type: Option<String>,
    /// Account the lead belongs to.
    #[serde(default)]
    pub account_id: Option<String>,
    /// When the lead was created.
    #[serde(default)]
    pub lead_creation_timestamp: Option<String>,
    /// Message-lead detail, when the lead came in as a message.
    #[serde(default)]
    pub message_lead: Option<MessageLead>,
    /// Phone-lead detail, when the lead came in by phone.
    #[serde(default)]
    pub phone_lead: Option<PhoneLead>,
}

/// Customer detail attached to a message lead.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MessageLead {
    /// Job type the customer asked about.
    #[serde(default)]
    pub job_type: Option<String>,
    /// Postal code of the customer.
    #[serde(default)]
    pub postal_code: Option<String>,
    /// Customer phone number.
    #[serde(default)]
    pub consumer_phone_number: Option<String>,
    /// Customer name.
    #[serde(default)]
    pub customer_name: Option<String>,
}

/// Call detail attached to a phone lead.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PhoneLead {
    /// Duration of the charged call.
    #[serde(default)]
    pub charged_connected_call_duration_seconds: Option<String>,
    /// Consumer phone number.
    #[serde(default)]
    pub consumer_phone_number: Option<String>,
    /// When the charged call happened.
    #[serde(default)]
    pub charged_call_timestamp: Option<String>,
}

/// The query shared by both search methods.
#[derive(Debug, Clone, Default)]
pub struct SearchPage {
    /// Inclusive start date.
    pub start: ApiDate,
    /// Inclusive end date.
    pub end: ApiDate,
    /// Rows per page. The API defaults to 1000 and caps at 10000.
    pub page_size: Option<u32>,
    /// Token from a previous page.
    pub page_token: Option<String>,
    /// Override the whole query string, for expressions this struct does not model.
    pub query: Option<String>,
}

impl SearchPage {
    /// A page for one manager customer over an inclusive date range.
    #[must_use]
    pub fn between(start: ApiDate, end: ApiDate) -> Self {
        Self {
            start,
            end,
            page_size: None,
            page_token: None,
            query: None,
        }
    }

    /// The query string sent to the API.
    ///
    /// The documented form is `manager_customer_id:<id>`, and the API rejects a
    /// request without one, so it is always included unless the caller supplied a
    /// full query.
    fn resolve_query(&self, manager_customer_id: &str) -> String {
        self.query
            .clone()
            .unwrap_or_else(|| format!("manager_customer_id:{}", manager_customer_id.trim()))
    }
}

/// A read-only Local Services Ads client.
///
/// One instance carries one tenant's access token; nothing is cached or shared, so
/// two tenants cannot observe each other (NFR-002). The token is held in
/// [`Secret`](crate::Secret), so `Debug` renders `***`.
#[derive(Debug)]
pub struct GoogleLsaClient {
    client: WirekitClient,
    access_token: crate::Secret<String>,
}

impl GoogleLsaClient {
    /// Build a client from a pooled HTTP client and an OAuth2 access token that
    /// carries [`LSA_SCOPE`].
    #[must_use]
    pub fn new(client: WirekitClient, access_token: crate::Secret<String>) -> Self {
        Self {
            client,
            access_token,
        }
    }

    /// Search account-level reports.
    pub async fn account_reports(
        &self,
        manager_customer_id: &str,
        page: &SearchPage,
    ) -> Result<AccountReportsPage, WirekitError> {
        let body = self
            .search("v1/accountReports:search", manager_customer_id, page)
            .await?;
        serde_json::from_slice(&body).map_err(|error| WirekitError::Decode {
            message: format!("localservices accountReports response did not match: {error}"),
            source: Some(Box::new(error)),
        })
    }

    /// Search per-lead detail reports.
    pub async fn detailed_lead_reports(
        &self,
        manager_customer_id: &str,
        page: &SearchPage,
    ) -> Result<DetailedLeadReportsPage, WirekitError> {
        let body = self
            .search("v1/detailedLeadReports:search", manager_customer_id, page)
            .await?;
        serde_json::from_slice(&body).map_err(|error| WirekitError::Decode {
            message: format!("localservices detailedLeadReports response did not match: {error}"),
            source: Some(Box::new(error)),
        })
    }

    /// Run one documented `:search` call and return the raw JSON bytes.
    async fn search(
        &self,
        path: &str,
        manager_customer_id: &str,
        page: &SearchPage,
    ) -> Result<Vec<u8>, WirekitError> {
        if manager_customer_id.trim().is_empty() && page.query.is_none() {
            return Err(WirekitError::InvalidConfig {
                message: String::from(
                    "localservices requires a manager customer id, or a full query string",
                ),
            });
        }

        let mut url = self.client.url(path)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("query", &page.resolve_query(manager_customer_id));
            pairs.append_pair("startDate.year", &page.start.year.to_string());
            pairs.append_pair("startDate.month", &page.start.month.to_string());
            pairs.append_pair("startDate.day", &page.start.day.to_string());
            pairs.append_pair("endDate.year", &page.end.year.to_string());
            pairs.append_pair("endDate.month", &page.end.month.to_string());
            pairs.append_pair("endDate.day", &page.end.day.to_string());
            if let Some(size) = page.page_size {
                pairs.append_pair("pageSize", &size.to_string());
            }
            if let Some(token) = page.page_token.as_deref() {
                pairs.append_pair("pageToken", token);
            }
        }

        let spec = RequestSpec::get(url)
            .try_header(
                "authorization",
                &format!("Bearer {}", self.access_token.expose()),
            )?
            .try_header("accept", "application/json")?
            .rate_limit_key("google-lsa");

        // The core client turns non-2xx responses into an error before `send`
        // returns, so the 403 special case has to be caught here rather than on a
        // RawResponse that never arrives (caught by ac_028).
        let raw = match self.client.send(spec).await {
            Ok(raw) => raw,
            Err(WirekitError::Api { status: 403, .. }) => {
                return Err(WirekitError::Api {
                    status: 403,
                    code: String::from(CODE_ACCOUNT_NOT_APPROVED),
                    message: String::from(
                        "the Google Ads account is not linked to this manager or its access is not approved",
                    ),
                    details: None,
                });
            }
            Err(other) => return Err(other),
        };

        if raw.status >= 400 {
            return Err(WirekitError::Api {
                status: raw.status,
                code: String::from("LOCALSERVICES_HTTP_ERROR"),
                message: String::from("localservices rejected the search request"),
                details: serde_json::from_slice(&raw.body).ok(),
            });
        }

        Ok(raw.body)
    }
}
