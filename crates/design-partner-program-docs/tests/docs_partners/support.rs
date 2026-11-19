//! Shared helpers for the design-partner program-docs acceptance tests.
//!
//! These tests deliberately exercise the *shipped* checker,
//! `scripts/docs-partners.js`, instead of re-reading the Markdown with a second
//! implementation of the same rules. A second copy would agree with the checker
//! only until one of them was edited, and the review's job is to hold that one
//! implementation to the spec -- not to let this crate quietly certify a
//! divergent one.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root, derived from Cargo's manifest path rather than from the
/// caller's working directory. `cargo test` runs binaries with the *package*
/// directory as cwd, so `scripts/` is two levels up and not `./scripts`.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives at <repo>/crates/<crate>")
        .to_path_buf()
}

/// Run the docs checker and return its process output.
///
/// The checker exits 0 when every document satisfies its acceptance criteria and
/// 1 after printing the failures. We return the full `Output` so a failing test
/// can quote the checker's own diagnostics instead of paraphrasing them.
pub fn run_checker() -> Output {
    let root = repo_root();
    assert!(
        root.join("scripts/docs-partners.js").is_file(),
        "missing scripts/docs-partners.js under {}",
        root.display()
    );

    Command::new("node")
        .arg("scripts/docs-partners.js")
        .current_dir(&root)
        .output()
        .expect("failed to spawn `node scripts/docs-partners.js`; is Node on PATH?")
}

/// Assert the checker passes, attaching its stderr to the panic if it does not.
pub fn assert_checker_passes() {
    let output = run_checker();
    assert!(
        output.status.success(),
        "docs-partners checker failed with {}\nstderr:\n{}",
        output.status,
        String::from_utf8_lossy(&output.stderr)
    );
}

/// The contents of a document under `docs/design-partners/`.
pub fn doc(name: &str) -> String {
    let path = repo_root().join("docs/design-partners").join(name);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// The repository README.
pub fn readme() -> String {
    let path = repo_root().join("README.md");
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Collapse every run of whitespace to one space so assertions are not hostage
/// to how a paragraph happens to be wrapped at 80 columns.
pub fn flat(text: &str) -> String {
    text.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// The data rows of a Markdown table, parsed the way a table is: the outer pipes
/// are delimiters, `\|` is a literal pipe inside a cell, and the `|---|`
/// alignment row is not data.
pub fn table_rows(table: &str) -> Vec<Vec<String>> {
    let mut rows = Vec::new();

    for line in table.lines() {
        let line = line.trim();
        if !line.starts_with('|') || !line.ends_with('|') {
            continue;
        }
        if line
            .chars()
            .all(|c| c == '|' || c == '-' || c == ':' || c.is_whitespace())
        {
            continue;
        }
        rows.push(split_row(line));
    }

    rows
}

fn split_row(line: &str) -> Vec<String> {
    let inner = line.trim_start_matches('|').trim_end_matches('|');

    let mut cells = Vec::new();
    let mut cell = String::new();
    let mut chars = inner.chars();

    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.clone().next() == Some('|') => {
                cell.push('|');
                chars.next();
            }
            '|' => {
                cells.push(cell.trim().to_string());
                cell.clear();
            }
            _ => cell.push(c),
        }
    }

    cells.push(cell.trim().to_string());
    cells
}
