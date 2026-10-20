#!/usr/bin/env node

// Cross-platform test runner.
//
// Node's `--test` accepts files, but glob expansion differs between shells and
// Node versions (Node 18 does not expand `test/**/*.js`). This walks the test
// directories itself and passes an explicit file list.
const fs = require("fs");
const path = require("path");
const { spawnSync } = require("child_process");

const ROOT = path.resolve(__dirname, "..");
const TEST_ENV = path.join(ROOT, "test", "helpers", "test-env.js");
const target = process.argv[2] || "test";
const mode = process.argv[3] || "all";

function walk(dir, files = []) {
  if (!fs.existsSync(dir)) return files;

  for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
    const full = path.join(dir, entry.name);

    if (entry.isDirectory()) walk(full, files);
    else if (entry.isFile() && entry.name.endsWith(".js")) files.push(full);
  }

  return files;
}

const absolute = path.resolve(ROOT, target);
let files = walk(absolute);

if (mode === "e2e") {
  files = files.filter(
    (file) => path.dirname(file) === path.join(ROOT, "test") && path.basename(file).startsWith("test-")
  );
}

files.sort();

if (files.length === 0) {
  console.error(`No test files found under ${target}.`);
  process.exit(1);
}

const result = spawnSync(
  process.execPath,
  ["--test", "--require", TEST_ENV, ...files],
  { cwd: ROOT, stdio: "inherit", env: process.env }
);

process.exit(typeof result.status === "number" ? result.status : 1);
