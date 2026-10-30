//! Acceptance tests for the `google-oauth-shared-helper` feature.
//!
//! Every test is either pure computation or a local `wiremock` server, so no live
//! Google account is involved and CI needs no credentials.

use std::collections::HashMap;
use std::time::{Duration, SystemTime};

use wirekit::google_oauth::{pkce_challenge, GoogleOAuthClient, MemoryTokenStore, TokenStore};
use wirekit::{Secret, WirekitClient};

const REDIRECT: &str = "https://app.example.com/cb";
const SCOPE: &str = "https://www.googleapis.com/auth/adwords";
const SECRET: &str = "client-secret-do-not-print";

fn client() -> GoogleOAuthClient {
    let http = WirekitClient::builder("https://oauth2.googleapis.com")
        .expect("an https base URL")
        .build()
        .expect("the client builds");

    GoogleOAuthClient::new(
        http,
        "client-123",
        Secret::new(String::from(SECRET)),
        REDIRECT,
        vec![String::from(SCOPE)],
    )
    .expect("the client builds")
}

fn mock_client(server: &wiremock::MockServer) -> GoogleOAuthClient {
    let auth = format!("{}/auth", server.uri());
    let token = format!("{}/token", server.uri());
    let revoke = format!("{}/revoke", server.uri());

    client()
        .with_endpoints(&auth, &token, &revoke)
        .expect("loopback endpoints are allowed")
}

fn query(request: &wirekit::google_oauth::AuthorizationRequest) -> HashMap<String, String> {
    request
        .url()
        .query_pairs()
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn token_body(expires_in: u64, refresh: bool) -> String {
    let refresh_field = if refresh {
        r#","refresh_token":"rt-1""#
    } else {
        ""
    };
    format!(
        r#"{{"access_token":"at-1","expires_in":{expires_in},"scope":"{SCOPE}"{refresh_field}}}"#
    )
}

async fn serve_token(body: &str, status: u16) -> wiremock::MockServer {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/token"))
        .respond_with(wiremock::ResponseTemplate::new(status).set_body_string(body))
        .mount(&server)
        .await;
    server
}

#[test]
fn ac_001_the_authorization_url_carries_state_and_an_s256_challenge() {
    let request = client().authorization_url().expect("a request is built");
    let pairs = query(&request);

    assert_eq!(
        request.url().origin().ascii_serialization(),
        "https://oauth2.googleapis.com"
    );
    assert_eq!(request.url().path(), "/o/oauth2/v2/auth");
    assert_eq!(pairs["response_type"], "code");
    assert_eq!(pairs["client_id"], "client-123");
    assert_eq!(pairs["redirect_uri"], REDIRECT);
    assert_eq!(pairs["scope"], SCOPE);
    assert_eq!(pairs["code_challenge_method"], "S256");
    assert_eq!(
        pairs["code_challenge"],
        pkce_challenge(request.code_verifier().expose()),
        "the challenge is the S256 digest of the verifier"
    );
    assert!(
        pairs["state"].len() >= 32,
        "the state is long enough to be random"
    );
}

#[test]
fn ac_002_each_request_uses_a_fresh_state_and_verifier() {
    let client = client();
    let first = client.authorization_url().expect("the first request");
    let second = client.authorization_url().expect("the second request");

    assert_ne!(first.state().expose(), second.state().expose());
    assert_ne!(
        first.code_verifier().expose(),
        second.code_verifier().expose()
    );
    assert!(
        !first.code_verifier().expose().contains(SECRET),
        "the verifier is not derived from the client secret"
    );

    assert!(client.validate_state(&first, first.state().expose()).is_ok());
    let error = client
        .validate_state(&first, second.state().expose())
        .expect_err("a foreign state must not validate");
    assert_eq!(error.code(), "W2002");
}

#[test]
fn ac_003_no_request_can_omit_the_pkce_challenge() {
    let client = client();

    // There is no argument that could ask for an implicit flow, so every request
    // this API can produce carries a challenge with method S256.
    for _ in 0..8 {
        let pairs = query(&client.authorization_url().expect("a request"));
        assert_eq!(pairs["code_challenge_method"], "S256");
        assert!(!pairs["code_challenge"].is_empty());
    }
}

#[tokio::test]
async fn ac_004_exchange_code_returns_a_typed_token_with_absolute_expiry() {
    let server = serve_token(&token_body(3600, true), 200).await;
    let client = mock_client(&server);
    let request = client.authorization_url().expect("a request");

    let token = client
        .exchange_code("auth-code-1", &request)
        .await
        .expect("the exchange succeeds");

    assert_eq!(token.access_token().expose(), "at-1");
    assert_eq!(
        token.refresh_token().map(Secret::expose).map(String::as_str),
        Some("rt-1")
    );
    assert_eq!(token.scopes(), &[String::from(SCOPE)]);

    let lifetime = token
        .expires_at()
        .duration_since(SystemTime::now())
        .expect("the expiry is in the future");
    assert!(lifetime <= Duration::from_secs(3600));
    assert!(lifetime > Duration::from_secs(3500));
}

#[tokio::test]
async fn ac_005_a_provider_error_becomes_an_auth_error_with_its_code() {
    let server = serve_token(
        r#"{"error":"invalid_grant","error_description":"Bad Request"}"#,
        400,
    )
    .await;
    let client = mock_client(&server);
    let request = client.authorization_url().expect("a request");

    let error = client
        .exchange_code("stale-code", &request)
        .await
        .expect_err("a rejected code must not return a token");

    assert!(error.is_auth(), "invalid_grant is an authentication failure");
    assert_eq!(error.code(), "W2002");
    assert!(error.to_string().contains("Bad Request"));
}

#[tokio::test]
async fn ac_006_an_expired_token_is_refreshed_once_through_the_store() {
    let server = wiremock::MockServer::start().await;

    // The first token request is the code exchange and is already expired; the
    // second is the refresh and is valid.
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/token"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(token_body(0, true)),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/token"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(token_body(3600, true)),
        )
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let request = client.authorization_url().expect("a request");
    let expired = client
        .exchange_code("auth-code", &request)
        .await
        .expect("the exchange succeeds");

    assert!(expired.is_expired(Duration::from_secs(60)), "the token is stale");

    let store = MemoryTokenStore::new();
    store.save("acct-1", &expired);

    let usable = client
        .token_for(&store, "acct-1")
        .await
        .expect("one refresh makes the token usable");

    assert!(!usable.is_expired(Duration::from_secs(60)));
    let cached = store.load("acct-1").expect("the store was updated");
    assert!(
        !cached.is_expired(Duration::from_secs(60)),
        "the refreshed token replaced the stale record"
    );
}

