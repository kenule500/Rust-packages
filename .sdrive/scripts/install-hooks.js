#!/usr/bin/env node

// Installs the SpecDrive pre-commit hook without destroying an existing one.
// The previous version overwrote .git/hooks/pre-commit unconditionally, which
// silently disabled whatever hook the project already had.
const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();
const hooksDir = path.join(projectRoot, ".git", "hooks");
const preCommitPath = path.join(hooksDir, "pre-commit");
const localPath = path.join(hooksDir, "pre-commit.local");
const MARKER = "# SpecDrive pre-commit hook";

const hookContent = `#!/bin/sh
${MARKER}
# Runs governance validation and spec-to-code verification.
# A previous hook, if one existed, is preserved at pre-commit.local and runs after.
if ! node .sdrive/scripts/pre-commit.js; then
  echo "SpecDrive pre-commit checks failed. Commit aborted."
  exit 1
fi

if [ -x "$(dirname "$0")/pre-commit.local" ]; then
  "$(dirname "$0")/pre-commit.local" "$@"
fi
`;

if (!fs.existsSync(hooksDir)) {
  console.error("❌ .git/hooks not found. Are you in a Git repository?");
  process.exit(1);
}

if (fs.existsSync(preCommitPath)) {
  const existing = fs.readFileSync(preCommitPath, "utf8");

  if (!existing.includes(MARKER)) {
    if (fs.existsSync(localPath)) {
      console.error("❌ .git/hooks/pre-commit.local already exists. Refusing to overwrite it.");
      console.error("   Move it aside, then re-run `sdrive hooks:install`.");
      process.exit(1);
    }

    fs.copyFileSync(preCommitPath, localPath);
    fs.chmodSync(localPath, 0o755);
    console.log("✅ Preserved the existing pre-commit hook at .git/hooks/pre-commit.local");
  }
}

fs.writeFileSync(preCommitPath, hookContent, { mode: 0o755 });
console.log("✅ Installed SpecDrive pre-commit hook.");
console.log("   Governance validation will now run before every commit.");
