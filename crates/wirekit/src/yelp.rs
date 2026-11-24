//! Yelp — Fusion API business search, business details, and reviews.
//!
//! # Scope
//!
//! Built from `catalogue/19_yelp.md`: search businesses by term and place,
//! read one business, and list its reviews. Auth, the token bucket, the retry
//! policy with `Retry-After`, tracing, and the response size limit are the
//! shared ones.
//!
//! # Non-goals
//!
//! - **No leads surface.** `catalogue/19_yelp.md` marks Yelp's lead API as
//!   partner-gated and unconfirmed ("often partner-only", Open questions 1; the
//!   feature spec records the same risk as RISK-001). No lead endpoint is
//!   implemented, because a fabricated route is worse than an honest absence.
//!   A tenant without lead access imports leads manually or as CSV.
//! - **No webhook receiver.** Lead webhooks are partner-only; this crate opens
//!   no listening socket and stores no dedupe ledger.
//! - **No review posting or gating.** Yelp's terms prohibit it, and the
//!   catalogue's review data is read-only.
//! - **No caching or attribution UI.** A daily quota rewards caching and Yelp
//!   requires attribution when data is displayed; both belong to the consuming
//!   application (CON-302).
//!
//! # Credentials
//!
//! The Fusion token is a bearer token and is held in a [`Secret`], so it
//! renders as `***` and never appears in a traced header.
//!
//! ```no_run
//! use wirekit::yelp::{BusinessSearchRequest, YelpClient, YelpConfig};
//! use wirekit::{Secret, WirekitClient};
//!
//! # async fn run() -> Result<(), wirekit::WirekitError> {
//! let transport = WirekitClient::new(wirekit::yelp::BASE_URL)?;
//! let client = YelpClient::new(
//!     transport,
//!     YelpConfig::new(Secret::new(String::from("<fusion-token>"))),
//! )?;
//!
//! let page = client
//!     .search_businesses(&BusinessSearchRequest::near("Austin, TX").with_term("roofing"))
//!     .await?;
//! println!("{} businesses", page.total);
//! # Ok(())
//! # }
//! ```

use std::sync::Arc;

use serde::Deserialize;

use crate::auth::{AuthScheme, BearerAuth};
use crate::client::WirekitClient;
use crate::error::WirekitError;
use crate::pipeline::{RequestSpec, Response};
use crate::secret::Secret;

/// The Yelp Fusion API base URL.
pub const BASE_URL: &str = "https://api.yelp.com/v3";

/// The order a search result page is sorted in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortBy {
    /// Yelp's relevance ranking.
    BestMatch,
    /// Highest rated first.
    Rating,
    /// Most reviewed first.
    ReviewCount,
    /// Nearest first. Requires a coordinate search.
    Distance,
}

impl SortBy {
    /// The provider's wire value.
    #[must_use]
    pub fn as_str(self) -> &'static str {
        match self {
            Self::BestMatch => "best_match",
            Self::Rating => "rating",
            Self::ReviewCount => "review_count",
            Self::Distance => "distance",
        }
    }
}

/// How the client is configured.
#[derive(Debug, Clone)]
pub struct YelpConfig {
    /// The Fusion API token, sent as `Authorization: Bearer <token>`.
    pub api_key: Secret<String>,
    /// The locale for returned text, for example `en_US`.
    pub locale: Option<String>,
}

impl YelpConfig {
    /// Build a configuration with the token.
    #[must_use]
    pub fn new(api_key: Secret<String>) -> Self {
        Self {
            api_key,
            locale: None,
        }
    }

    /// Ask for text in a specific locale.
    #[must_use]
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = Some(locale.into());
        self
    }
}

/// A business search request.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct BusinessSearchRequest {
    /// What to search for, for example `roofing`.
    pub term: Option<String>,
    /// A place name, address, or postal code.
    pub location: Option<String>,
    /// Latitude for a coordinate search. Needs [`Self::longitude`].
    pub latitude: Option<f64>,
    /// Longitude for a coordinate search. Needs [`Self::latitude`].
    pub longitude: Option<f64>,
    /// Restrict to these category aliases.
    pub categories: Vec<String>,
    /// Restrict to these price tiers, each 1 to 4.
    pub price: Vec<u8>,
    /// The page size, 1 to 50. `None` uses the provider default of 20.
    pub limit: Option<u32>,
    /// The number of results to skip.
    pub offset: Option<u32>,
    /// The result order.
    pub sort_by: Option<SortBy>,
}

impl BusinessSearchRequest {
    /// Search around a place.
    #[must_use]
    pub fn near(location: impl Into<String>) -> Self {
        Self {
            location: Some(location.into()),
            ..Self::default()
        }
    }

