#!/usr/bin/env node

// Fails on mojibake and invalid UTF-8. The dashboard shipped for months with
// garbled middle dots and emoji because a UTF-8 file was re-saved through a
// legacy code page.
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const IGNORED = new Set([".git", "node_modules", ".npm-cache", "coverage"]);
// This document quotes the bad sequences on purpose.
const IGNORED_FILES = new Set(["docs/development-improvements.md"]);
const EXTENSIONS = new Set([".js", ".cjs", ".mjs", ".json", ".md", ".yml", ".yaml", ".html", ".css", ".sh"]);
const MOJIBAKE = [
  "\u00C2\u00B7", // double-encoded middle dot
  "\u00E2\u20AC", // double-encoded em/en dash family
  "\u00F0\u0178", // double-encoded emoji
  "\u00E2\u0153", // double-encoded check mark
  "\u00E2\u017D", // double-encoded branch glyph
  "\u00E2\u2013", 
  "\u00E2\u2014", 
  "\u00C3\u00A9" // double-encoded e-acute
];

function walk(dir, files = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (IGNORED.has(entry.name)) continue;

    const full = path.join(dir, entry.name);

    if (entry.isDirectory()) walk(full, files);
    else if (EXTENSIONS.has(path.extname(entry.name))) files.push(full);
  }

  return files;
}

const failures = [];

for (const file of walk(ROOT)) {
  const relative = path.relative(ROOT, file);

  if (IGNORED_FILES.has(relative.split(path.sep).join("/"))) continue;

  const buffer = fs.readFileSync(file);
  const text = buffer.toString("utf8");

  if (text.includes("\uFFFD")) {
    failures.push({ file: relative, reason: "invalid UTF-8 (replacement character)" });
    continue;
  }

  for (const marker of MOJIBAKE) {
    if (text.includes(marker)) {
      failures.push({ file: relative, reason: `mojibake sequence: ${JSON.stringify(marker)}` });
      break;
    }
  }
}

if (failures.length > 0) {
  failures.forEach((failure) => console.error(`❌ ${failure.file}: ${failure.reason}`));
  console.error(`\n${failures.length} file(s) failed the encoding check.`);
  process.exit(1);
}

console.log("✅ UTF-8 and encoding check passed.");