#[tokio::test]
async fn ac_006_a_token_without_a_refresh_token_fails_rather_than_looping() {
    let server = serve_token(&token_body(0, false), 200).await;
    let client = mock_client(&server);
    let request = client.authorization_url().expect("a request");
    let expired = client
        .exchange_code("auth-code", &request)
        .await
        .expect("the exchange succeeds");

    let store = MemoryTokenStore::new();
    store.save("acct-1", &expired);

    let error = client
        .token_for(&store, "acct-1")
        .await
        .expect_err("without a refresh token the caller must re-authorize");

    assert!(error.is_auth());
    assert!(error.to_string().contains("re-authorize"));

    let requests = server
        .received_requests()
        .await
        .expect("the mock records requests");
    assert_eq!(
        requests.len(),
        1,
        "only the exchange reached the provider; no retry loop"
    );
}

#[tokio::test]
async fn ac_007_revoke_calls_the_endpoint_once_and_clears_the_store() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/token"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(token_body(3600, true)),
        )
        .mount(&server)
        .await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .and(wiremock::matchers::path("/revoke"))
        .and(wiremock::matchers::body_string_contains("token=rt-1"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;

    let client = mock_client(&server);
    let request = client.authorization_url().expect("a request");
    let token = client
        .exchange_code("auth-code", &request)
        .await
        .expect("the exchange succeeds");

    let store = MemoryTokenStore::new();
    store.save("acct-1", &token);

    let revoked = client
        .revoke_stored(&store, "acct-1", Some("refresh_token"))
        .await
        .expect("revocation succeeds");

    assert!(revoked, "a stored token was revoked");
    assert!(store.load("acct-1").is_none(), "the record is removed");

    let again = client
        .revoke_stored(&store, "acct-1", None)
        .await
        .expect("revoking an absent token is not an error");
    assert!(!again, "nothing was left to revoke");
}

#[tokio::test]
async fn ac_008_no_secret_renders_in_debug_or_display() {
    let server = serve_token(&token_body(3600, true), 200).await;
    let client = mock_client(&server);
    let request = client.authorization_url().expect("a request");

    let rendered = format!("{client:?}");
    assert!(!rendered.contains(SECRET), "the client secret must not render");
    assert!(rendered.contains("***"), "the secret renders redacted");

    let rendered = format!("{request:?}");
    assert!(!rendered.contains(request.state().expose()));
    assert!(!rendered.contains(request.code_verifier().expose()));

    let token = client
        .exchange_code("auth-code", &request)
        .await
        .expect("the exchange succeeds");
    let rendered = format!("{token:?}");
    assert!(!rendered.contains("at-1"), "the access token must not render");
    assert!(!rendered.contains("rt-1"), "the refresh token must not render");
    assert_eq!(format!("{}", token.access_token()), "***");
}

#[test]
fn ac_009_the_module_performs_no_io_of_its_own() {
    let source = include_str!("../../src/google_oauth.rs");

    for needle in ["std::fs", "std::env", "File::open", "dotenv", "std::net"] {
        assert!(
            !source.contains(needle),
            "the module must not touch {needle} (CON-108)"
        );
    }

    // Persistence is the caller's: the crate ships a trait and one in-memory type.
    let store: Box<dyn TokenStore> = Box::new(MemoryTokenStore::new());
    assert!(store.load("absent").is_none());
}
}
