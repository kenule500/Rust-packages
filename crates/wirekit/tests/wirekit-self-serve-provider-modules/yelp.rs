//! Acceptance tests for the Yelp module (AC-001 to AC-005).

use std::time::{Duration, Instant};

use serde_json::json;
use wirekit::yelp::{BusinessSearchRequest, SortBy, YelpClient, YelpConfig};
use wirekit::{Secret, WirekitError};
use wiremock::MockServer;

use crate::common::{self, Fixture};

fn config() -> YelpConfig {
    YelpConfig::new(Secret::new(String::from(common::API_KEY))).with_locale("en_US")
}

fn client(server: &MockServer) -> YelpClient {
    YelpClient::new(common::transport(server), config()).expect("the client builds")
}

fn business_json() -> serde_json::Value {
    json!({
        "id": "b1",
        "alias": "acme-roofing",
        "name": "Acme Roofing",
        "image_url": "https://example.com/acme.jpg",
        "is_closed": false,
        "url": "https://www.yelp.com/biz/acme-roofing",
        "review_count": 42,
        "rating": 4.5,
        "price": "$$",
        "phone": "+15125550100",
        "display_phone": "(512) 555-0100",
        "distance": 1200.5,
        "location": {
            "address1": "1 Main St",
            "city": "Austin",
            "zip_code": "78701",
            "country": "US",
            "state": "TX",
            "display_address": ["1 Main St", "Austin, TX 78701"]
        },
        "categories": [{ "alias": "roofing", "title": "Roofing" }],
        "coordinates": { "latitude": 30.2672, "longitude": -97.7431 },
        "transactions": ["pickup"]
    })
}

#[tokio::test]
async fn ac_001_yelp_surface_matches_the_catalogue() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/businesses/search",
        Fixture::json(200, json!({ "businesses": [business_json()], "total": 42 })),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/businesses/b1",
        Fixture::json(200, business_json()),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/businesses/b1/reviews",
        Fixture::json(
            200,
            json!({
                "reviews": [{
                    "id": "r1",
                    "url": "https://www.yelp.com/biz/acme-roofing?hrid=r1",
                    "text": "Great roof, no leaks.",
                    "rating": 5.0,
                    "time_created": "2026-11-01 10:00:00",
                    "user": {
                        "id": "u1",
                        "name": "Pat",
                        "image_url": "https://example.com/pat.jpg",
                        "profile_url": "https://www.yelp.com/user_details?userid=u1"
                    }
                }],
                "total": 12,
                "possible_languages": ["en"]
            }),
        ),
    )
    .await;

    let client = client(&server);

    let page = client
        .search_businesses(
            &BusinessSearchRequest::near("Austin, TX")
                .with_term("roofing")
                .with_category("roofing")
                .with_limit(5)
                .with_sort_by(SortBy::Rating),
        )
        .await
        .expect("the search succeeds");
    assert_eq!(page.total, 42);
    assert_eq!(page.businesses.len(), 1);
    let business = &page.businesses[0];
    assert_eq!(business.id, "b1");
    assert_eq!(business.name.as_deref(), Some("Acme Roofing"));
    assert_eq!(business.review_count, Some(42));
    assert_eq!(business.rating, Some(4.5));
    assert_eq!(business.categories[0].alias, "roofing");
    assert_eq!(
        business
            .location
            .as_ref()
            .map(|location| location.city.as_deref()),
        Some(Some("Austin"))
    );
    assert_eq!(
        business
            .coordinates
            .as_ref()
            .map(|coordinates| coordinates.latitude),
        Some(30.2672)
    );

    let details = client
        .get_business("b1")
        .await
        .expect("the lookup succeeds");
    assert_eq!(details.alias.as_deref(), Some("acme-roofing"));
    assert_eq!(details.is_closed, Some(false));

    let reviews = client
        .list_reviews("b1")
        .await
        .expect("the reviews succeed");
    assert_eq!(reviews.total, 12);
    assert_eq!(reviews.reviews[0].id, "r1");
    assert_eq!(reviews.reviews[0].rating, Some(5.0));
    assert_eq!(
        reviews.reviews[0]
            .user
            .as_ref()
            .map(|user| user.name.as_deref()),
        Some(Some("Pat"))
    );
    assert_eq!(reviews.possible_languages, vec![String::from("en")]);

    let requests = common::requests(&server).await;
    assert_eq!(requests.len(), 3);
    let search = common::query(&requests[0]);
    for expected in [
        "location=Austin%2C+TX",
        "term=roofing",
        "categories=roofing",
        "limit=5",
        "sort_by=rating",
        "locale=en_US",
    ] {
        assert!(
            search.contains(expected),
            "{expected} missing from {search}"
        );
    }
    assert!(common::query(&requests[1]).contains("locale=en_US"));
    assert!(common::query(&requests[2]).contains("locale=en_US"));
    assert_eq!(
        common::header(&requests[1], "authorization"),
        Some("Bearer test_credential_do_not_log_1234")
    );
}

