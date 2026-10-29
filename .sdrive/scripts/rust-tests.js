#!/usr/bin/env node

// Rust test reporter for SpecDrive's `sdrive test` evidence.
//
// `cargo test` prints its summary as `test result: ok. 36 passed; 0 failed;
// 2 ignored; ...`, which SpecDrive's built-in parsers do not recognise. This
// wrapper runs the workspace suite, forwards the real output, and appends one
// Jest-style summary line plus TEST-###/AC-### IDs so the evidence recorder
// records real counts instead of zeros.
const { spawnSync } = require("child_process");

const command = "cargo test --workspace --all-features";
const result = spawnSync(command, { shell: true, encoding: "utf8" });
const output = `${result.stdout || ""}\n${result.stderr || ""}`;

process.stdout.write(result.stdout || "");
process.stderr.write(result.stderr || "");

const counts = { passed: 0, failed: 0, skipped: 0 };

for (const line of output.split(/\r?\n/)) {
  const match = line.match(
    /test result:\s*\w+\.\s*(\d+)\s+passed;\s*(\d+)\s+failed;\s*(\d+)\s+ignored/i
  );

  if (!match) continue;

  counts.passed += Number(match[1]);
  counts.failed += Number(match[2]);
  counts.skipped += Number(match[3]);
}

const total = counts.passed + counts.failed + counts.skipped;

console.log(
  `Tests: ${counts.failed} failed, ${counts.passed} passed, ${counts.skipped} skipped, ${total} total`
);

const ids = new Set();

for (const match of output.matchAll(/ac_(\d{3})/g)) {
  ids.add(match[1]);
}

for (const id of [...ids].sort()) {
  console.log(`TEST-${id}`);
  console.log(`AC-${id}`);
}

process.exit(typeof result.status === "number" ? result.status : 1);
