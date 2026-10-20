#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();

const featureName = process.argv[2];

if (!featureName) {
  console.error("❌ Usage: node version-new.js <feature>");
  process.exit(1);
}

const sdriveRoot = path.join(projectRoot, ".sdrive");
const workflowStatePath = path.join(sdriveRoot, "workflow-state.json");

// A feature may be in flight (specs/ongoing/) or already released (specs/completed/).
// Adding a version to a RELEASED feature must not require un-archiving it: that would
// rewrite the record of a version that already shipped. So old versions are read where
// they live, and the new one is always written under specs/ongoing/ - which is valid
// because findSpecLocation() resolves specs/<stage>/<feature>/<version> per version.
const specFeatureDir = findFeatureDir(path.join(sdriveRoot, "specs", "ongoing"), featureName);
const releasedSpecDir = findFeatureDir(path.join(sdriveRoot, "specs", "completed"), featureName);
const govFeatureDir = findFeatureDir(path.join(sdriveRoot, "governance"), featureName);

if (!specFeatureDir && !releasedSpecDir && !govFeatureDir) {
  console.error(
    `❌ Feature "${featureName}" not found in specs/ongoing, specs/completed or governance.`
  );
  process.exit(1);
}

// Where existing versions' files live (legacy migration reads these), and where the new
// version goes. The released folder is read, never written.
const specSourceDir = specFeatureDir || releasedSpecDir;
const specTargetDir =
  specFeatureDir || path.join(sdriveRoot, "specs", "ongoing", featureName);

function findFeatureDir(baseDir, name) {
  if (!fs.existsSync(baseDir)) return null;

  const candidate = path.join(baseDir, name);
  if (fs.existsSync(candidate)) return candidate;

  return null;
}

function loadWorkflowState() {
  if (!fs.existsSync(workflowStatePath)) {
    console.error("❌ workflow-state.json not found.");
    process.exit(1);
  }
  return JSON.parse(fs.readFileSync(workflowStatePath, "utf8"));
}

function saveWorkflowState(state) {
  fs.writeFileSync(workflowStatePath, JSON.stringify(state, null, 2));
}

function computeNextVersion(feature) {
  if (!feature.versions || feature.versions.length === 0) {
    return "v1";
  }

  const maxVersion = feature.versions
    .map((v) => parseInt(v.id.replace("v", ""), 10))
    .reduce((a, b) => Math.max(a, b), 0);

  return `v${maxVersion + 1}`;
}

function migrateToVersions(feature, specDir, govDir) {
  console.log("🔧 Migrating feature to version folders...");

  const v1SpecDir = path.join(specDir, "v1");
  const v1GovDir = path.join(govDir, "v1");

  fs.mkdirSync(v1SpecDir, { recursive: true });
  fs.mkdirSync(v1GovDir, { recursive: true });

  const specFiles = ["spec.md", "plan.md", "tasks.md"];
  const govFiles = ["spec.json", "plan.json", "tasks.json", "traceability.json"];

  specFiles.forEach((file) => {
    const src = path.join(specDir, file);
    const dest = path.join(v1SpecDir, file);
    if (fs.existsSync(src)) {
      fs.renameSync(src, dest);
      console.log(`   Moved: ${file} → v1/${file}`);
    }
  });

  govFiles.forEach((file) => {
    const src = path.join(govDir, file);
    const dest = path.join(v1GovDir, file);
    if (fs.existsSync(src)) {
      fs.renameSync(src, dest);
      console.log(`   Moved: ${file} → v1/${file}`);
    }
  });

  return {
    id: "v1",
    phase: feature.phase || "ongoing",
    currentStep: feature.currentStep || "specification",
    gates: feature.gates || {
      specification: "pending",
      planning: "pending",
      implementation: "pending",
      review: "pending"
    },
    history: feature.history || []
  };
}

function createVersionSkeleton(version, specDir, govDir) {
  const versionSpecDir = path.join(specDir, version);
  const versionGovDir = path.join(govDir, version);

  fs.mkdirSync(versionSpecDir, { recursive: true });
  fs.mkdirSync(versionGovDir, { recursive: true });

  fs.writeFileSync(
    path.join(versionSpecDir, "spec.md"),
    `---\ntitle: ${featureName} (${version})\nstatus: backlog\ncreated: ${new Date().toISOString()}\nupdated: ${new Date().toISOString()}\n---\n\n# ${featureName} (${version})\n`
  );

  fs.writeFileSync(
    path.join(versionSpecDir, "plan.md"),
    `# Technical Plan: ${featureName} (${version})\n`
  );

  fs.writeFileSync(
    path.join(versionSpecDir, "tasks.md"),
    `# Execution Tasks: ${featureName} (${version})\n`
  );

  fs.writeFileSync(
    path.join(versionGovDir, "spec.json"),
    JSON.stringify({ feature: featureName, title: featureName, version: 1, overview: {}, userStories: [], functionalRequirements: [] }, null, 2)
  );

  fs.writeFileSync(
    path.join(versionGovDir, "plan.json"),
    JSON.stringify({ feature: featureName, version: 1, architecture: {}, dataModel: {} }, null, 2)
  );

  fs.writeFileSync(
    path.join(versionGovDir, "tasks.json"),
    JSON.stringify({ feature: featureName, version: 1, phases: [] }, null, 2)
  );

  fs.writeFileSync(
    path.join(versionGovDir, "traceability.json"),
    JSON.stringify({ feature: featureName, version: 1, rows: [] }, null, 2)
  );
}

function main() {
  const state = loadWorkflowState();
  const feature = state.features?.find((f) => f.name === featureName);

  if (!feature) {
    console.error(`❌ Feature "${featureName}" not found in workflow-state.json.`);
    process.exit(1);
  }

  // Migrate if needed
  if (!feature.versions || feature.versions.length === 0) {
    const v1 = migrateToVersions(feature, specSourceDir, govFeatureDir);
    feature.versions = [v1];
    feature.currentVersion = "v1";

    // Remove old top-level fields
    delete feature.phase;
    delete feature.currentStep;
    delete feature.gates;
    delete feature.history;
  }

  const nextVersionId = computeNextVersion(feature);

  console.log(`📦 Creating version ${nextVersionId} for "${featureName}"`);

  createVersionSkeleton(nextVersionId, specTargetDir, govFeatureDir);

  feature.versions.push({
    id: nextVersionId,
    phase: "ongoing",
    currentStep: "specification",
    gates: {
      specification: "pending",
      planning: "pending",
      implementation: "pending",
      review: "pending"
    },
    history: [
      {
        event: "version_created",
        timestamp: new Date().toISOString()
      }
    ]
  });

  feature.currentVersion = nextVersionId;

  saveWorkflowState(state);

  console.log(`✅ Version ${nextVersionId} created.`);
  console.log(`   Spec folder: .sdrive/specs/ongoing/${featureName}/${nextVersionId}/`);
  console.log(`   Governance: .sdrive/governance/${featureName}/${nextVersionId}/`);

  if (releasedSpecDir) {
    console.log(
      `   ${featureName} stays released: specs/completed/${featureName}/ and its record are untouched.`
    );
  }

  console.log(`   Next: run /sdrive:propose to fill it.`);
}

main();