#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();
const sdriveRoot = path.join(projectRoot, ".sdrive");

const speckitDir = path.join(projectRoot, ".specify");
const speckitSpecsDir = path.join(speckitDir, "specs");
const speckitMemoryDir = path.join(speckitDir, "memory");
const speckitConstitution = path.join(speckitMemoryDir, "constitution.md");

if (!fs.existsSync(speckitDir)) {
  console.error("❌ .specify/ folder not found. Nothing to migrate.");
  process.exit(1);
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
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

function stripNumberPrefix(name) {
  // Spec-Kit uses 001-feature-name
  return name.replace(/^[0-9]+[-_]/, "");
}

function slugify(name) {
  return name
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

function findInDir(dir, names) {
  for (const name of names) {
    const full = path.join(dir, name);
    if (fs.existsSync(full)) return full;
  }
  return null;
}

function writeSpecDriveFeature(featureName, sourceDir) {
  const featureFolder = path.join(
    sdriveRoot,
    "specs",
    "ongoing",
    featureName,
    "v1"
  );
  const govFolder = path.join(sdriveRoot, "governance", featureName, "v1");

  ensureDir(featureFolder);
  ensureDir(govFolder);

  const specSource = findInDir(sourceDir, ["spec.md", "specification.md"]);
  const planSource = findInDir(sourceDir, ["plan.md", "technical-plan.md"]);
  const tasksSource = findInDir(sourceDir, ["tasks.md", "todo.md"]);

  const now = new Date().toISOString();

  const specContent = specSource
    ? readFile(specSource)
    : `# ${featureName}\n\n(Migrated from Spec-Kit — no source spec found)`;

  const planContent = planSource
    ? readFile(planSource)
    : `# Technical Plan: ${featureName} (migrated)\n\nThis plan was migrated from Spec-Kit. No technical plan was extracted.\n`;

  const tasksContent = tasksSource
    ? readFile(tasksSource)
    : `# Execution Tasks: ${featureName}\n\n(Migrated from Spec-Kit — no tasks found)\n`;

  // spec.md
  fs.writeFileSync(
    path.join(featureFolder, "spec.md"),
    `---
title: ${featureName}
status: ongoing
version: v1
created: ${now}
updated: ${now}
retroactive: true
migratedFrom: speckit
---

${specContent}
`
  );

  // plan.md
  fs.writeFileSync(path.join(featureFolder, "plan.md"), planContent);

  // tasks.md
  fs.writeFileSync(path.join(featureFolder, "tasks.md"), tasksContent);

  // governance JSON
  fs.writeFileSync(
    path.join(govFolder, "spec.json"),
    JSON.stringify(
      {
        feature: featureName,
        title: featureName,
        version: 1,
        retroactive: true,
        migratedFrom: "speckit",
        overview: {
          problemStatement: "(migrated from Spec-Kit)",
          proposedSolution: "(migrated from Spec-Kit)",
          targetAudience: "(migrated from Spec-Kit)"
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
    spec: specSource ? path.relative(projectRoot, specSource) : null,
    plan: planSource ? path.relative(projectRoot, planSource) : null,
    tasks: tasksSource ? path.relative(projectRoot, tasksSource) : null
  };
}

function migrateSpecs() {
  const migrated = [];

  if (!fs.existsSync(speckitSpecsDir)) {
    return migrated;
  }

  const folders = listDirectories(speckitSpecsDir);

  for (const folder of folders) {
    const cleaned = stripNumberPrefix(folder);
    const featureName = slugify(cleaned);
    const sourceDir = path.join(speckitSpecsDir, folder);

    const result = writeSpecDriveFeature(featureName, sourceDir);
    migrated.push(result);
    console.log(`✅ Migrated spec: ${featureName}`);
  }

  return migrated;
}

function migrateConstitution() {
  if (!fs.existsSync(speckitConstitution)) {
    return false;
  }

  const content = readFile(speckitConstitution);
  if (!content) return false;

  const targetConstitution = path.join(sdriveRoot, "constitution.md");

  if (fs.existsSync(targetConstitution)) {
    // Append instead of overwrite
    const existing = readFile(targetConstitution) || "";
    fs.writeFileSync(
      targetConstitution,
      `${existing}\n\n---\n\n## Migrated from Spec-Kit\n\n${content}\n`
    );
    console.log("✅ Appended Spec-Kit constitution to existing SpecDrive constitution.");
  } else {
    fs.writeFileSync(
      targetConstitution,
      `# Project Constitution\n\n> Migrated from Spec-Kit\n\n${content}\n`
    );
    console.log("✅ Created SpecDrive constitution from Spec-Kit constitution.");
  }

  return true;
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
    const existing = state.features.find((f) => f.name === item.feature);

    if (!existing) {
      state.features.push({
        name: item.feature,
        currentVersion: "v1",
        versions: [
          {
            id: "v1",
            phase: "ongoing",
            currentStep: "review",
            gates: {
              specification: "pending",
              planning: "pending",
              implementation: "pending",
              review: "pending"
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

    state.onboarding.migrated.push({
      feature: item.feature,
      from: "speckit",
      date: new Date().toISOString(),
      source: item.source
    });

    if (!state.onboarding.retroactiveFeatures.includes(item.feature)) {
      state.onboarding.retroactiveFeatures.push(item.feature);
    }
  }

  fs.writeFileSync(statePath, JSON.stringify(state, null, 2));
}

function main() {
  console.log("🔍 Migrating from Spec-Kit...\n");

  const migrated = migrateSpecs();
  const constitutionMigrated = migrateConstitution();

  if (migrated.length === 0 && !constitutionMigrated) {
    console.log("\n⚠️  No Spec-Kit features or constitution found to migrate.");
    process.exit(0);
  }

  updateWorkflowState(migrated);

  console.log("");
  console.log(`✅ Migration complete: ${migrated.length} features.`);
  if (constitutionMigrated) {
    console.log("✅ Constitution merged.");
  }
  console.log("");
  console.log("Original .specify/ folder was NOT deleted.");
  console.log("All migrated features are marked retroactive: true.");
  console.log("");
  console.log("Next: run sdrive validate to confirm the new files.");
}

main();