    /// Search around a coordinate.
    #[must_use]
    pub fn at(latitude: f64, longitude: f64) -> Self {
        Self {
            latitude: Some(latitude),
            longitude: Some(longitude),
            ..Self::default()
        }
    }

    /// Set the search term.
    #[must_use]
    pub fn with_term(mut self, term: impl Into<String>) -> Self {
        self.term = Some(term.into());
        self
    }

    /// Restrict to a category alias.
    #[must_use]
    pub fn with_category(mut self, category: impl Into<String>) -> Self {
        self.categories.push(category.into());
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

    /// Set the result order.
    #[must_use]
    pub fn with_sort_by(mut self, sort_by: SortBy) -> Self {
        self.sort_by = Some(sort_by);
        self
    }
}

/// An address as Yelp renders it.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct BusinessLocation {
    /// The first street line.
    #[serde(default)]
    pub address1: Option<String>,
    /// The second street line.
    #[serde(default)]
    pub address2: Option<String>,
    /// The third street line.
    #[serde(default)]
    pub address3: Option<String>,
    /// The city.
    #[serde(default)]
    pub city: Option<String>,
    /// The postal code.
    #[serde(default)]
    pub zip_code: Option<String>,
    /// The country code.
    #[serde(default)]
    pub country: Option<String>,
    /// The state or province code.
    #[serde(default)]
    pub state: Option<String>,
    /// The address lines already formatted for display.
    #[serde(default)]
    pub display_address: Vec<String>,
}

/// A business category.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct Category {
    /// The category alias, for example `roofing`.
    #[serde(default)]
    pub alias: String,
    /// The display title.
    #[serde(default)]
    pub title: String,
}

/// A business coordinate pair.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Coordinates {
    /// Latitude in degrees.
    #[serde(default)]
    pub latitude: f64,
    /// Longitude in degrees.
    #[serde(default)]
    pub longitude: f64,
}

/// A business from a search or a detail lookup.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Business {
    /// The provider's identifier.
    #[serde(default)]
    pub id: String,
    /// The URL-safe alias.
    #[serde(default)]
    pub alias: Option<String>,
    /// The business name.
    #[serde(default)]
    pub name: Option<String>,
    /// The header image.
    #[serde(default)]
    pub image_url: Option<String>,
    /// Whether the business is marked as closed.
    #[serde(default)]
    pub is_closed: Option<bool>,
    /// The Yelp page URL.
    #[serde(default)]
    pub url: Option<String>,
    /// How many reviews the business has.
    #[serde(default)]
    pub review_count: Option<u32>,
    /// The average rating, between 1 and 5.
    #[serde(default)]
    pub rating: Option<f64>,
    /// The price tier, for example `$$`.
    #[serde(default)]
    pub price: Option<String>,
    /// The phone number in provider form.
    #[serde(default)]
    pub phone: Option<String>,
    /// The phone number formatted for display.
    #[serde(default)]
    pub display_phone: Option<String>,
    /// Distance from the search coordinate, in metres.
    #[serde(default)]
    pub distance: Option<f64>,
    /// The address.
    #[serde(default)]
    pub location: Option<BusinessLocation>,
    /// The categories the business belongs to.
    #[serde(default)]
    pub categories: Vec<Category>,
    /// The coordinate pair.
    #[serde(default)]
    pub coordinates: Option<Coordinates>,
    /// The transaction types the business supports.
    #[serde(default)]
    pub transactions: Vec<String>,
}

/// One page of a business search.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct BusinessSearchPage {
    /// The businesses on this page.
    #[serde(default)]
    pub businesses: Vec<Business>,
    /// The total matches the provider reports, which may exceed this page.
    #[serde(default)]
    pub total: u32,
}

/// The person who wrote a review.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct ReviewUser {
    /// The provider's identifier.
    #[serde(default)]
    pub id: String,
    /// The display name.
    #[serde(default)]
    pub name: Option<String>,
    /// The avatar image.
    #[serde(default)]
    pub image_url: Option<String>,
    /// The Yelp profile URL.
    #[serde(default)]
    pub profile_url: Option<String>,
}

/// A review of a business.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct Review {
    /// The provider's identifier.
    #[serde(default)]
    pub id: String,
    /// The Yelp page URL.
    #[serde(default)]
    pub url: Option<String>,
    /// The review text.
    #[serde(default)]
    pub text: Option<String>,
    /// The rating, between 1 and 5.
    #[serde(default)]
    pub rating: Option<f64>,
    /// When the review was written, as the provider formats it.
    #[serde(default)]
    pub time_created: Option<String>,
    /// The reviewer.
    #[serde(default)]
    pub user: Option<ReviewUser>,
}

