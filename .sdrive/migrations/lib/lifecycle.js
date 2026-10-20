const fs = require("fs");
const path = require("path");
const { readJson, writeJsonAtomic, hashFile, relative } = require("./files");

function migrationDir(root, id) {
  return path.join(root, ".sdrive", "migration-runs", id);
}

function loadMigration(root, id) {
  const file = path.join(migrationDir(root, id), "report.json");
  const report = readJson(file, null);

  if (!report) {
    const error = new Error(`Migration "${id}" not found at ${relative(root, file)}`);
    error.code = "ENOENT";
    throw error;
  }

  return { report, file };
}

function verifyMigration(root, id, options = {}) {
  const { report } = loadMigration(root, id);
  const blockers = [];
  const warnings = [];
  const features = [];

  for (const feature of report.features || []) {
    if (feature.status !== "imported") continue;

    const missing = [];

    for (const mapping of feature.files || []) {
      if (!fs.existsSync(path.join(root, mapping.target))) missing.push(mapping.target);
    }

    if (!fs.existsSync(path.join(root, feature.governanceDir, "migration.json"))) {
      missing.push(`${feature.governanceDir}/migration.json`);
    }

    if (missing.length > 0) {
      blockers.push(`${feature.feature}: missing ${missing.join(", ")}`);
    }

    features.push({ feature: feature.feature, status: missing.length ? "invalid" : "present", missing });
  }

  // Strict mode validates the migrated governance JSON. Default mode only
  // verifies the import itself, because a migrated scaffold is expected to need
  // completion before the review gate can pass.
  if (options.strict) {
    try {
      const { validateArtifacts } = require(path.join(__dirname, "..", "..", "scripts", "validate-governance"));
      const featureNames = features.map((entry) => entry.feature);

      for (const feature of featureNames) {
        const validation = validateArtifacts(root, { feature, only: ["features"] });
        const invalid = validation.results.filter((result) => result.status === "invalid");

        for (const result of invalid) {
          blockers.push(`${feature}: ${result.file} is invalid (${result.errors.length} schema error(s))`);
        }
      }
    } catch (error) {
      warnings.push(`Strict schema validation could not run: ${error.message}`);
    }
  } else {
    warnings.push(
      "Migrated governance is a schema-valid placeholder (US-001/AC-001/FR-001/T1.1); replace it with the source requirements before review. Run with --strict to validate schemas."
    );
  }

  const ok = blockers.length === 0;
  const verification = {
    migrationId: id,
    verifiedAt: new Date().toISOString(),
    ok,
    blockers,
    warnings,
    features,
    reportFeatures: report.counts
  };

  writeJsonAtomic(path.join(migrationDir(root, id), "verification.json"), verification);

  return verification;
}

function rollbackMigration(root, id, options = {}) {
  const { report } = loadMigration(root, id);
  const rollbackFile = path.join(migrationDir(root, id), "rollback.json");

  if (readJson(rollbackFile, null) && !options.force) {
    return { ok: true, alreadyRolledBack: true, migrationId: id };
  }

  const blockers = [];

  // Restore workflow state first, so a crash mid-cleanup still leaves a usable
  // project rather than a state file that references removed folders.
  const statePath = path.join(root, ".sdrive", "workflow-state.json");
  const stateBackup = path.join(root, report.rollback.stateBackup);

  if (fs.existsSync(stateBackup)) {
    fs.copyFileSync(stateBackup, statePath);
  } else {
    blockers.push(`State backup missing: ${report.rollback.stateBackup}`);
  }

  for (const file of report.rollback.createdFiles || []) {
    const full = path.join(root, file);
    if (fs.existsSync(full)) fs.rmSync(full, { force: true });
  }

  const dirs = [...(report.rollback.createdDirs || [])].sort(
    (a, b) => b.split("/").length - a.split("/").length
  );

  const protectedRoots = new Set([
    path.join(root, ".sdrive"),
    path.join(root, ".sdrive", "specs"),
    path.join(root, ".sdrive", "governance")
  ]);

  const removeEmptyParents = (start) => {
    let current = path.dirname(start);

    while (
      current.startsWith(path.join(root, ".sdrive")) &&
      !protectedRoots.has(current)
    ) {
      try {
        if (fs.existsSync(current) && fs.readdirSync(current).length === 0) {
          fs.rmdirSync(current);
        } else {
          break;
        }
      } catch (error) {
        break;
      }

      current = path.dirname(current);
    }
  };

  for (const dir of dirs) {
    const full = path.join(root, dir);
    try {
      if (fs.existsSync(full) && fs.readdirSync(full).length === 0) fs.rmdirSync(full);
      removeEmptyParents(full);
    } catch (error) {
      // A non-empty directory is left in place; it may contain user work.
    }
  }

  const record = {
    migrationId: id,
    rolledBackAt: new Date().toISOString(),
    restoredState: relative(root, statePath),
    restoredStateHash: fs.existsSync(statePath) ? hashFile(statePath) : null,
    blockers
  };

  writeJsonAtomic(rollbackFile, record);

  return { ok: blockers.length === 0, alreadyRolledBack: false, migrationId: id, blockers, record };
}

module.exports = {
  migrationDir,
  loadMigration,
  verifyMigration,
  rollbackMigration
};
