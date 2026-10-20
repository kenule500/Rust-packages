#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const { getWorkflowState, saveWorkflowState, isGateApproved } = require("./gates");

const projectRoot = process.cwd();

function checkPermission(command) {
  const permScript = path.join(projectRoot, ".sdrive", "scripts", "check-permission.js");

  if (!fs.existsSync(permScript)) {
    console.warn("⚠️  Permission checker not found. Skipping permission check.");
    return true;
  }

  try {
    execSync(`node "${permScript}" slash "${command.command}"`, { stdio: "pipe" });
    return true;
  } catch (error) {
    console.error(`\n❌ Permission denied for ${command.command}`);
    if (error.stdout) console.error(error.stdout.toString());
    if (error.stderr) console.error(error.stderr.toString());
    return false;
  }
}

function loadManifest() {
  const manifestPath = path.join(projectRoot, ".sdrive", "commands", "manifest.json");

  if (!fs.existsSync(manifestPath)) {
    console.error("❌ manifest.json not found. Run `sdrive init` first.");
    process.exit(1);
  }

  return JSON.parse(fs.readFileSync(manifestPath, "utf8"));
}

function findCommand(commandName) {
  const manifest = loadManifest();
  return manifest.commands.find((cmd) => cmd.command === commandName);
}

function loadAgent(agentFile) {
  const agentPath = path.join(projectRoot, agentFile);

  if (!fs.existsSync(agentPath)) {
    console.error(`❌ Agent file not found: ${agentFile}`);
    process.exit(1);
  }

  return fs.readFileSync(agentPath, "utf8");
}

function runValidators(validators) {
  const validatorPath = path.join(projectRoot, ".sdrive", "scripts", "validate-governance.js");

  if (!validators || validators.length === 0) {
    return true;
  }

  if (!fs.existsSync(validatorPath)) {
    console.warn("⚠️  Validator script not found. Skipping validation.");
    return true;
  }

  try {
    execSync(`node "${validatorPath}"`, { stdio: "inherit" });
    return true;
  } catch (error) {
    console.error("❌ Validation failed.");
    return false;
  }
}

function toKebabCase(text) {
  return text
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function main() {
  const commandName = process.argv[2];
  const rawArgs = process.argv.slice(3);

  if (!commandName) {
    console.error("❌ Usage: node router.js /sdrive:<command> [feature-description] [--all]");
    process.exit(1);
  }

  const command = findCommand(commandName);

  if (!command) {
    console.error(`❌ Unknown command: ${commandName}`);
    process.exit(1);
  }

  // Detect the --all flag anywhere in the arguments.
  const allFlag = rawArgs.includes("--all");
  const rawFeature = rawArgs.find((arg) => arg !== "--all");

  // Permission check
  if (!checkPermission(command)) {
    process.exit(1);
  }

  if (allFlag && !command.acceptsAll) {
    console.error(`❌ ${command.command} does not support the --all flag.`);
    console.error(`   Run it against a single feature instead.`);
    process.exit(1);
  }

  let featureName = null;

  if (allFlag) {
    console.log(`\n📦 Mode: --all (every ongoing feature)`);
  } else if (rawFeature) {
    featureName = toKebabCase(rawFeature);
    console.log(`\n📦 Feature: ${featureName}`);
  }

  console.log(`\n🚀 SpecDrive Command: ${command.command}`);
  console.log(`📄 Agent: ${command.agent}`);
  console.log(`📁 Agent File: ${command.agentFile}`);
  console.log(`📝 Description: ${command.description}`);

  // A command that accepts a feature must receive one, even when it has no
  // gates. Without this, `/sdrive:propose` (gates: []) silently ran with no
  // feature and the agent had nothing to work on.
  if (command.acceptsFeature && !featureName && !allFlag) {
    console.error(`❌ ${command.command} requires a feature name or description.`);
    console.error(`   Usage: node router.js ${commandName} "<feature-description>"`);
    process.exit(1);
  }

  if (command.gates && command.gates.length > 0) {
    // In --all mode gates are evaluated per feature by the agent, not here.
    if (!featureName && !allFlag) {
      console.error(`❌ This command requires a feature name or description.`);
      console.error(`   Usage: node router.js ${commandName} "<feature-description>"`);
      process.exit(1);
    }

    if (allFlag) {
      console.log(`\n🚧 Required Gates: ${command.gates.join(", ")} (evaluated per feature in --all mode)`);
    } else {
      console.log(`\n🚧 Required Gates: ${command.gates.join(", ")}`);
    }

    // In --all mode there is no single feature to gate-check here; the agent
    // evaluates each feature's gates individually as it processes them.
    if (!allFlag) {
      for (const gate of command.gates) {
        if (!isGateApproved(featureName, gate)) {
          console.log(`\n⏸️  Gate "${gate}" is NOT approved for "${featureName}".`);
          console.log(`   Please approve this gate before running this command.`);
          console.log(`   Use: sdrive approve ${featureName} ${gate}\n`);
          process.exit(1);
        }
        console.log(`✅ Gate "${gate}" is approved.`);
      }
    }
  }

  const agentContent = loadAgent(command.agentFile);
  console.log(`\n✅ Loaded agent instructions (${agentContent.length} characters).`);

  const valid = runValidators(command.validators);

  if (!valid) {
    console.error("\n❌ Command execution blocked due to validation failure.");
    process.exit(1);
  }

  console.log("\n🤖 The AI tool should now read the agent instructions and execute the command.");
  console.log("⏸️  Remember to stop for human approval after the command completes.\n");
}

main();