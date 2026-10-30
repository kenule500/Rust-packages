//! Local Services Ads acceptance tests (AC-026 to AC-029).
//!
//! The published `localservices:v1` API is reporting-only, so these exercise the
//! two documented `:search` methods against a mock: no Ads account is needed.

use wirekit::google_lsa::{ApiDate, GoogleLsaClient, SearchPage, CODE_ACCOUNT_NOT_APPROVED};
use wirekit::{Secret, WirekitClient};

fn client(server: &wiremock::MockServer) -> GoogleLsaClient {
    let http = WirekitClient::builder(server.uri())
        .expect("a loopback base URL")
        .allow_insecure_localhost(true)
        .build()
        .expect("the client builds");
    GoogleLsaClient::new(http, Secret::new(String::from("token-1")))
}

fn page() -> SearchPage {
    SearchPage::between(ApiDate::new(2026, 10, 1), ApiDate::new(2026, 10, 31))
}

#[tokio::test]
async fn ac_026_account_reports_uses_the_documented_search_path() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"accountReports":[{"accountId":"111","businessName":"Acme"}],"nextPageToken":"n2"}"#,
        ))
        .mount(&server)
        .await;

    let reports = client(&server)
        .account_reports("123", &page())
        .await
        .expect("the search succeeds");

    assert_eq!(reports.account_reports.len(), 1);
    assert_eq!(
        reports.account_reports[0].account_id.as_deref(),
        Some("111")
    );
    assert_eq!(reports.next_page_token.as_deref(), Some("n2"));

    let requests = server.received_requests().await.expect("the mock records");
    let url = requests[0].url.as_str();
    assert!(
        url.contains("v1/accountReports:search"),
        "the documented path is used: {url}"
    );
    assert!(
        url.contains("manager_customer_id"),
        "the manager customer id is sent: {url}"
    );
}

#[tokio::test]
async fn ac_027_detailed_lead_reports_return_typed_rows() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200).set_body_string(
            r#"{"detailedLeadReports":[{"leadId":"L1","leadType":"MESSAGE","messageLead":{"jobType":"Roofing","customerName":"Dana"}}]}"#,
        ))
        .mount(&server)
        .await;

    let leads = client(&server)
        .detailed_lead_reports("123", &page())
        .await
        .expect("the search succeeds");

    assert_eq!(leads.detailed_lead_reports.len(), 1);
    let lead = &leads.detailed_lead_reports[0];
    assert_eq!(lead.lead_id.as_deref(), Some("L1"));
    assert_eq!(lead.lead_type.as_deref(), Some("MESSAGE"));
    assert_eq!(
        lead.message_lead
            .as_ref()
            .and_then(|message| message.job_type.as_deref()),
        Some("Roofing")
    );

    let requests = server.received_requests().await.expect("the mock records");
    assert!(
        requests[0]
            .url
            .as_str()
            .contains("v1/detailedLeadReports:search"),
        "the documented path is used"
    );
}

#[tokio::test]
async fn ac_028_a_403_is_an_explicit_account_not_approved_error() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(403).set_body_string("{}"))
        .mount(&server)
        .await;

    let error = client(&server)
        .account_reports("123", &page())
        .await
        .expect_err("403 must fail");

    let rendered = error.to_string();
    assert!(
        rendered.contains(CODE_ACCOUNT_NOT_APPROVED),
        "the error names the unapproved account rather than a generic auth failure: {rendered}"
    );
    assert!(rendered.contains("not linked"), "{rendered}");
}

#[tokio::test]
async fn ac_029_a_search_without_a_manager_id_fails_before_any_request() {
    let server = wiremock::MockServer::start().await;
    wiremock::Mock::given(wiremock::matchers::method("GET"))
        .respond_with(wiremock::ResponseTemplate::new(200))
        .mount(&server)
        .await;

    let error = client(&server)
        .account_reports("   ", &page())
        .await
        .expect_err("a manager customer id is required");

    assert_eq!(error.code(), "W2006");
    assert!(server
        .received_requests()
        .await
        .expect("recorded")
        .is_empty());
}
