//! Acceptance tests for the `m5-gated-integrations` feature.
//!
//! Covers the half that needs no network: the Angi parser, importer, dedupe key
//! and webhook verifier, plus secret redaction. The LSA and Intacct mock-server
//! tests land separately.

use wirekit::angi::{import_csv, parse_email, AngiWebhook, ColumnMap, ImportOutcome};
use wirekit::Secret;

fn angi_columns() -> ColumnMap {
    ColumnMap {
        lead_id: Some(String::from("Lead ID")),
        contact_name: Some(String::from("Name")),
        phone: Some(String::from("Phone")),
        email: Some(String::from("Email")),
        address: Some(String::from("Address")),
        city: Some(String::from("City")),
        region: Some(String::from("State")),
        postal_code: Some(String::from("Zip")),
        service: Some(String::from("Service")),
        received_at: Some(String::from("Received")),
    }
}

#[test]
fn ac_001_parses_a_documented_angi_email_into_a_lead() {
    let body = "\
Lead ID: A-778
Name: Dana Reed
Phone: 555-0142
Email: dana@example.com
Address: 12 Oak Street
City: Springfield
State: IL
Zip: 62704
Service: Roofing
Received: 2026-10-01
";

    let lead = parse_email(body).expect("a documented template parses");

    assert_eq!(lead.source, "angi-email");
    assert_eq!(lead.lead_id.as_deref(), Some("A-778"));
    assert_eq!(lead.contact_name.as_deref(), Some("Dana Reed"));
    assert_eq!(lead.phone.as_deref(), Some("555-0142"));
    assert_eq!(lead.email.as_deref(), Some("dana@example.com"));
    assert_eq!(lead.service.as_deref(), Some("Roofing"));
    assert_eq!(lead.received_at.as_deref(), Some("2026-10-01"));

    let address = lead.address.expect("the address block is present");
    assert_eq!(address.line1, "12 Oak Street");
    assert_eq!(address.postal_code, "62704");
}

#[test]
fn ac_002_an_unknown_template_fails_naming_a_field() {
    let error = parse_email("This is not an Angi template at all.").expect_err("unknown template fails");

    assert_eq!(error.field, "lead id");
    assert!(error.reason.contains("documented Angi template"));

    // The conversion into the shared error type keeps the field name (FR-026).
    let converted: wirekit::WirekitError = error.into();
    assert_eq!(converted.code(), "W2006");
    assert!(converted.to_string().contains("lead id"));
}

#[test]
fn ac_003_csv_import_reports_one_outcome_per_row() {
    let export = "\
Lead ID,Name,Phone,Email,Address,City,State,Zip,Service,Received
A-1,Ann,555-0100,ann@example.com,1 Main,Springfield,IL,62701,Plumbing,2026-10-01
A-1,Ann,555-0100,ann@example.com,1 Main,Springfield,IL,62701,Plumbing,2026-10-01
,,,,,,,,,
,,,bo@example.com,,,,,,2026-10-02
";

    let report = import_csv(export.as_bytes(), &angi_columns()).expect("the export imports");

    assert_eq!(report.imported(), 2, "two distinct leads are imported");
    assert_eq!(report.skipped(), 1, "the repeated row is skipped");
    assert_eq!(report.failed(), 1, "the row with no identifier fails");
    assert_eq!(report.outcomes.len(), 4, "one outcome per data row");

    let lines: Vec<usize> = report.outcomes.iter().map(|entry| entry.line).collect();
    assert_eq!(lines, vec![2, 3, 4, 5], "lines are 1-based and consecutive");

    assert_eq!(report.outcomes[1].outcome, ImportOutcome::Skipped);
    assert!(report.outcomes[1]
        .reason
        .as_deref()
        .unwrap_or_default()
        .contains("duplicate of line 2"));
    assert_eq!(report.outcomes[2].outcome, ImportOutcome::Failed);
    assert_eq!(report.outcomes[3].outcome, ImportOutcome::Imported);
}

#[test]
fn ac_003_a_missing_mapped_column_fails_before_any_row() {
    let export = "Lead ID,Name\nA-1,Ann\n";
    let error = import_csv(export.as_bytes(), &angi_columns()).expect_err("a missing column fails");

    assert_eq!(error.code(), "W2006");
    assert!(error.to_string().contains("Phone"), "names the missing column");
}

#[test]
fn ac_004_the_dedupe_key_survives_formatting_differences() {
    let variant_a = "\
Lead ID:
Phone: (555) 010-0100
Address: 1 Main St.
City: Springfield
Received: 2026-10-01
";
    let variant_b = "\
Phone: 555.010.0100
Address: 1   Main   St
City: springfield
Received: 2026-10-01
";

    let a = parse_email(variant_a).expect("variant a parses");
    let b = parse_email(variant_b).expect("variant b parses");

    assert_eq!(
        a.dedupe_key(),
        b.dedupe_key(),
        "spacing, punctuation and case must not change the key"
    );
    assert_eq!(a.dedupe_key(), "phy:5550100100|1 main st springfield|2026 10 01");
}

#[test]
fn ac_009_an_unprovisioned_feed_refuses_to_verify() {
    let webhook = AngiWebhook::unprovisioned();

    assert!(!webhook.is_provisioned());
    let error = webhook
        .verify(b"payload", "deadbeef")
        .expect_err("nothing can be verified without a feed");
    assert_eq!(error.code(), "W2006");
    assert!(error.to_string().contains("partner feed is provisioned"));
}

#[test]
fn ac_009_a_wrong_signature_is_rejected() {
    let webhook = AngiWebhook::provisioned(Secret::new(String::from("shared-secret")));

    assert!(webhook.is_provisioned());
    let error = webhook
        .verify(b"payload", "0000")
        .expect_err("a wrong signature must not verify");
    assert_eq!(error.code(), "W2002");
}

#[test]
fn ac_012_the_intacct_sender_password_never_renders() {
    let config = wirekit::sage_intacct::SageIntacctConfig {
        sender_id: String::from("sender-1"),
        sender_password: Secret::new(String::from("hunter2-do-not-print")),
        company_id: String::from("acme"),
        user_id: String::from("svc-user"),
    };

    let rendered = format!("{config:?}");
    assert!(!rendered.contains("hunter2"), "the password must not appear");
    assert!(rendered.contains("***"), "the password renders redacted");
    assert!(rendered.contains("sender-1"), "the sender id is still visible");
}
