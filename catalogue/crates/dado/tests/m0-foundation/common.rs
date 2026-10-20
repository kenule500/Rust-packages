//! Shared file and assertion helpers for the m0-foundation acceptance tests.

use std::path::{Path, PathBuf};

/// Repository root, three levels above the `dado` crate directory.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("dado is nested three levels below the repository root")
        .to_path_buf()
}

/// Resolve a path relative to the repository root.
pub fn repo_path(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

/// Read a UTF-8 file relative to the repository root.
pub fn read_repo(relative: &str) -> String {
    let path = repo_path(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

/// Resolve a path relative to the `dado` crate directory.
pub fn crate_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join(relative)
}

/// Read a UTF-8 file relative to the `dado` crate directory.
pub fn read_crate(relative: &str) -> String {
    let path = crate_path(relative);
    std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("could not read {}: {error}", path.display()))
}

/// Assert that a document contains a required phrase.
pub fn assert_contains(document: &str, needle: &str, label: &str) {
    assert!(document.contains(needle), "{label} must contain {needle:?}");
}

/// Assert that every phrase in a list appears in a document.
pub fn assert_contains_all(document: &str, needles: &[&str], label: &str) {
    for needle in needles {
        assert_contains(document, needle, label);
    }
}

/// Assert that a document does not contain a forbidden phrase.
pub fn assert_not_contains(document: &str, needle: &str, label: &str) {
    assert!(
        !document.contains(needle),
        "{label} must not contain {needle:?}"
    );
}
