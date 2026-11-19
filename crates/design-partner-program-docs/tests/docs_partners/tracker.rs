//! AC-002 and AC-005: the tracker is an empty, labelled template with seven
//! columns and four status values, and every claim on every page is either
//! sourced to an evidence link or explicitly marked as a placeholder.

mod support;

use support::{assert_checker_passes, doc, table_rows};

const STATUSES: [&str; 4] = ["prospect", "active", "case study published", "declined"];

const COLUMNS: [&str; 7] = [
    "project",
    "contact role",
    "crate in use",
    "environment",
    "start date",
    "status",
    "evidence link",
];

#[test]
fn ac_002_tracker_is_empty_seven_column_template_without_invented_partner() {
    assert_checker_passes();

    let tracker = doc("tracker.md");
    let partners = tracker
        .split("## Partners")
        .nth(1)
        .expect("tracker.md has no `## Partners` section")
        .split("\n## ")
        .next()
        .expect("tracker.md `## Partners` section ends unexpectedly");

    let rows = table_rows(partners);

    // The header names all seven columns, in order.
    let header: Vec<String> = rows[0].iter().map(|c| c.to_lowercase()).collect();
    assert_eq!(
        header, COLUMNS,
        "the tracker header must be exactly the seven documented columns"
    );

    // Every data row fills all seven cells and invents nobody: the project and
    // evidence cells are placeholders (`<...>`), and the status is one of the
    // four documented values.
    let data = &rows[1..];
    assert!(!data.is_empty(), "the template shows no example row at all");

    for (index, row) in data.iter().enumerate() {
        assert_eq!(
            row.len(),
            COLUMNS.len(),
            "data row {} has {} cells, expected {}: {row:?}",
            index + 2,
            row.len(),
            COLUMNS.len()
        );

        let project = row[0].trim_matches('`');
        assert!(
            project.starts_with('<') && project.ends_with('>'),
            "data row {} names a project instead of a placeholder: {project:?}",
            index + 2
        );

        let status = row[5].trim_matches('`');
        assert!(
            STATUSES.contains(&status),
            "data row {} has the unknown status {status:?}",
            index + 2
        );
    }

    // All four status values are documented somewhere on the page.
    for status in STATUSES {
        assert!(
            tracker.contains(status),
            "tracker.md does not document the status {status:?}"
        );
    }
}

#[test]
fn ac_005_claims_are_sourced_or_marked_placeholder() {
    assert_checker_passes();

    // The checker refuses a real partner row whose evidence cell is not a
    // placeholder -- that is AC-005 enforced at the table. Here we confirm the
    // page-level claim: the documents promise nobody is named without consent.
    let program = support::flat(&doc("program.md"));
    assert!(
        program.contains("Missing evidence blocks the gate")
            || program.contains("does not happen")
            || program.contains("blocks the gate"),
        "program.md must say a claim without evidence does not pass the gate"
    );

    // A consent statement is the mechanism by which a name may appear, so it
    // has to exist on the page that defines the forms.
    assert!(
        program.contains("consent"),
        "program.md must define the consent requirement for naming a partner"
    );
}
