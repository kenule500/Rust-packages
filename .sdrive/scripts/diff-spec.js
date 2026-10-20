const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();

function readLines(filePath) {
  if (!fs.existsSync(filePath)) {
    return [];
  }
  return fs.readFileSync(filePath, "utf8").split("\n");
}

function simpleDiff(oldFile, newFile) {
  const oldLines = readLines(oldFile);
  const newLines = readLines(newFile);

  const oldSet = new Set(oldLines);
  const newSet = new Set(newLines);

  const added = newLines.filter((line) => !oldSet.has(line));
  const removed = oldLines.filter((line) => !newSet.has(line));

  console.log(`\n📄 Diff between:`);
  console.log(`  Old: ${path.basename(oldFile)}`);
  console.log(`  New: ${path.basename(newFile)}\n`);

  if (added.length === 0 && removed.length === 0) {
    console.log("✅ No changes detected.");
    return;
  }

  if (removed.length > 0) {
    console.log("❌ Removed:");
    removed.forEach((line) => console.log(`  - ${line}`));
  }

  if (added.length > 0) {
    console.log("✅ Added:");
    added.forEach((line) => console.log(`  + ${line}`));
  }
}

const oldFile = process.argv[2];
const newFile = process.argv[3];

if (!oldFile || !newFile) {
  console.error("❌ Usage: node diff-spec.js <old-file> <new-file>");
  process.exit(1);
}

simpleDiff(path.resolve(oldFile), path.resolve(newFile));