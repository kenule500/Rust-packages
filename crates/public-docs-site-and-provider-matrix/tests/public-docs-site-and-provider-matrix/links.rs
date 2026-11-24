//! AC-003: every internal link resolves, and the check fails on a broken one.
//! EC-004: a relative link that leaves `docs/src` is reported as broken rather
//! than resolved silently.

mod support;

use support::{assert_node_ok, node, Mutation};

#[test]
fn ac_003_broken_internal_links_fail_the_check() {
    // The clean tree passes, so the failures below are caused by the injected link
    // and not by a pre-existing one.
    assert_node_ok("scripts/docs-check.js", &[]);

    // Each case appends one link to a page, runs the check, and asserts the check
    // fails *and* names the offending target. `Mutation` restores the page on drop.
    let cases = [
        (
            "a link to a page that does not exist",
            "[broken](missing-page.md)",
            "missing-page.md",
        ),
        (
            "an external link over plain HTTP",
            "[plain](http://example.com)",
            "http://example.com",
        ),
        (
            "a link that escapes docs/src",
            "[outside](../design-partners/program.md)",
            "../design-partners/program.md",
        ),
    ];

    for (label, injected, expected) in cases {
        let mutation = Mutation::append("docs/src/introduction.md", &format!("\n{injected}\n"));

        let output = node("scripts/docs-check.js", &[]);
        let stderr = String::from_utf8_lossy(&output.stderr);

        assert!(
            !output.status.success(),
            "docs-check must fail on {label}, injected {injected:?}"
        );
        assert!(
            stderr.contains(expected),
            "docs-check must name {expected:?} when reporting {label}, got:\n{stderr}"
        );
        assert!(
            stderr.contains("introduction.md"),
            "docs-check must name the page carrying the bad link, got:\n{stderr}"
        );

        drop(mutation);
    }

    // Nothing was left behind: the tree checks clean again.
    assert_node_ok("scripts/docs-check.js", &[]);
}
