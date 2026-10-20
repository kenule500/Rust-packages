#!/usr/bin/env node

// Thin CLI over the shared validator. config.json used to be validated by its own Ajv
// setup here, so "is this JSON valid?" had two implementations that could drift. Only the
// entry-point checks stay local, because this command is deliberately stricter than
// project-wide validation: it refuses a missing config rather than skipping it.

const fs = require("fs");
const path = require("path");

const { validateArtifacts } = require("./validate-governance");

function main() {
  const projectRoot = process.cwd();
  const configPath = path.join(projectRoot, ".sdrive", "config.json");
  const schemaPath = path.join(projectRoot, ".sdrive", "schemas", "config.schema.json");

  if (!fs.existsSync(configPath)) {
    console.error("❌ config.json not found. Run `sdrive init` first.");
    process.exit(1);
  }

  if (!fs.existsSync(schemaPath)) {
    console.error("❌ config.schema.json not found.");
    process.exit(1);
  }

  const { ok } = validateArtifacts(projectRoot, {
    only: ["config"],
    log: (line) => console.log(line)
  });

  process.exit(ok ? 0 : 1);
}

if (require.main === module) {
  main();
}

module.exports = { main };