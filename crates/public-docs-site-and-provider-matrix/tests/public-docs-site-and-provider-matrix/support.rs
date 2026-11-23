//! Shared helpers for the public-docs-site acceptance tests.
//!
//! These tests deliberately drive the *shipped* scripts — `scripts/docs-matrix.js`,
//! `scripts/docs-errors.js`, and `scripts/docs-check.js` — instead of re-reading the
//! generated Markdown with a second implementation of the same rules. A second copy
//! would agree with the scripts only until one of them was edited, and the review's
//! job is to hold one implementation to the spec rather than let this crate quietly
//! certify a divergent one.

// Each of the five test binaries compiles this module separately, so a helper used
// by one binary is genuinely unused in the others.
#![allow(dead_code)]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

/// The repository root, derived from Cargo's manifest path rather than from the
/// caller's working directory. `cargo test` runs binaries with the *package*
/// directory as cwd, so `scripts/` and `docs/` are two levels up, not `./scripts`.
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("crate lives at <repo>/crates/<crate>")
        .to_path_buf()
}

/// The absolute path of a repository-relative path.
pub fn path_of(relative: &str) -> PathBuf {
    repo_root().join(relative)
}

/// The contents of a repository-relative text file.
pub fn read(relative: &str) -> String {
    let path = path_of(relative);
    fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()))
}

/// Run a repository script with `node`, from the repository root.
///
/// The full `Output` comes back so a failing test can quote the script's own
/// diagnostics instead of paraphrasing them.
pub fn node(script: &str, args: &[&str]) -> Output {
    let root = repo_root();
    assert!(
        root.join(script).is_file(),
        "missing {script} under {}",
        root.display()
    );

    Command::new("node")
        .arg(script)
        .args(args)
        .current_dir(&root)
        .output()
        .unwrap_or_else(|e| panic!("failed to spawn `node {script}`; is Node on PATH? {e}"))
}

/// Assert a repository script succeeds, attaching its output to the panic if not.
pub fn assert_node_ok(script: &str, args: &[&str]) {
    let output = node(script, args);
    assert!(
        output.status.success(),
        "`node {script} {}` failed with {}\nstdout:\n{}\nstderr:\n{}",
        args.join(" "),
        output.status,
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Every `.rs` file under `dir`, recursively.
pub fn rust_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];

    while let Some(current) = pending.pop() {
        let entries = match fs::read_dir(&current) {
            Ok(entries) => entries,
            Err(_) => continue,
        };
        for entry in entries.flatten() {
            let full = entry.path();
            if full.is_dir() {
                pending.push(full);
            } else if full.extension().is_some_and(|ext| ext == "rs") {
                files.push(full);
            }
        }
    }

    files.sort();
    files
}

/// Whether any file in `crates/<crate>/src` mentions `needle`.
///
/// This is how the error-reference test proves a documented code really is
/// declared in the crate it is attributed to, rather than only believing the
/// page the generator wrote.
pub fn crate_source_mentions(crate_name: &str, needle: &str) -> bool {
    let src = path_of(&format!("crates/{crate_name}/src"));
    assert!(src.is_dir(), "crates/{crate_name}/src does not exist");

    rust_files(&src)
        .iter()
        .any(|file| fs::read_to_string(file).is_ok_and(|text| text.contains(needle)))
}

/// `mdbook` on `PATH`, when it is installed.
pub fn mdbook() -> Option<OsString> {
    let path = std::env::var_os("PATH")?;

    for dir in std::env::split_paths(&path) {
        for name in ["mdbook.exe", "mdbook"] {
            let candidate = dir.join(name);
            if candidate.is_file() {
                return Some(candidate.into_os_string());
            }
        }
    }

    None
}

/// A temporary edit to a tracked file, undone when this value is dropped.
///
/// The drift and link checks can only be proven to *fail* by making the tree
/// wrong, so the mutation is restored through `Drop` rather than by a line after
/// the assertion: a panicking assertion must not leave the working tree dirty.
pub struct Mutation {
    path: PathBuf,
    original: Vec<u8>,
}

impl Mutation {
    /// Append `text` to a repository-relative file, keeping the original bytes.
    pub fn append(relative: &str, text: &str) -> Self {
        let path = path_of(relative);
        let original =
            fs::read(&path).unwrap_or_else(|e| panic!("cannot read {}: {e}", path.display()));
        let mut mutated = original.clone();
        mutated.extend_from_slice(text.as_bytes());
        fs::write(&path, &mutated)
            .unwrap_or_else(|e| panic!("cannot write {}: {e}", path.display()));

        Self { path, original }
    }
}

impl Drop for Mutation {
    fn drop(&mut self) {
        let restored = fs::write(&self.path, &self.original);
        assert!(
            restored.is_ok(),
            "could not restore {}: {:?}",
            self.path.display(),
            restored.err()
        );
    }
}

/// The lines of one top-level mapping entry in a YAML file, e.g. one CI job.
///
/// A hand-written reader is enough here: the workflow is a two-level document,
/// and pulling a YAML parser in would add a dependency for one assertion.
pub fn yaml_block(text: &str, key: &str) -> Option<String> {
    let lines: Vec<&str> = text.lines().collect();
    let header = format!("  {key}:");

    let start = lines.iter().position(|line| *line == header)?;

    let end = lines
        .iter()
        .enumerate()
        .skip(start + 1)
        .find_map(|(index, line)| {
            let nested_job =
                line.starts_with("  ") && !line.starts_with("   ") && line.ends_with(':');
            let document_key = !line.starts_with(' ') && !line.is_empty();
            (nested_job || document_key).then_some(index)
        })
        .unwrap_or(lines.len());

    Some(lines[start..end].join("\n"))
}

/// The Markdown table row that mentions `needle`, trimmed.
pub fn table_row<'a>(markdown: &'a str, needle: &str) -> &'a str {
    markdown
        .lines()
        .map(str::trim)
        .find(|line| line.starts_with('|') && line.contains(needle))
        .unwrap_or_else(|| panic!("no table row mentions {needle:?}"))
}
