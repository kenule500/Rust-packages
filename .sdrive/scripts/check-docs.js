#!/usr/bin/env node

// Docs and registry drift check.
//
// The repository already shipped a documented `sdrive migrate` with no command
// and a README table missing several commands. This makes that class of drift a
// CI failure instead of a discovery.
const fs = require("fs");
const path = require("path");

const ROOT = path.resolve(__dirname, "..");
const read = (file) => fs.readFileSync(path.join(ROOT, file), "utf8");
const errors = [];
const warnings = [];

const src = read("src/index.js");
const cliCommands = [
  ...new Set(
    [...src.matchAll(/\.command\("([^"<]+)/g)]
      .map((match) => match[1].trim().split(/\s+/)[0])
      .filter(Boolean)
  )
];

// Hidden/deprecated aliases are intentionally absent from user-facing tables.
const HIDDEN = new Set(["reopen"]);

const documentedIn = (file, pattern) => {
  const text = read(file);
  return [...new Set([...text.matchAll(pattern)].map((match) => match[1]))];
};

const readmeCommands = documentedIn("README.md", /^\|\s*`sdrive ([a-z][a-z0-9:-]*)/gm);
const docsCommands = documentedIn("docs/commands.md", /^###\s*`sdrive ([a-z][a-z0-9:-]*)/gm);

const visible = cliCommands.filter((command) => !HIDDEN.has(command));

for (const command of visible) {
  if (!readmeCommands.includes(command)) {
    errors.push(`README.md terminal table is missing: sdrive ${command}`);
  }
  if (!docsCommands.includes(command)) {
    errors.push(`docs/commands.md is missing a section for: sdrive ${command}`);
  }
}

for (const command of [...readmeCommands, ...docsCommands]) {
  if (!cliCommands.includes(command)) {
    errors.push(`Documentation names a command that does not exist: sdrive ${command}`);
  }
}

// Slash commands: manifest, agent files, and permission rules must agree.
const manifest = JSON.parse(read("commands/manifest.json"));
const permissions = JSON.parse(read("commands/permissions.json"));

for (const command of manifest.commands || []) {
  if (!fs.existsSync(path.join(ROOT, command.agentFile))) {
    errors.push(`Manifest references a missing agent file: ${command.agentFile}`);
  }
  if (!permissions.rules || !Array.isArray(permissions.rules[command.command])) {
    errors.push(`No slash permission rule for: ${command.command}`);
  }
}

const agentFiles = fs
  .readdirSync(path.join(ROOT, "agents"))
  .filter((file) => file.endsWith(".md"));

for (const file of agentFiles) {
  const referenced = (manifest.commands || []).some((command) =>
    command.agentFile.endsWith(file)
  );
  if (!referenced) warnings.push(`Agent file is not referenced by the manifest: agents/${file}`);
}

// CLI permission rules must name real commands, and mutating commands need one.
const { MUTATING } = require("../src/lib/permissions");
const cliRules = permissions.cli || {};

for (const command of MUTATING) {
  if (cliCommands.includes(command) && !Array.isArray(cliRules[command])) {
    errors.push(`No cli permission rule for mutating command: ${command}`);
  }
}

for (const command of Object.keys(cliRules)) {
  if (!cliCommands.includes(command)) {
    warnings.push(`Permission rule names an unknown CLI command: ${command}`);
  }
}

// Adapter registry must still validate.
try {
  const { loadRegistry, validateRegistry } = require("./lib/tool-registry");
  const registry = loadRegistry(path.join(ROOT, ".sdrive"));
  const verdict = validateRegistry(registry.tools);
  verdict.errors.forEach((error) => errors.push(`adapter registry: ${error}`));
  verdict.warnings.forEach((warning) => warnings.push(`adapter registry: ${warning}`));
} catch (error) {
  warnings.push(`Could not check the adapter registry: ${error.message}`);
}

warnings.forEach((warning) => console.warn(`⚠️  ${warning}`));

if (errors.length > 0) {
  errors.forEach((error) => console.error(`❌ ${error}`));
  console.error(`\n${errors.length} documentation/registry error(s).`);
  process.exit(1);
}

console.log(
  `✅ Docs and registry agree: ${visible.length} CLI command(s), ${(manifest.commands || []).length} slash command(s).`
);
