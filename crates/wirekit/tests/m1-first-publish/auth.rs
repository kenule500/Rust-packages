//! Acceptance tests for the auth layer (AC-021 to AC-027).

use std::sync::Arc;
use std::time::Duration;

use serde_json::json;
use wirekit::auth::{constant_time_eq, hmac_sha256_base64, HmacAlgorithm};
use wirekit::pipeline::RequestSpec;
use wirekit::sendgrid::{EmailAddress, SendGridConfig};
use wirekit::{
    ApiKeyHeader, ApiKeyQuery, BasicAuth, BearerAuth, HmacAuth, OAuth2ClientCredentials, Secret,
};

use crate::common;

#[tokio::test]
async fn ac_021_a_bearer_token_becomes_the_authorization_header() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/me",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;

    let client = common::client(&server);
    let spec = RequestSpec::get(client.url("v1/me").expect("the request URL"))
        .auth(Arc::new(BearerAuth::new("tok_abc")));
    client.send(spec).await.expect("the request is sent");

    assert_eq!(
        recorder.last().header("authorization"),
        Some("Bearer tok_abc")
    );
}

#[tokio::test]
async fn ac_022_basic_credentials_are_base64_encoded() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/secure",
        common::Recorder::json(200, json!({})),
    )
    .await;

    let client = common::client(&server);
    let spec = RequestSpec::get(client.url("v1/secure").expect("the request URL"))
        .auth(Arc::new(BasicAuth::new("AC123", "secret")));
    client.send(spec).await.expect("the request is sent");

    assert_eq!(
        recorder.last().header("authorization").map(str::to_string),
        Some(format!("Basic {}", common::TWILIO_BASIC_AUTH)),
        "the header is base64(user:password)"
    );
}

#[tokio::test]
async fn ac_023_an_api_key_in_a_header_reaches_the_provider() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/keyed",
        common::Recorder::json(200, json!({})),
    )
    .await;

    let client = common::client(&server);
    let scheme = ApiKeyHeader::new("x-api-key", "key_abc").expect("a valid header name");
    let spec =
        RequestSpec::get(client.url("v1/keyed").expect("the request URL")).auth(Arc::new(scheme));
    client.send(spec).await.expect("the request is sent");

    let request = recorder.last();
    assert_eq!(request.header("x-api-key"), Some("key_abc"));
    assert!(
        request.header("authorization").is_none(),
        "a header key does not invent an Authorization header"
    );
}

#[tokio::test]
async fn ac_024_an_api_key_in_the_query_string_is_percent_encoded() {
    let server = common::server().await;
    let plain = common::mount(
        &server,
        "GET",
        "/v1/plain",
        common::Recorder::json(200, json!({})),
    )
    .await;
    let escaped = common::mount(
        &server,
        "GET",
        "/v1/escaped",
        common::Recorder::json(200, json!({})),
    )
    .await;

    let client = common::client(&server);

    let spec = RequestSpec::get(client.url("v1/plain").expect("the request URL"))
        .auth(Arc::new(ApiKeyQuery::new("api_key", "key_abc")));
    client.send(spec).await.expect("the request is sent");
    assert_eq!(
        plain.last().query_param("api_key").as_deref(),
        Some("key_abc")
    );

    let spec = RequestSpec::get(client.url("v1/escaped").expect("the request URL"))
        .auth(Arc::new(ApiKeyQuery::new("api_key", "a&b=c d")));
    client.send(spec).await.expect("the request is sent");
    assert_eq!(
        escaped.last().query_param("api_key").as_deref(),
        Some("a%26b%3Dc%20d"),
        "a value containing &, =, or a space cannot alter the query"
    );

    let empty = RequestSpec::get(client.url("v1/plain").expect("the request URL"))
        .auth(Arc::new(ApiKeyQuery::new("api_key", "")));
    let error = client
        .send(empty)
        .await
        .expect_err("an empty key fails closed");
    assert_eq!(error.code(), "W2006");
}

