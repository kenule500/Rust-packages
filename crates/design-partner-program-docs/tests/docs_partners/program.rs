//! AC-001 and AC-006: the program page states eligibility, what a partner
//! receives, what a partner commits to, how "in production" is judged, and the
//! G5 gate, whose missing evidence blocks 1.0.

mod support;

use support::{assert_checker_passes, doc, flat};

#[test]
fn ac_001_program_states_eligibility_benefits_obligations_and_production() {
    // The checker is the contract for AC-001: if it passes, the sections exist
    // and the 30-day production rule is stated advice.
    assert_checker_passes();

    let program = flat(&doc("program.md"));
    for required in [
        "## Eligibility",
        "## What a partner receives",
        "## What a partner commits to",
        "## Obligations of the project",
        "## What \"in production\" means",
    ] {
        assert!(
            program.contains(required),
            "program.md is missing the section {required:?}"
        );
    }
}

#[test]
fn ac_006_program_names_g5_and_blocks_1_0_without_evidence() {
    assert_checker_passes();

    let program = flat(&doc("program.md"));

    // The gate is named both as an ID and in words: three external projects.
    assert!(program.contains("G5"), "program.md must name the G5 gate");
    assert!(
        program.contains("three external projects"),
        "program.md must state the G5 rule as three external projects"
    );

    // A missing proof has to be described as blocking the release, not as a
    // nicety: any of these phrasings satisfies AC-006.
    let blocks = ["does not happen", "blocks the gate", "blocks 1.0"];
    assert!(
        blocks.iter().any(|p| program.contains(p)),
        "program.md must state that missing evidence blocks 1.0; found none of {blocks:?}"
    );

    // AC-006 also requires G5 and G6 to be distinguished: "three external
    // projects" alone reads as if GIA were included in the count.
    assert!(
        program.contains("G6"),
        "program.md must name G6 so GIA is not counted inside the three external projects"
    );
}
