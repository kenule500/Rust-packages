//! Acceptance tests for `WirekitClient` and its builder (AC-016 to AC-020).

use std::time::Duration;

use serde_json::json;
use wirekit::client::{
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_MAX_RESPONSE_BYTES, DEFAULT_POOL_MAX_IDLE_PER_HOST,
    DEFAULT_TIMEOUT,
};
use wirekit::noaa::NoaaClient;
use wirekit::pipeline::RequestSpec;
use wirekit::slack::{SlackClient, SlackMessage};
use wirekit::WirekitClient;

use crate::common;

/// Produce a `HeaderName`/`HeaderValue` pair without naming the `http` types:
/// they are cloned out of a request specification by the call site, so the
/// concrete types are inferred.
macro_rules! marker_headers {
    ($client:expr) => {{
        let probe = RequestSpec::get($client.url("v1/probe").expect("the probe URL"))
            .try_header("x-wirekit-test", "marker")
            .expect("a valid probe header");
        let name = probe
            .headers()
            .keys()
            .next()
            .expect("the probe header exists")
            .clone();
        let value = probe
            .headers()
            .get("x-wirekit-test")
            .expect("the probe header exists")
            .clone();
        (name, value)
    }};
}

#[tokio::test]
async fn ac_016_two_providers_share_one_transport_and_connection_pool() {
    let server = common::server().await;
    common::mount(
        &server,
        "POST",
        "/chat.postMessage",
        common::Recorder::json(200, json!({ "ok": true, "channel": "C1", "ts": "1.1" })),
    )
    .await;
    common::mount(
        &server,
        "GET",
        "/radar/stations/KTLX",
        common::Recorder::json(
            200,
            json!({ "properties": { "stationIdentifier": "KTLX", "name": "Oklahoma City" } }),
        ),
    )
    .await;

    let client = common::client(&server);
    let slack = SlackClient::builder(client.clone())
        .bot_token("xoxb-test")
        .build()
        .expect("the Slack client builds");
    let noaa = NoaaClient::builder(client.clone())
        .user_agent("wirekit-tests/0.1 (qa@example.com)")
        .build();

    assert!(
        std::ptr::eq(client.http(), slack.client().http()),
        "the Slack client must reuse the shared transport"
    );
    assert!(
        std::ptr::eq(client.http(), noaa.client().http()),
        "the NOAA client must reuse the shared transport"
    );

    slack
        .post_message(&SlackMessage::text("C1", "hello"))
        .await
        .expect("the message is accepted");
    noaa.radar_metadata("KTLX")
        .await
        .expect("the radar metadata is returned");
}

#[tokio::test]
async fn ac_017_an_injected_client_is_used_verbatim() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/slow",
        // The responder must hold the answer far longer than the injected
        // timeout, but the request itself must still reach the recorder.
        // A 10 ms timeout raced the TCP send and could give up before the
        // request arrived, which made `recorder.last()` panic under load.
        common::Recorder::json(200, json!({})).delay(3000),
    )
    .await;

    let scratch = common::client(&server);
    let (name, value) = marker_headers!(&scratch);

    let injected = WirekitClient::builder(server.uri())
        .expect("a loopback base URL")
        .allow_insecure_localhost(true)
        .default_header(name, value)
        .timeout(Duration::from_millis(300))
        .build()
        .expect("the source client builds");

    let client = common::builder(&server)
        .timeout(Duration::from_secs(5))
        .injected_client(injected.http().clone())
        .build()
        .expect("the injecting client builds");

    let error = client
        .get_json::<serde_json::Value>("v1/slow")
        .await
        .expect_err("the injected 300 ms timeout must apply");

    assert_eq!(
        error.code(),
        "W2004",
        "the builder's 5 s timeout must be ignored when a client is injected"
    );
    assert_eq!(
        recorder.last().header("x-wirekit-test"),
        Some("marker"),
        "the injected client's default header reached the provider"
    );
}

#[tokio::test]
async fn ac_018_builder_settings_apply_to_requests() {
    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/config",
        // Same margin as ac_017: the responder must outlast the client
        // timeout without the client aborting before the request is recorded.
        common::Recorder::json(200, json!({ "ok": true })).delay(3000),
    )
    .await;

    let scratch = common::client(&server);
    let (name, value) = marker_headers!(&scratch);

    let client = common::builder(&server)
        .timeout(Duration::from_millis(300))
        .connect_timeout(Duration::from_millis(50))
        .pool_max_idle_per_host(2)
        .default_header(name, value)
        .build()
        .expect("the client builds");

    let error = client
        .get_json::<serde_json::Value>("v1/config")
        .await
        .expect_err("the 300 ms timeout must apply");
    assert_eq!(error.code(), "W2004");
    assert_eq!(
        recorder.last().header("x-wirekit-test"),
        Some("marker"),
        "the default header was applied"
    );

    // A proxy is configured on the builder. reqwest validates the proxy URL
    // shape; the property AC-018 requires is that the setting is accepted and
    // leaves the rest of the builder intact.
    let proxied = common::builder(&server)
        .proxy("http://127.0.0.1:8080")
        .build()
        .expect("a syntactically valid proxy is accepted");
    assert_eq!(proxied.retry_policy().attempts, 1);
}

#[tokio::test]
async fn ac_019_a_plain_http_base_url_is_rejected_before_any_socket() {
    let error = WirekitClient::builder("http://api.example.com")
        .expect("the URL parses")
        .build()
        .expect_err("plain HTTP must be rejected");
    assert_eq!(error.code(), "W2006");
    assert!(
        error.to_string().contains("https"),
        "the message must explain the requirement: {error}"
    );

    let error = WirekitClient::builder("http://api.example.com")
        .expect("the URL parses")
        .allow_insecure_localhost(true)
        .build()
        .expect_err("the loopback exception must not apply to a public host");
    assert_eq!(error.code(), "W2006");

    // A loopback mock is the one permitted exception.
    let server = common::server().await;
    let client = common::client(&server);
    assert_eq!(client.base_url().scheme(), "http");

    let error = client
        .url("http://169.254.169.254/latest/meta-data")
        .expect_err("an absolute plain-HTTP URL must be rejected");
    assert_eq!(error.code(), "W2006");
    assert!(error.to_string().contains("https"));
}

#[tokio::test]
async fn ac_020_the_default_configuration_applies_timeouts_and_pool_limits() {
    assert_eq!(DEFAULT_TIMEOUT, Duration::from_secs(30));
    assert_eq!(DEFAULT_CONNECT_TIMEOUT, Duration::from_secs(10));
    assert_eq!(DEFAULT_POOL_MAX_IDLE_PER_HOST, 8);
    assert_eq!(DEFAULT_MAX_RESPONSE_BYTES, 8 * 1024 * 1024);

    let server = common::server().await;
    let recorder = common::mount(
        &server,
        "GET",
        "/v1/defaults",
        common::Recorder::json(200, json!({ "ok": true })),
    )
    .await;

    let client = WirekitClient::builder(server.uri())
        .expect("a loopback base URL")
        .allow_insecure_localhost(true)
        .build()
        .expect("the default client builds");

    assert_eq!(
        client.retry_policy().attempts,
        3,
        "the default policy allows three attempts"
    );
    assert_eq!(client.base_url().as_str(), format!("{}/", server.uri()));

    for _ in 0..3 {
        client
            .get_json::<serde_json::Value>("v1/defaults")
            .await
            .expect("a default request succeeds");
    }
    assert_eq!(recorder.hits(), 3);
}
