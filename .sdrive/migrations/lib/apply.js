const fs = require("fs");
const path = require("path");
const {
  ensureDir,
  readText,
  readJson,
  writeJsonAtomic,
  hashFile,
  relative
} = require("./files");

function migrationId() {
  const stamp = new Date().toISOString().replace(/[-:.TZ]/g, "").slice(0, 14);
  const random = Math.random().toString(36).slice(2, 6);
  return `mig-${stamp}-${random}`;
}

function stripFrontmatter(markdown) {
  return String(markdown || "").replace(/^---\r?\n[\s\S]*?\r?\n---\r?\n?/, "");
}

function frontmatter(feature, adapter, lifecycle, source) {
  const now = new Date().toISOString();

  return [
    "---",
    `title: ${feature}`,
    `status: ${lifecycle}`,
    "version: v1",
    `created: ${now}`,
    `updated: ${now}`,
    "retroactive: true",
    `migratedFrom: ${adapter}`,
    `migratedAt: ${now}`,
    `source: ${source}`,
    "---",
    ""
  ].join("\n");
}

function readGates(root) {
  try {
    return require(path.join(root, ".sdrive", "commands", "gates.js"));
  } catch (error) {
    return null;
  }
}

function scaffoldGovernance(root, feature, adapterId, mapping) {
  const gates = readGates(root);
  const approved = gates ? gates.GATE_APPROVED : "approved";
  const govDir = path.join(root, ".sdrive", "governance", feature, "v1");
  ensureDir(govDir);

  const now = new Date().toISOString();
  const files = {
    "spec.json": {
      feature,
      title: feature,
      version: 1,
      overview: {
        problemStatement: `Migrated from ${adapterId}; replace this with the source problem statement.`,
        proposedSolution: `Migrated from ${adapterId}; replace this with the intended solution.`,
        targetAudience: "project users"
      },
      userStories: [
        {
          id: "US-001",
          title: "Migrated feature placeholder",
          role: "user",
          want: "the migrated behavior",
          benefit: "the behavior is preserved",
          acceptanceCriteria: [
            {
              id: "AC-001",
              text: "Placeholder: replace with a migrated acceptance criterion before review."
            }
          ]
        }
      ],
      functionalRequirements: [
        {
          id: "FR-001",
          text: "The migrated feature MUST be completed and verified before release."
        }
      ]
    },
    "plan.json": {
      feature,
      version: 1,
      architecture: {
        approach: `Migrated from ${adapterId}; replace this with the source technical plan.`,
        components: [],
        referencedRequirementIds: ["FR-001"]
      },
      dataModel: {
        entities: [],
        flowDescription: `Migrated from ${adapterId}; confirm the data flow.`
      }
    },
    "tasks.json": {
      feature,
      version: 1,
      phases: [
        {
          phaseId: "P1",
          name: "Migrated work",
          tasks: [
            {
              id: "T1.1",
              title: "Complete the migrated feature",
              description: "Replace the migrated placeholder with real tasks from the source.",
              linkedRequirements: ["FR-001"],
              linkedAcceptanceCriteria: ["AC-001"],
              completed: false
            }
          ]
        }
      ]
    },
    "traceability.json": {
      feature,
      version: 1,
      rows: [
        {
          requirementId: "FR-001",
          acceptanceCriteriaIds: ["AC-001"],
          taskIds: ["T1.1"],
          testIds: [],
          status: "not_started"
        }
      ]
    },
    "migration.json": {
      feature,
      version: "v1",
      from: adapterId,
      migratedAt: now,
      status: "scaffold",
      mapping,
      note: "Migrated source imported as a schema-valid placeholder. Replace the placeholder requirements, tasks, and tests before review."
    }
  };

  const written = [];

  for (const [name, value] of Object.entries(files)) {
    const file = path.join(govDir, name);
    writeJsonAtomic(file, value);
    written.push(file);
  }

  return { written, approved, govDir };
}

