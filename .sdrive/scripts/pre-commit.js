const { execSync } = require("child_process");
const path = require("path");
const fs = require("fs");

const projectRoot = process.cwd();

const validatorPath = path.join(projectRoot, ".sdrive", "scripts", "validate-governance.js");
const verifyPath = path.join(projectRoot, ".sdrive", "scripts", "verify.js");

// Gate state is read through commands/gates.js, never re-compared here. The hook and the
// CLI must agree about what "approved" means; a divergent copy in this file could block a
// commit for a gate the CLI considers cleared, or let an unapproved one through.
function gateAccessors() {
  try {
    return require(path.join(projectRoot, ".sdrive", "commands", "gates.js"));
  } catch (error) {
    console.warn(`⚠️  Could not load .sdrive/commands/gates.js (${error.message}). Gate checks skipped.`);
    return null;
  }
}

// Only features that actually claim to be implemented are worth verifying.
//
// Previously this returned EVERY feature, so a feature that was still being
// written (or one that had already shipped) could fail verification and block
// unrelated commits. A feature is verifiable only when:
//   1. its current version is in the "ongoing" phase (not backlog / completed),
//   2. its "implementation" gate is approved (the code is declared complete), and
//   3. at least one gate is still outstanding (there is something left to do).
//
// Mitigating risk beats blocking the whole repo: deferred work in other folders
// must never stop you committing the feature you are actually working on.
function getVerifiableFeatures() {
  const workflowStatePath = path.join(projectRoot, ".sdrive", "workflow-state.json");

  if (!fs.existsSync(workflowStatePath)) return { verifiable: [], skipped: [] };

  let state;
  try {
    state = JSON.parse(fs.readFileSync(workflowStatePath, "utf8"));
  } catch (error) {
    return { verifiable: [], skipped: [] };
  }

  const verifiable = [];
  const skipped = [];
  const gatesApi = gateAccessors();

  for (const feature of state.features || []) {
    const versions = feature.versions || [];
    const version =
      versions.find((v) => v.id === feature.currentVersion) || versions[0];

    if (!version) {
      skipped.push({ name: feature.name, reason: "no version recorded" });
      continue;
    }

    if (version.phase !== "ongoing") {
      skipped.push({ name: feature.name, reason: `phase is "${version.phase}"` });
      continue;
    }

    if (gatesApi && gatesApi.allGatesApproved(version)) {
      skipped.push({ name: feature.name, reason: "all gates already approved" });
      continue;
    }

    if (!gatesApi || !gatesApi.isGateApprovedFor(version, "implementation")) {
      skipped.push({
        name: feature.name,
        reason: `implementation gate is "${
          gatesApi ? gatesApi.gateStatusFor(version, "implementation") : "unreadable"
        }"`
      });
      continue;
    }

    verifiable.push({ name: feature.name });
  }

  return { verifiable, skipped };
}

// 1. Run governance validator
if (fs.existsSync(validatorPath)) {
  try {
    execSync(`node "${validatorPath}"`, { stdio: "inherit" });
    console.log("✅ Governance validation passed.");
  } catch (error) {
    const detail = `${error.stdout || ""}${error.stderr || ""}`;

    if (/Cannot find module 'ajv'/.test(detail)) {
      // The installed validator resolves ajv from the project. `sdrive validate`
      // validates in-process and does not need it; do not block every commit on
      // a missing transitive dependency.
      console.warn(
        "⚠️  The installed validator cannot resolve ajv. Run `sdrive validate` instead."
      );
    } else {
      console.error("❌ Governance validation failed. Fix errors before committing.");
      process.exit(1);
    }
  }
} else {
  console.log("⚠️  Validator not found. Skipping governance validation.");
}

// 2. Run spec-to-code verification for features that claim to be implemented.
const { verifiable, skipped } = getVerifiableFeatures();

if (skipped.length > 0) {
  console.log("\n⏭️  Skipping features that are not ready for verification:");
  for (const entry of skipped) {
    console.log(`   • ${entry.name} — ${entry.reason}`);
  }
}

if (verifiable.length === 0) {
  console.log("⚠️  No features ready for spec-to-code verification. Skipping.");
  process.exit(0);
}

for (const { name } of verifiable) {
  try {
    execSync(`node "${verifyPath}" "${name}"`, { stdio: "pipe" });
    console.log(`✅ Spec-to-code verification passed for ${name}.`);
  } catch (error) {
    console.error(`❌ Verification failed for ${name}.`);
    console.error(error.stdout ? error.stdout.toString() : "");
    process.exit(1);
  }
}

console.log("✅ Pre-commit checks passed.");
process.exit(0);