/// One page of reviews.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
pub struct ReviewPage {
    /// The reviews on this page.
    #[serde(default)]
    pub reviews: Vec<Review>,
    /// The total the provider reports for the business.
    #[serde(default)]
    pub total: u32,
    /// Other languages the reviews are available in.
    #[serde(default)]
    pub possible_languages: Vec<String>,
}

struct YelpInner {
    client: WirekitClient,
    config: YelpConfig,
    auth: Arc<dyn AuthScheme>,
}

/// The Yelp client. Clone it freely; the transport is shared.
#[derive(Clone)]
pub struct YelpClient {
    inner: Arc<YelpInner>,
}

impl YelpClient {
    /// Build a client over a shared [`WirekitClient`].
    ///
    /// Build the underlying client against [`BASE_URL`]. An empty token is
    /// [`WirekitError::InvalidConfig`].
    pub fn new(client: WirekitClient, config: YelpConfig) -> Result<Self, WirekitError> {
        if config.api_key.expose().trim().is_empty() {
            return Err(WirekitError::invalid_config("a Yelp token is required"));
        }
        if config
            .locale
            .as_deref()
            .is_some_and(|locale| locale.trim().is_empty())
        {
            return Err(WirekitError::invalid_config(
                "a Yelp locale must not be blank",
            ));
        }

        let auth: Arc<dyn AuthScheme> = Arc::new(BearerAuth::new(config.api_key.expose().clone()));
        Ok(Self {
            inner: Arc::new(YelpInner {
                client,
                config,
                auth,
            }),
        })
    }

    /// The client configuration.
    #[must_use]
    pub fn config(&self) -> &YelpConfig {
        &self.inner.config
    }

    /// The shared transport.
    #[must_use]
    pub fn client(&self) -> &WirekitClient {
        &self.inner.client
    }

    /// Search businesses.
    ///
    /// A request needs a place or a coordinate pair; both missing, a
    /// half-specified pair, or a page size outside 1 to 50 is
    /// [`WirekitError::InvalidConfig`] rather than a provider rejection.
    pub async fn search_businesses(
        &self,
        request: &BusinessSearchRequest,
    ) -> Result<BusinessSearchPage, WirekitError> {
        self.validate_search(request)?;

        let mut url = self.inner.client.url("businesses/search")?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(term) = request.term.as_deref() {
                if term.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "a Yelp search term must not be blank",
                    ));
                }
                pairs.append_pair("term", term.trim());
            }
            if let Some(location) = request.location.as_deref() {
                if location.trim().is_empty() {
                    return Err(WirekitError::invalid_config(
                        "a Yelp search location must not be blank",
                    ));
                }
                pairs.append_pair("location", location.trim());
            }
            if let Some(latitude) = request.latitude {
                pairs.append_pair("latitude", &latitude.to_string());
            }
            if let Some(longitude) = request.longitude {
                pairs.append_pair("longitude", &longitude.to_string());
            }
            if !request.categories.is_empty() {
                pairs.append_pair("categories", &request.categories.join(","));
            }
            if !request.price.is_empty() {
                let price = request
                    .price
                    .iter()
                    .map(u8::to_string)
                    .collect::<Vec<_>>()
                    .join(",");
                pairs.append_pair("price", &price);
            }
            if let Some(limit) = request.limit {
                pairs.append_pair("limit", &limit.to_string());
            }
            if let Some(offset) = request.offset {
                pairs.append_pair("offset", &offset.to_string());
            }
            if let Some(sort_by) = request.sort_by {
                pairs.append_pair("sort_by", sort_by.as_str());
            }
            if let Some(locale) = self.inner.config.locale.as_deref() {
                pairs.append_pair("locale", locale.trim());
            }
        }

        let spec = self.request(url);
        let response: Response<BusinessSearchPage> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_yelp_error)?;

        Ok(response.value)
    }

    /// Read one business.
    pub async fn get_business(&self, id: &str) -> Result<Business, WirekitError> {
        ensure_identifier(id, "business")?;

        let mut url = self
            .inner
            .client
            .url(&format!("businesses/{}", id.trim()))?;
        self.append_locale(&mut url);
        let spec = self.request(url);
        let response: Response<Business> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_yelp_error)?;

        Ok(response.value)
    }

    /// List the reviews the provider exposes for a business.
    ///
    /// The Fusion API returns a subset of a business's reviews; the `total` on
    /// the page is the full count, not the size of this page.
    pub async fn list_reviews(&self, business_id: &str) -> Result<ReviewPage, WirekitError> {
        ensure_identifier(business_id, "business")?;

        let mut url = self
            .inner
            .client
            .url(&format!("businesses/{}/reviews", business_id.trim()))?;
        self.append_locale(&mut url);
        let spec = self.request(url);
        let response: Response<ReviewPage> = self
            .inner
            .client
            .execute(spec)
            .await
            .map_err(map_yelp_error)?;

        Ok(response.value)
    }

    fn validate_search(&self, request: &BusinessSearchRequest) -> Result<(), WirekitError> {
        let has_location = request
            .location
            .as_deref()
            .is_some_and(|location| !location.trim().is_empty());
        let has_latitude = request.latitude.is_some();
        let has_longitude = request.longitude.is_some();

        if has_latitude != has_longitude {
            return Err(WirekitError::invalid_config(
                "a Yelp coordinate search needs both a latitude and a longitude",
            ));
        }
        if !has_location && !has_latitude {
            return Err(WirekitError::invalid_config(
                "a Yelp search needs a location or a coordinate pair",
            ));
        }
        if let Some(latitude) = request.latitude {
            if !(-90.0..=90.0).contains(&latitude) {
                return Err(WirekitError::invalid_config(
                    "a Yelp latitude must be between -90 and 90",
                ));
            }
        }
        if let Some(longitude) = request.longitude {
            if !(-180.0..=180.0).contains(&longitude) {
                return Err(WirekitError::invalid_config(
                    "a Yelp longitude must be between -180 and 180",
                ));
            }
        }
        if request
            .limit
            .is_some_and(|limit| !(1..=50).contains(&limit))
        {
            return Err(WirekitError::invalid_config(
                "a Yelp page size must be between 1 and 50",
            ));
        }
        if request.price.iter().any(|tier| !(1..=4).contains(tier)) {
            return Err(WirekitError::invalid_config(
                "a Yelp price tier must be between 1 and 4",
            ));
        }
        if request
            .categories
            .iter()
            .any(|category| category.trim().is_empty())
        {
            return Err(WirekitError::invalid_config(
                "a Yelp category alias must not be blank",
            ));
        }

        Ok(())
    }

    fn append_locale(&self, url: &mut url::Url) {
        if let Some(locale) = self.inner.config.locale.as_deref() {
            url.query_pairs_mut().append_pair("locale", locale.trim());
        }
    }

    fn request(&self, url: url::Url) -> RequestSpec {
        RequestSpec::get(url)
            .auth(self.inner.auth.clone())
            .rate_limit_key("yelp")
            .body_error_hook(reject_error_envelope)
    }
}

