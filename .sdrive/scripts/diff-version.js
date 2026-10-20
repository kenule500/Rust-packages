#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();
const sdriveRoot = path.join(projectRoot, ".sdrive");

const featureName = process.argv[2];
const versionA = process.argv[3];
const versionB = process.argv[4];

if (!featureName || !versionA || !versionB) {
  console.error("❌ Usage: node diff-version.js <feature> <v1> <v2>");
  process.exit(1);
}

function findFile(baseDir, version, fileName) {
  const candidate = path.join(baseDir, featureName, version, fileName);

  if (fs.existsSync(candidate)) {
    return candidate;
  }

  return null;
}

function readLines(filePath) {
  return fs.readFileSync(filePath, "utf8").split("\n");
}

function simpleDiff(oldLines, newLines) {
  const added = [];
  const removed = [];
  const unchanged = [];

  const oldSet = new Set(oldLines);
  const newSet = new Set(newLines);

  newLines.forEach((line) => {
    if (oldSet.has(line)) {
      unchanged.push(line);
    } else {
      added.push(line);
    }
  });

  oldLines.forEach((line) => {
    if (!newSet.has(line)) {
      removed.push(line);
    }
  });

  return { added, removed, unchanged };
}

function diffFile(fileName, label) {
  const specA = findFile(path.join(sdriveRoot, "specs", "ongoing"), versionA, fileName);
  const specB = findFile(path.join(sdriveRoot, "specs", "ongoing"), versionB, fileName);

  if (!specA || !specB) {
    console.log(`\n⚠️  ${label}: not both versions have this file. Skipping.`);
    return;
  }

  const oldLines = readLines(specA);
  const newLines = readLines(specB);
  const { added, removed } = simpleDiff(oldLines, newLines);

  console.log(`\n📄 ${label} (${versionA} → ${versionB})`);

  if (added.length === 0 && removed.length === 0) {
    console.log("   ✅ No changes.");
    return;
  }

  if (removed.length > 0) {
    console.log("   ❌ Removed:");
    removed.forEach((line) => console.log(`     - ${line}`));
  }

  if (added.length > 0) {
    console.log("   ✅ Added:");
    added.forEach((line) => console.log(`     + ${line}`));
  }
}

function main() {
  console.log(`\n🔍 Diff: ${featureName} (${versionA} → ${versionB})`);

  diffFile("spec.md", "Specification");
  diffFile("plan.md", "Plan");
  diffFile("tasks.md", "Tasks");
}

main();