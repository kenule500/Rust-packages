#!/usr/bin/env node

// Permission checker for slash commands and legacy CLI callers.
//
// The CLI now uses src/lib/permissions.js in-process, but the router and
// pre-commit hook still execute this script from .sdrive/scripts/. Keep the
// policy identical: missing governance fails closed, and mutating commands are
// deny-by-default when no rule exists.
const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();

const MUTATING = new Set([
  "init",
  "update",
  "template:set",
  "create",
  "update-propose",
  "assign",
  "approve",
  "archive:finalize",
  "team:add",
  "team:remove",
  "team:update",
  "team:sync",
  "hooks:install",
  "amend",
  "version:new",
  "adapters:add",
  "test",
  "check",
  "config:set"
]);

const BOOTSTRAP = new Set(["team:add", "adapters:add"]);

function getCurrentUser() {
  if (process.env.SDRIVE_ACTOR) return process.env.SDRIVE_ACTOR.trim();

  try {
    return execSync("git config user.name", { encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }).trim();
  } catch (error) {
    return process.env.USERNAME || process.env.USER || null;
  }
}

function loadTeam() {
  const teamPath = path.join(projectRoot, ".sdrive", "team.json");

  if (!fs.existsSync(teamPath)) {
    console.error("❌ team.json not found. Run `sdrive init` first.");
    process.exit(2);
  }

  return JSON.parse(fs.readFileSync(teamPath, "utf8"));
}

function loadPermissions() {
  const permPath = path.join(projectRoot, ".sdrive", "commands", "permissions.json");

  if (!fs.existsSync(permPath)) {
    console.error("❌ permissions.json not found. Run `sdrive init` or `sdrive update` first.");
    process.exit(2);
  }

  return JSON.parse(fs.readFileSync(permPath, "utf8"));
}

function deny(message) {
  console.error(message);
  process.exit(5);
}

function checkPermission(commandType, commandName) {
  if (process.env.SDRIVE_SKIP_PERMISSIONS === "1" || process.env.SDRIVE_SKIP_PERMISSIONS === "true") {
    console.error(`⚠️  Permissions skipped for ${commandType}:${commandName} (SDRIVE_SKIP_PERMISSIONS=1).`);
    process.exit(0);
  }

  const username = getCurrentUser();

  if (!username) {
    deny("❌ Could not detect Git user. Set git config user.name.");
  }

  const team = loadTeam();
  const members = Array.isArray(team.members) ? team.members : [];

  if (members.length === 0) {
    if (commandType === "cli" && BOOTSTRAP.has(commandName)) {
      console.log(`✅ Bootstrap: team.json has no members. Allowing '${commandName}' to add the first member.`);
      process.exit(0);
    }

    deny(
      "❌ team.json has no members. Add the first admin with `sdrive team:add <your-git-username> admin`."
    );
  }

  const member = members.find((entry) => entry.username === username);

  if (!member) {
    deny(
      `❌ User "${username}" is not in team.json.\n   Fix: run \`sdrive team:add "${username}" admin\` from the project root.\n   Note: the username must match \`git config user.name\` exactly.`
    );
  }

  const permissions = loadPermissions();
  const rules = commandType === "cli" ? permissions.cli : permissions.rules;
  const allowedRoles = rules && rules[commandName];
  const mutating = commandType === "cli" ? MUTATING.has(commandName) : true;

  if (!allowedRoles || !Array.isArray(allowedRoles)) {
    if (!mutating) {
      console.log(`⚠️  No permission rule for ${commandType}:${commandName}. Read-only command allowed.`);
      process.exit(0);
    }

    deny(
      `❌ No permission rule for ${commandType}:${commandName}. Add one to .sdrive/commands/permissions.json.`
    );
  }

  if (!allowedRoles.includes(member.role)) {
    deny(
      `❌ User "${username}" (${member.role}) is not allowed to run ${commandType}:${commandName}.\n   Allowed roles: ${allowedRoles.join(", ")}`
    );
  }

  console.log(`✅ User "${username}" (${member.role}) is allowed.`);
  process.exit(0);
}

const commandType = process.argv[2];
const commandName = process.argv[3];

if (!commandType || !commandName) {
  console.error("❌ Usage: node check-permission.js <cli|slash> <command>");
  process.exit(2);
}

checkPermission(commandType, commandName);
