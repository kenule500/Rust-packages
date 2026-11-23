//! AC-001: `mdbook build docs` produces a site with a landing page, a crate
//! index, a provider matrix, an error-code reference, and an upgrade guide.
//! AC-006: the site needs no JavaScript bundle, no Node build step for the pages
//! themselves, and no external service.

mod support;

use std::fs;
use std::process::Command;

use support::{mdbook, path_of, read, repo_root};

/// The five pages AC-001 names, by chapter stem.
const PAGES: [&str; 5] = [
    "introduction",
    "crates",
    "provider-matrix",
    "errors",
    "upgrading",
];

#[test]
fn ac_001_book_builds_and_exposes_the_five_pages() {
    // `create-missing = false` is what makes the next assertion meaningful: mdBook
    // fails the build instead of inventing an empty chapter, so "every chapter file
    // exists" and "the build succeeds" cannot disagree.
    let book = read("docs/book.toml");
    assert!(
        book.contains("create-missing = false"),
        "docs/book.toml must set create-missing = false so a missing chapter fails the build"
    );
    assert!(
        book.contains("src = \"src\""),
        "docs/book.toml must declare docs/src as the book source"
    );

    let summary = read("docs/src/SUMMARY.md");
    for page in PAGES {
        assert!(
            summary.contains(&format!("({page}.md)")),
            "docs/src/SUMMARY.md must list {page}.md"
        );
        assert!(
            path_of(&format!("docs/src/{page}.md")).is_file(),
            "docs/src/{page}.md is listed in SUMMARY.md but does not exist"
        );
    }

    // AC-001 asks for a built site. Build it when mdBook is installed and assert the
    // rendered pages exist; the CI `book` job runs the same command on every pull
    // request, so the site is built there even when this runner has no mdBook.
    match mdbook() {
        Some(binary) => {
            let output = Command::new(binary)
                .arg("build")
                .arg("docs")
                .current_dir(repo_root())
                .output()
                .expect("failed to spawn `mdbook build docs`");

            assert!(
                output.status.success(),
                "`mdbook build docs` failed with {}\nstderr:\n{}",
                output.status,
                String::from_utf8_lossy(&output.stderr)
            );

            for page in PAGES {
                let rendered = path_of(&format!("docs/book/{page}.html"));
                assert!(
                    rendered.is_file(),
                    "mdbook build docs did not produce {}",
                    rendered.display()
                );
            }
        }
        None => {
            eprintln!(
                "note: mdbook is not on PATH, so this run asserts the structural \
                 equivalent (create-missing = false and every SUMMARY chapter present) \
                 rather than rendering HTML. The CI book job runs `mdbook build docs`."
            );
        }
    }
}

#[test]
fn ac_006_site_needs_no_js_bundle_or_external_service() {
    let book = read("docs/book.toml");

    // No custom theme, no extra assets, no remote service: mdBook's default theme
    // and its own bundled assets only. The keys are compared exactly, because
    // `default-theme` and `preferred-dark-theme` are built-in settings that a
    // substring search would wrongly reject.
    for (index, line) in book.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') {
            continue;
        }
        assert!(
            trimmed != "[output.html.theme]",
            "docs/book.toml line {}: a custom theme directory ships extra assets",
            index + 1
        );

        let Some((key, value)) = trimmed.split_once('=') else {
            continue;
        };
        let forbidden = matches!(key.trim(), "additional-js" | "additional-css" | "theme")
            || (key.trim() == "mathjax-support" && value.trim() == "true");
        assert!(
            !forbidden,
            "docs/book.toml line {} declares {}; the site ships no extra bundle",
            index + 1,
            key.trim()
        );
    }
    assert!(
        !book.contains("http://"),
        "docs/book.toml must not reference a remote asset over plain HTTP"
    );

    // The pages are Markdown. A `<script>` tag or a checked-in .js/.css file would
    // mean a JavaScript build step for the pages themselves.
    let pages = path_of("docs/src");
    let entries = fs::read_dir(&pages).expect("docs/src must exist");
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        assert!(
            !name.ends_with(".js") && !name.ends_with(".css"),
            "docs/src/{name} is a JavaScript or stylesheet asset; the pages must be Markdown"
        );

        if entry.path().extension().is_some_and(|ext| ext == "md") {
            let text = fs::read_to_string(entry.path()).expect("page must be readable");
            assert!(
                !text.contains("<script"),
                "docs/src/{name} embeds a <script> tag; the site must not ship a JS bundle"
            );
        }
    }

    // If the book has been built here, no rendered page may fetch an asset from
    // another origin. Repo links (HTTPS, to the source) are prose, not assets.
    let built = path_of("docs/book");
    if built.is_dir() {
        for entry in fs::read_dir(&built)
            .expect("docs/book must be readable")
            .flatten()
        {
            let path = entry.path();
            if path.extension().is_none_or(|ext| ext != "html") {
                continue;
            }
            let text = fs::read_to_string(&path).expect("rendered page must be readable");
            assert!(
                !text.contains("src=\"http"),
                "{} loads a remote script or asset",
                path.display()
            );
            assert!(
                !text.contains("href=\"http://"),
                "{} links over plain HTTP",
                path.display()
            );
        }
    }
}