#[tokio::test]
async fn ac_025_the_hmac_scheme_signs_the_canonical_request_bytes() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "POST",
        "/v1/signed",
        common::Recorder::json(200, json!({})),
    )
    .await;

    let client = common::client(&server);
    let url = client.url("v1/signed").expect("the request URL");
    let body = br#"{"amount":42}"#.to_vec();
    let canonical = format!("POST\n{url}\n{}", String::from_utf8_lossy(&body));
    let expected = hmac_sha256_base64(b"signing-key", canonical.as_bytes());

    let scheme = HmacAuth::new(
        b"signing-key".to_vec(),
        "x-signature",
        HmacAlgorithm::Sha256,
    )
    .expect("a valid header name");
    let spec = RequestSpec::post(url)
        .auth(Arc::new(scheme))
        .bytes_body(body);
    client.send(spec).await.expect("the request is sent");

    let request = recorder.last();
    assert_eq!(request.header("x-signature"), Some(expected.as_str()));
    assert_eq!(request.text(), r#"{"amount":42}"#);

    assert!(constant_time_eq(expected.as_bytes(), expected.as_bytes()));
    assert!(!constant_time_eq(expected.as_bytes(), b"something-else"));
    assert!(!constant_time_eq(b"short", b"a-longer-value"));
}

#[tokio::test]
async fn ac_026_oauth2_fails_closed_and_fetches_a_token_before_the_request() {
    let error = OAuth2ClientCredentials::new("http://127.0.0.1:9/token", "id", "secret")
        .expect_err("a plain-HTTP token URL must be rejected");
    assert_eq!(error.code(), "W2006");
    assert!(error.to_string().contains("https"));

    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/oauth",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;
    let client = common::builder(&server)
        .timeout(Duration::from_secs(2))
        .build()
        .expect("the client builds");

    // Missing credentials fail before any socket is opened.
    let scheme = OAuth2ClientCredentials::new("https://127.0.0.1:1/token", "", "")
        .expect("a syntactically valid token URL")
        .with_scopes(["read:things"])
        .with_refresh_margin(Duration::from_secs(30));
    let spec =
        RequestSpec::get(client.url("v1/oauth").expect("the request URL")).auth(Arc::new(scheme));
    let error = client.send(spec).await.expect_err("no credentials");
    assert_eq!(error.code(), "W2006");
    assert_eq!(recorder.hits(), 0, "no API request is sent");

    // With no cached token the scheme fetches one first; here the endpoint is
    // closed, so the fetch is what fails.
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("a free port");
    let port = listener.local_addr().expect("the bound address").port();
    drop(listener);
    let token_url = format!("https://127.0.0.1:{port}/token");
    let scheme =
        OAuth2ClientCredentials::new(token_url, "client-id", "client-secret").expect("a valid URL");
    let spec =
        RequestSpec::get(client.url("v1/oauth").expect("the request URL")).auth(Arc::new(scheme));
    let error = client
        .send(spec)
        .await
        .expect_err("the token endpoint is unreachable");
    assert_eq!(
        error.code(),
        "W2005",
        "the token fetch happens before the API request: {error}"
    );
    assert_eq!(
        recorder.hits(),
        0,
        "the API request is not sent without a token"
    );
}

#[test]
fn ac_027_secrets_render_as_stars_everywhere() {
    let secret = Secret::new(String::from("sk_live_VALUE"));
    assert_eq!(format!("{secret}"), "***");
    assert_eq!(format!("{secret:?}"), "***");
    assert_eq!(secret.expose(), "sk_live_VALUE");
    assert_eq!(secret.clone().into_inner(), "sk_live_VALUE");

    let schemes: Vec<Box<dyn std::fmt::Debug>> = vec![
        Box::new(BearerAuth::new("bearer_VALUE")),
        Box::new(BasicAuth::new("basic_USER", "basic_VALUE")),
        Box::new(ApiKeyHeader::new("x-api-key", "header_VALUE").expect("a valid header name")),
        Box::new(ApiKeyQuery::new("api_key", "query_VALUE")),
        Box::new(
            HmacAuth::new(b"hmac_VALUE".to_vec(), "x-signature", HmacAlgorithm::Sha256)
                .expect("a valid header name"),
        ),
        Box::new(
            OAuth2ClientCredentials::new(
                "https://example.com/token",
                "client_VALUE",
                "secret_VALUE",
            )
            .expect("a valid token URL"),
        ),
    ];
    for scheme in schemes {
        let rendered = format!("{scheme:?}");
        for marker in [
            "bearer_VALUE",
            "basic_USER",
            "basic_VALUE",
            "header_VALUE",
            "query_VALUE",
            "hmac_VALUE",
            "client_VALUE",
            "secret_VALUE",
        ] {
            assert!(
                !rendered.contains(marker),
                "{marker} leaked into {rendered}"
            );
        }
        assert!(rendered.contains("***"), "the scheme redacts: {rendered}");
    }

    let config = SendGridConfig::new("SG.secret_VALUE", EmailAddress::new("from@example.com"));
    let rendered = format!("{config:?}");
    assert!(!rendered.contains("SG.secret_VALUE"), "{rendered}");
    assert!(rendered.contains("***"), "{rendered}");
}
