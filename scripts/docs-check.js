#!/usr/bin/env node
'use strict';

// Fails when the docs site has drifted from the repository.
//
// Run from the repository root:
//
//   node scripts/docs-check.js
//
// Three things are checked, and any one of them fails the job (AC-003, AC-004):
//
//   1. Drift — both generators run in `--check` mode, so a generated page that
//      no longer matches `catalogue/`, `Cargo.toml`, or the crates' source is an
//      error rather than a stale claim (FR-002, FR-007).
//   2. Links — every relative link in `docs/src` must resolve to a file inside
//      `docs/src`, and no link may use `http://` (EC-004, CON-520).
//   3. Reachability — every page must be listed in SUMMARY.md, so nothing is
//      published that a reader cannot reach from the landing page (NFR-004).
//
// It reads files, it does not build or serve anything: `mdbook build docs` runs
// separately, in CI and locally.

const fs = require('fs');
const path = require('path');
const { spawnSync } = require('child_process');

const ROOT = path.resolve(__dirname, '..');
const DOCS_SRC = path.join(ROOT, 'docs', 'src');
const SUMMARY = path.join(DOCS_SRC, 'SUMMARY.md');

const GENERATORS = ['scripts/docs-matrix.js', 'scripts/docs-errors.js'];

const failures = [];

function fail(message) {
  failures.push(message);
}

function relativeToRoot(file) {
  return path.relative(ROOT, file).split(path.sep).join('/');
}

function markdownFiles(dir) {
  const files = [];
  const pending = [dir];

  while (pending.length > 0) {
    const current = pending.pop();
    for (const entry of fs.readdirSync(current, { withFileTypes: true })) {
      const full = path.join(current, entry.name);
      if (entry.isDirectory()) pending.push(full);
      else if (entry.name.endsWith('.md')) files.push(full);
    }
  }

  files.sort();
  return files;
}

// Fenced blocks and inline code are examples, not links: a shell snippet or an
// escaped sample must not be resolved as a page.
function proseOnly(text) {
  return text
    .replace(/^([ \t]*)```[\s\S]*?^[ \t]*```/gm, '')
    .replace(/`[^`\n]*`/g, (span) => span.replace(/[[\]()]/g, ''));
}

function linksIn(text) {
  const links = [];
  for (const match of text.matchAll(/\[[^\]]*\]\(([^)\s]+)(?:\s+"[^"]*")?\)/g)) {
    links.push(match[1]);
  }
  return links;
}

function checkGenerators() {
  for (const generator of GENERATORS) {
    const result = spawnSync(process.execPath, [generator, '--check'], {
      cwd: ROOT,
      encoding: 'utf8',
    });

    for (const stream of [result.stdout, result.stderr]) {
      if (!stream) continue;
      const text = stream.trim();
      if (text) console.log(text);
    }

    if (result.error) {
      fail(`${generator} could not run: ${result.error.message}`);
    } else if (result.status !== 0) {
      fail(`${generator} reports the generated pages are out of date`);
    }
  }
}

function checkLinks(files) {
  for (const file of files) {
    const text = proseOnly(fs.readFileSync(file, 'utf8'));
    const where = relativeToRoot(file);

    for (const target of linksIn(text)) {
      if (target.startsWith('#')) continue;

      if (target.startsWith('http://')) {
        fail(`${where}: ${target} — external links must use HTTPS (CON-520)`);
        continue;
      }

      if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(target) && !target.startsWith('https:')) {
        // `mailto:` is fine; anything else is a scheme the site cannot serve.
        if (!target.startsWith('mailto:')) {
          fail(`${where}: ${target} — unsupported link scheme`);
        }
        continue;
      }

      if (target.startsWith('https:')) continue;

      const withoutAnchor = target.split('#')[0].split('?')[0];
      if (!withoutAnchor) continue;

      let decoded = withoutAnchor;
      try {
        decoded = decodeURIComponent(withoutAnchor);
      } catch {
        fail(`${where}: ${target} — link is not valid percent-encoding`);
        continue;
      }

      const resolved = path.resolve(path.dirname(file), decoded);
      const inside = path.relative(DOCS_SRC, resolved);

      if (inside.startsWith('..') || path.isAbsolute(inside)) {
        fail(`${where}: ${target} — resolves outside docs/src (EC-004)`);
        continue;
      }

      if (!fs.existsSync(resolved) || !fs.statSync(resolved).isFile()) {
        fail(`${where}: ${target} — target does not exist`);
      }
    }
  }
}

function checkReachability(files) {
  if (!fs.existsSync(SUMMARY)) {
    fail('docs/src/SUMMARY.md is missing');
    return;
  }

  const listed = new Set();
  for (const target of linksIn(proseOnly(fs.readFileSync(SUMMARY, 'utf8')))) {
    if (/^[a-zA-Z][a-zA-Z0-9+.-]*:/.test(target)) continue;
    const withoutAnchor = target.split('#')[0].split('?')[0];
    if (!withoutAnchor) continue;
    listed.add(path.resolve(DOCS_SRC, withoutAnchor));
  }

  for (const file of files) {
    if (path.resolve(file) === path.resolve(SUMMARY)) continue;
    if (!listed.has(path.resolve(file))) {
      fail(`${relativeToRoot(file)} is not listed in SUMMARY.md (NFR-004)`);
    }
  }
}

function main() {
  if (!fs.existsSync(DOCS_SRC)) {
    fail('docs/src is missing');
    return 1;
  }

  checkGenerators();

  const files = markdownFiles(DOCS_SRC);
  checkLinks(files);
  checkReachability(files);

  if (failures.length > 0) {
    console.error(`\ndocs-check: ${failures.length} problem(s)`);
    for (const failure of failures) console.error(`  ${failure}`);
    return 1;
  }

  console.log(
    `docs-check: ${files.length} page(s) checked — generated pages current, links resolve, every page is in SUMMARY.md`
  );
  return 0;
}

try {
  process.exitCode = main();
} catch (error) {
  console.error(`docs-check: ${error.message}`);
  process.exitCode = 1;
}