function applyPlan(root, plan, options = {}) {
  const id = options.id || migrationId();
  const snapshotDir = path.join(root, ".sdrive", ".backups", "migrations", id);
  ensureDir(snapshotDir);

  const statePath = path.join(root, ".sdrive", "workflow-state.json");
  const previousState = readJson(statePath, { features: [] });
  const stateBackup = path.join(snapshotDir, "workflow-state.json");
  writeJsonAtomic(stateBackup, previousState);

  const state = JSON.parse(JSON.stringify(previousState));
  const createdFiles = [];
  const createdDirs = [];
  const features = [];

  const rememberDir = (dir) => {
    if (!fs.existsSync(dir)) createdDirs.push(relative(root, dir));
    ensureDir(dir);
  };

  for (const feature of plan.features) {
    if (feature.status === "conflict") {
      features.push({ ...feature, status: "skipped", reason: "target already exists" });
      continue;
    }

    const existing = (state.features || []).find((entry) => entry.name === feature.feature);
    if (existing) {
      features.push({ ...feature, status: "skipped", reason: "feature already exists in workflow-state.json" });
      continue;
    }

    const targetDir = path.join(root, feature.targetDir);
    rememberDir(targetDir);

    try {
      for (const mapping of feature.files) {
        const target = path.join(root, mapping.target);
        rememberDir(path.dirname(target));

        const sourceText = mapping.source ? readText(path.join(root, mapping.source)) : null;
        const body = sourceText
          ? stripFrontmatter(sourceText)
          : `# ${path.basename(mapping.target)}\n\n(Migrated from ${plan.name}; source did not contain this document.)\n`;
        const header = frontmatter(feature.feature, plan.framework, feature.targetLifecycle, feature.source);
        fs.writeFileSync(target, `${header}\n${body}`.replace(/\s+$/, "") + "\n");
        createdFiles.push(relative(root, target));
      }

      const governance = scaffoldGovernance(root, feature.feature, plan.framework, feature.files);
      createdFiles.push(...governance.written.map((file) => relative(root, file)));

      const approved = feature.targetLifecycle === "completed";
      state.features = state.features || [];
      state.features.push({
        name: feature.feature,
        currentVersion: "v1",
        versions: [
          {
            id: "v1",
            phase: feature.targetLifecycle,
            currentStep: "specification",
            gates: {
              specification: approved ? governance.approved : "pending",
              planning: approved ? governance.approved : "pending",
              implementation: approved ? governance.approved : "pending",
              review: approved ? governance.approved : "pending"
            },
            history: [
              {
                event: "feature_migrated",
                by: "migration",
                timestamp: new Date().toISOString()
              }
            ]
          }
        ]
      });

      state.onboarding = state.onboarding || {
        mode: "brownfield",
        date: new Date().toISOString(),
        missing: [],
        deferred: [],
        retroactiveFeatures: [],
        verifiedFeatures: [],
        notes: ""
      };
      state.onboarding.migrated = state.onboarding.migrated || [];
      state.onboarding.migrated.push({
        feature: feature.feature,
        from: plan.framework,
        date: new Date().toISOString(),
        source: feature.source
      });
      state.onboarding.retroactiveFeatures = state.onboarding.retroactiveFeatures || [];
      if (!state.onboarding.retroactiveFeatures.includes(feature.feature)) {
        state.onboarding.retroactiveFeatures.push(feature.feature);
      }

      features.push({ ...feature, status: "imported", governance: feature.governanceDir });
    } catch (error) {
      features.push({ ...feature, status: "failed", reason: error.message });
    }
  }

  writeJsonAtomic(statePath, state);

  const report = {
    migrationId: id,
    framework: plan.framework,
    name: plan.name,
    appliedAt: new Date().toISOString(),
    sourceRoot: plan.root,
    sourceConfidence: plan.confidence,
    counts: {
      planned: plan.summary.features,
      imported: features.filter((feature) => feature.status === "imported").length,
      skipped: features.filter((feature) => feature.status === "skipped").length,
      failed: features.filter((feature) => feature.status === "failed").length
    },
    features,
    rollback: {
      snapshot: relative(root, snapshotDir),
      stateBackup: relative(root, stateBackup),
      stateHash: hashFile(stateBackup),
      createdFiles,
      createdDirs
    }
  };

  const reportDir = path.join(root, ".sdrive", "migration-runs", id);
  ensureDir(reportDir);
  writeJsonAtomic(path.join(reportDir, "report.json"), report);

  return { report, reportFile: path.join(reportDir, "report.json") };
}

module.exports = {
  migrationId,
  applyPlan
};
