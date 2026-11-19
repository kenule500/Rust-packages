//! AC-004: a developer looking for adoption information finds it from the
//! repository README, which links to the program page.
//!
//! The test does not merely ask whether the string
//! `docs/design-partners/program.md` occurs anywhere in the README, it asks
//! whether the link is a link a GitHub reader would be able to follow: a
//! Markdown link target relative to the repository root, resolving to a file
//! that exists.
//!
//! It reads the root README directly rather than asking `sdrive verify` which
//! file the plan's `README.md` item "is". The scanner does not index the
//! repository root, so it reports this item as renamed to
//! `crates/dado-yaml/README.md` -- a different file that does not link here. The
//! link, not the scanner's opinion of it, is the evidence.

mod support;

use support::{readme, repo_root};

#[test]
fn ac_004_readme_links_repository_root_to_program_page() {
    let readme = readme();

    // The link must appear as a Markdown link target, not only as prose.
    let target = "[design partner program](docs/design-partners/program.md)";
    assert!(
        readme.contains(target),
        "README.md must link the program page as {target:?}\nREADME tail:\n{}",
        readme.lines().rev().take(12).collect::<Vec<_>>().join("\n")
    );

    // ...and the file it names must resolve from the repository root.
    let program = repo_root().join("docs/design-partners/program.md");
    assert!(
        program.is_file(),
        "README.md links {} but that file does not exist",
        program.display()
    );
}
