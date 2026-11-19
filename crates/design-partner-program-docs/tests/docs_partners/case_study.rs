//! AC-003: the case-study template requires written consent, a redaction
//! checklist, and a measured-outcomes section that takes numbers, not adjectives.

mod support;

use support::{assert_checker_passes, doc, flat};

#[test]
fn ac_003_case_study_has_consent_redaction_and_measured_outcomes() {
    assert_checker_passes();

    let template = flat(&doc("case-study-template.md"));

    for section in [
        "## Consent statement",
        "## Redaction checklist",
        "### Measured outcomes",
    ] {
        assert!(
            template.contains(section),
            "case-study-template.md is missing the section {section:?}"
        );
    }

    // Consent must be written approval, not an assumption.
    assert!(
        template.contains("written approval"),
        "the consent statement must require written approval"
    );

    // The redaction checklist has to cover credentials by name.
    assert!(
        template.contains("credentials"),
        "the redaction checklist must cover credentials"
    );

    // Measured outcomes must state that numbers are required.
    assert!(
        template.contains("Numbers, not adjectives"),
        "the measured-outcomes section must require numbers rather than adjectives"
    );
}
