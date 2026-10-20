#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();
const sdriveRoot = path.join(projectRoot, ".sdrive");

const openspecDir = path.join(projectRoot, "openspec");
const openspecSpecsDir = path.join(openspecDir, "specs");
const openspecChangesDir = path.join(openspecDir, "changes");
const openspecArchiveDir = path.join(openspecDir, "archive");

if (!fs.existsSync(openspecDir)) {
  console.error("❌ openspec/ folder not found. Nothing to migrate.");
  process.exit(1);
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

// The stored gate value belongs to commands/gates.js. A migration that spelled it out
// here would produce records the CLI stops recognising the day that value gains a shape,
// and it would do so silently - the migrated state would simply look unapproved.
function approvedGateValue() {
  const gatesModulePath = path.join(sdriveRoot, "commands", "gates.js");

  if (!fs.existsSync(gatesModulePath)) {
    console.error("❌ .sdrive/commands/gates.js is missing. Run `sdrive init` before migrating.");
    process.exit(1);
  }

  return require(gatesModulePath).GATE_APPROVED;
}

function readFile(filePath) {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch (error) {
    return null;
  }
}

function listDirectories(dir) {
  if (!fs.existsSync(dir)) return [];
  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name);
}

function slugify(name) {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function findSpecFile(dir) {
  const candidates = ["spec.md", "proposal.md", "README.md"];
  for (const candidate of candidates) {
    const full = path.join(dir, candidate);
    if (fs.existsSync(full)) return full;
  }
  return null;
}

function findTasksFile(dir) {
  const candidates = ["tasks.md", "todo.md"];
  for (const candidate of candidates) {
    const full = path.join(dir, candidate);
    if (fs.existsSync(full)) return full;
  }
  return null;
}

function writeSpecDriveFeature(featureName, sourceDir, lifecycle) {
  const featureFolder = path.join(
    sdriveRoot,
    "specs",
    lifecycle,
    featureName,
    "v1"
  );
  const govFolder = path.join(sdriveRoot, "governance", featureName, "v1");

  ensureDir(featureFolder);
  ensureDir(govFolder);

  const specSource = findSpecFile(sourceDir);
  const tasksSource = findTasksFile(sourceDir);

  const specContent = specSource
    ? readFile(specSource)
    : `# ${featureName}\n\n(Migrated from OpenSpec — no source spec found)`;

  const tasksContent = tasksSource
    ? readFile(tasksSource)
    : `# Execution Tasks: ${featureName}\n\n(Migrated from OpenSpec — no tasks found)`;

  const now = new Date().toISOString();

  // spec.md
  fs.writeFileSync(
    path.join(featureFolder, "spec.md"),
    `---
title: ${featureName}
status: ${lifecycle === "completed" ? "completed" : "ongoing"}
version: v1
created: ${now}
updated: ${now}
retroactive: true
migratedFrom: openspec
---

${specContent}
`
  );

  // plan.md
  fs.writeFileSync(
    path.join(featureFolder, "plan.md"),
    `# Technical Plan: ${featureName} (migrated)\n\nThis plan was migrated from OpenSpec. No technical plan was extracted.\n`
  );

  // tasks.md
  fs.writeFileSync(
    path.join(featureFolder, "tasks.md"),
    tasksContent
  );

  // governance JSON
  fs.writeFileSync(
    path.join(govFolder, "spec.json"),
    JSON.stringify(
      {
        feature: featureName,
        title: featureName,
        version: 1,
        retroactive: true,
        migratedFrom: "openspec",
        overview: {
          problemStatement: "(migrated from OpenSpec)",
          proposedSolution: "(migrated from OpenSpec)",
          targetAudience: "(migrated from OpenSpec)"
        },
        userStories: [],
        functionalRequirements: []
      },
      null,
      2
    )
  );

  fs.writeFileSync(
    path.join(govFolder, "plan.json"),
    JSON.stringify(
      {
        feature: featureName,
        version: 1,
        architecture: { approach: "(migrated)", components: [] },
        dataModel: { entities: [], flowDescription: "(migrated)" }
      },
      null,
      2
    )
  );

  fs.writeFileSync(
    path.join(govFolder, "tasks.json"),
    JSON.stringify(
      { feature: featureName, version: 1, phases: [] },
      null,
      2
    )
  );

  fs.writeFileSync(
    path.join(govFolder, "traceability.json"),
    JSON.stringify(
      { feature: featureName, version: 1, rows: [] },
      null,
      2
    )
  );

  return {
    feature: featureName,
    source: path.relative(projectRoot, sourceDir),
    lifecycle,
    spec: specSource ? path.relative(projectRoot, specSource) : null,
    tasks: tasksSource ? path.relative(projectRoot, tasksSource) : null
  };
}

function migrate() {
  const migrated = [];

  // 1. Active specs
  if (fs.existsSync(openspecSpecsDir)) {
    const features = listDirectories(openspecSpecsDir);
    for (const feature of features) {
      const featureName = slugify(feature);
      const sourceDir = path.join(openspecSpecsDir, feature);
      const result = writeSpecDriveFeature(featureName, sourceDir, "ongoing");
      migrated.push(result);
      console.log(`✅ Migrated spec: ${featureName}`);
    }
  }

  // 2. Active changes
  if (fs.existsSync(openspecChangesDir)) {
    const changes = listDirectories(openspecChangesDir);
    for (const change of changes) {
      const featureName = slugify(change);
      const sourceDir = path.join(openspecChangesDir, change);
      const result = writeSpecDriveFeature(featureName, sourceDir, "ongoing");
      migrated.push(result);
      console.log(`✅ Migrated change: ${featureName}`);
    }
  }

  // 3. Archive
  if (fs.existsSync(openspecArchiveDir)) {
    const archived = listDirectories(openspecArchiveDir);
    for (const feature of archived) {
      const featureName = slugify(feature);
      const sourceDir = path.join(openspecArchiveDir, feature);
      const result = writeSpecDriveFeature(featureName, sourceDir, "completed");
      migrated.push(result);
      console.log(`✅ Migrated archive: ${featureName}`);
    }
  }

  return migrated;
}

function updateWorkflowState(migrated) {
  const statePath = path.join(sdriveRoot, "workflow-state.json");

  let state = { features: [] };
  if (fs.existsSync(statePath)) {
    state = JSON.parse(fs.readFileSync(statePath, "utf8"));
  }

  if (!state.onboarding) {
    state.onboarding = {
      mode: "brownfield",
      date: new Date().toISOString(),
      missing: [],
      deferred: [],
      retroactiveFeatures: [],
      verifiedFeatures: [],
      notes: ""
    };
  }

  if (!state.onboarding.migrated) {
    state.onboarding.migrated = [];
  }

  for (const item of migrated) {
    // Add to features list if not present
    const existing = state.features.find((f) => f.name === item.feature);

    if (!existing) {
      const approved = approvedGateValue();

      state.features.push({
        name: item.feature,
        currentVersion: "v1",
        versions: [
          {
            id: "v1",
            phase: item.lifecycle === "completed" ? "completed" : "ongoing",
            currentStep: "review",
            gates: {
              specification: item.lifecycle === "completed" ? approved : "pending",
              planning: item.lifecycle === "completed" ? approved : "pending",
              implementation: item.lifecycle === "completed" ? approved : "pending",
              review: item.lifecycle === "completed" ? approved : "pending"
            },
            history: [
              {
                event: "feature_migrated",
                timestamp: new Date().toISOString(),
                by: "onboarding"
              }
            ]
          }
        ]
      });
    }

    // Add migration record
    state.onboarding.migrated.push({
      feature: item.feature,
      from: "openspec",
      date: new Date().toISOString(),
      source: item.source
    });

    // Add to retroactive features
    if (!state.onboarding.retroactiveFeatures.includes(item.feature)) {
      state.onboarding.retroactiveFeatures.push(item.feature);
    }
  }

  fs.writeFileSync(statePath, JSON.stringify(state, null, 2));
}

function main() {
  console.log("🔍 Migrating from OpenSpec...\n");

  const migrated = migrate();

  if (migrated.length === 0) {
    console.log("\n⚠️  No OpenSpec features found to migrate.");
    process.exit(0);
  }

  updateWorkflowState(migrated);

  console.log("");
  console.log(`✅ Migration complete: ${migrated.length} features.`);
  console.log("");
  console.log("Original openspec/ folder was NOT deleted.");
  console.log("All migrated features are marked retroactive: true.");
  console.log("");
  console.log("Next: run sdrive validate to confirm the new files.");
}

main();