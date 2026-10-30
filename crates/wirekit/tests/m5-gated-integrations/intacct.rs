//! Intacct gateway and sandbox-override acceptance tests.
//!
//! These use a local mock, so they need no partner access: the point is the
//! contract (status/errorno parsing, bounds, idempotence key), not the provider.

use wirekit::sage_intacct::{SageIntacctClient, SageIntacctConfig};
use wirekit::{Secret, WirekitClient};

fn config() -> SageIntacctConfig {
    SageIntacctConfig {
        sender_id: String::from("sender-1"),
        sender_password: Secret::new(String::from("hunter2")),
        company_id: String::from("acme"),
        user_id: String::from("svc"),
    }
}

fn client(server: &wiremock::MockServer) -> SageIntacctClient {
    let http = WirekitClient::builder(server.uri())
        .expect("a loopback base URL")
        .allow_insecure_localhost(true)
        .build()
        .expect("the client builds");
    SageIntacctClient::new(config(), http)
}

fn envelope(results: &str) -> String {
    format!(
        "<?xml version=\"1.0\" encoding=\"UTF-8\"?><response><control>\
         <status>success</status></control><operation>{results}</operation></response>"
    )
}

async fn serve(body: String) -> (wiremock::MockServer, SageIntacctClient) {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;
    let client = client(&server);
    (server, client)
}

#[tokio::test]
async fn ac_010_a_create_returns_the_gateway_key() {
    let (_server, client) = serve(envelope(
        "<result><status>success</status><key>42</key>\
         <data><CUSTOMERID>42</CUSTOMERID></data></result>",
    ))
    .await;

    let outcome = client
        .create("CUSTOMER", &[("NAME", "Acme")], None)
        .await
        .expect("the gateway reported success");

    assert!(outcome.is_success());
    assert_eq!(outcome.key.as_deref(), Some("42"));
    assert_eq!(outcome.field("CUSTOMERID"), Some("42"));
}

#[tokio::test]
async fn ac_011_http_200_with_a_failure_status_is_an_error() {
    let (_server, client) = serve(envelope(
        "<result><status>failure</status><errorno>BL01001986</errorno>\
         <description>The customer id is invalid.</description></result>",
    ))
    .await;

    let error = client
        .create("CUSTOMER", &[("NAME", "Acme")], None)
        .await
        .expect_err("a failure status is not success");

    assert_eq!(error.code(), "W2001");
    let rendered = error.to_string();
    assert!(
        rendered.contains("BL01001986"),
        "keeps the errorno: {rendered}"
    );
    assert!(rendered.contains("customer id is invalid"));
}

#[tokio::test]
async fn ac_013_a_response_over_the_ceiling_is_rejected() {
    let (_server, client) = serve(envelope(
        "<result><status>success</status><key>42</key></result>",
    ))
    .await;

    let client = client.max_response_bytes(16);
    let error = client
        .create("CUSTOMER", &[("NAME", "Acme")], None)
        .await
        .expect_err("the body is over the ceiling");

    assert_eq!(error.code(), "W2007");
}

#[tokio::test]
async fn ac_014_the_caller_key_is_sent_as_uniqueid() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200)
                .set_body_string(envelope("<result><status>success</status></result>")),
        )
        .mount(&server)
        .await;

    let intacct = client(&server);
    intacct
        .create("CUSTOMER", &[("NAME", "Acme")], Some("key-7"))
        .await
        .expect("the create succeeds");

    let requests = server
        .received_requests()
        .await
        .expect("the mock records requests");
    let body = String::from_utf8_lossy(&requests[0].body);
    assert!(
        body.contains("<uniqueid>key-7</uniqueid>"),
        "the idempotence key is sent: {body}"
    );
}

#[tokio::test]
async fn ac_015_a_business_error_is_not_retried() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("POST"))
        .respond_with(
            wiremock::ResponseTemplate::new(200).set_body_string(envelope(
                "<result><status>failure</status><errorno>BL01001986</errorno>\
             <description>The customer id is invalid.</description></result>",
            )),
        )
        .mount(&server)
        .await;

    let intacct = client(&server);
    let error = intacct
        .create("CUSTOMER", &[("NAME", "Acme")], None)
        .await
        .expect_err("a business error is a failure");

    assert_eq!(error.code(), "W2001");

    // A business-rule failure will fail identically on every attempt, so the
    // shared policy must not spend three attempts on it (CON-208).
    let requests = server
        .received_requests()
        .await
        .expect("the mock records requests");
    assert_eq!(
        requests.len(),
        1,
        "a business error must not be retried, saw {} requests",
        requests.len()
    );
}

#[tokio::test]
async fn ac_025_a_sandbox_base_url_is_used_verbatim() {
    let (server, client) = serve(envelope(
        "<result><status>success</status><key>1</key></result>",
    ))
    .await;

    client
        .create("CUSTOMER", &[("NAME", "Acme")], None)
        .await
        .expect("the sandbox answered");

    assert_eq!(
        server.received_requests().await.map(|r| r.len()),
        Some(1),
        "the request reached the sandbox host, not the production gateway"
    );
}