fn ensure_identifier(id: &str, what: &str) -> Result<(), WirekitError> {
    let id = id.trim();
    if id.is_empty() {
        return Err(WirekitError::invalid_config(format!(
            "a Yelp {what} id is required"
        )));
    }
    if id.contains(['/', '?', '#']) {
        return Err(WirekitError::invalid_config(format!(
            "a Yelp {what} id must not contain a path or query separator"
        )));
    }
    Ok(())
}

/// Reject a 2xx response whose body reports a failure (CON-213).
fn reject_error_envelope(status: u16, body: &[u8]) -> Option<WirekitError> {
    let value: serde_json::Value = serde_json::from_slice(body).ok()?;
    let error = value.get("error")?;
    let code = error
        .get("code")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("yelp_error");
    let message = error
        .get("description")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Yelp reported a failure");

    Some(WirekitError::api_with_details(
        status,
        code,
        message,
        value.clone(),
    ))
}

/// Map a Yelp rejection onto the shared error type.
///
/// Yelp nests the reason under `error.code` and `error.description`; it is
/// lifted out so the caller sees the provider's text. A 401 or 403 becomes an
/// authentication failure.
fn map_yelp_error(error: WirekitError) -> WirekitError {
    let WirekitError::Api {
        status,
        details: Some(details),
        ..
    } = &error
    else {
        return error;
    };

    if *status == 401 || *status == 403 {
        return WirekitError::auth("Yelp rejected the token");
    }

    let nested = details.get("error");
    let code = nested
        .and_then(|error| error.get("code"))
        .and_then(serde_json::Value::as_str);
    let message = nested
        .and_then(|error| error.get("description"))
        .and_then(serde_json::Value::as_str);

    match (code, message) {
        (Some(code), Some(message)) => {
            WirekitError::api_with_details(*status, code, message, details.clone())
        }
        (None, Some(message)) => {
            WirekitError::api_with_details(*status, "yelp_error", message, details.clone())
        }
        _ => error,
    }
}
