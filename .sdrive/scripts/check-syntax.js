#!/usr/bin/env node

// Syntax-only lint with no dependencies and no child processes.
//
// `node --check` in a subprocess is the obvious implementation, but it cannot
// run where child-process stdio is restricted. Compiling with vm.Script checks
// the same grammar in-process.
const fs = require("fs");
const path = require("path");
const vm = require("vm");

const ROOT = path.resolve(__dirname, "..");
const IGNORED = new Set([".git", "node_modules", ".npm-cache", "coverage", ".sdrive"]);
const EXTENSIONS = new Set([".js", ".cjs", ".mjs"]);

function walk(dir, files = []) {
  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    if (IGNORED.has(entry.name)) continue;

    const full = path.join(dir, entry.name);

    if (entry.isDirectory()) walk(full, files);
    else if (EXTENSIONS.has(path.extname(entry.name))) files.push(full);
  }

  return files;
}

const files = walk(ROOT);
const failures = [];

for (const file of files) {
  let source = fs.readFileSync(file, "utf8");

  // Strip a shebang; vm.Script does not accept one.
  if (source.startsWith("#!")) source = source.replace(/^#![^\n]*\n/, "");

  try {
    new vm.Script(source, { filename: file });
  } catch (error) {
    failures.push({ file: path.relative(ROOT, file), message: error.message });
  }
}

if (failures.length > 0) {
  failures.forEach((failure) => {
    console.error(`\n❌ ${failure.file}`);
    console.error(failure.message);
  });
  console.error(`\n${failures.length} file(s) failed syntax checks.`);
  process.exit(1);
}

console.log(`✅ ${files.length} JavaScript file(s) parsed cleanly.`);