#[tokio::test]
async fn ac_002_yelp_failures_are_wirekit_errors_and_bad_input_is_rejected() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/businesses/search",
        Fixture::json(
            401,
            json!({ "error": { "code": "TOKEN_INVALID", "description": "The token is invalid" } }),
        ),
    )
    .await;
    let error = client(&server)
        .search_businesses(&BusinessSearchRequest::near("Austin, TX"))
        .await
        .expect_err("a 401 is an error");
    assert!(matches!(error, WirekitError::Auth { .. }), "got {error:?}");

    // A half-specified coordinate pair is rejected before any request.
    let server = common::server().await;
    let request = BusinessSearchRequest {
        latitude: Some(30.0),
        ..BusinessSearchRequest::default()
    };
    let error = client(&server)
        .search_businesses(&request)
        .await
        .expect_err("a half-specified coordinate pair is invalid");
    assert!(
        matches!(error, WirekitError::InvalidConfig { .. }),
        "got {error:?}"
    );
    assert!(common::requests(&server).await.is_empty());
}

#[tokio::test]
async fn ac_003_yelp_rejects_an_error_body_at_http_200() {
    let server = common::server().await;
    common::mount(
        &server,
        "GET",
        "/businesses/b1/reviews",
        Fixture::json(
            200,
            json!({ "error": { "code": "BUSINESS_NOT_FOUND", "description": "No such business" } }),
        ),
    )
    .await;
    let error = client(&server)
        .list_reviews("b1")
        .await
        .expect_err("a 200 with an error envelope is not a success");
    assert!(
        matches!(error, WirekitError::Api { status: 200, .. }),
        "got {error:?}"
    );
    assert_eq!(error.api_code(), Some("BUSINESS_NOT_FOUND"));
}

#[tokio::test]
async fn ac_004_yelp_honours_retry_after_and_never_retries_another_4xx() {
    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "GET",
        "/businesses/search",
        Fixture::json(200, json!({ "businesses": [], "total": 0 }))
            .first_status(429)
            .header("Retry-After", "1"),
    )
    .await;

    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = YelpClient::new(transport, config()).expect("the client builds");

    let started = Instant::now();
    client
        .search_businesses(&BusinessSearchRequest::near("Austin, TX"))
        .await
        .expect("the retry succeeds");
    assert_eq!(fixture.hits(), 2);
    assert!(
        started.elapsed() >= Duration::from_millis(900),
        "Retry-After: 1 must delay the retry, elapsed {:?}",
        started.elapsed()
    );

    let server = common::server().await;
    let fixture = common::mount(
        &server,
        "GET",
        "/businesses/search",
        Fixture::json(
            400,
            json!({ "error": { "code": "VALIDATION_ERROR", "description": "Bad search" } }),
        ),
    )
    .await;
    let transport = common::builder(&server)
        .retry(common::one_retry())
        .build()
        .expect("the transport builds");
    let client = YelpClient::new(transport, config()).expect("the client builds");
    let _ = client
        .search_businesses(&BusinessSearchRequest::near("Austin, TX"))
        .await;
    assert_eq!(fixture.hits(), 1, "a 4xx other than 429 is not retried");
}

#[tokio::test]
async fn ac_005_yelp_secret_is_redacted() {
    let rendered = format!("{:?}", config());
    assert!(rendered.contains("***"));
    assert!(!rendered.contains(common::API_KEY));
}
