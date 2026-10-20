// Governance validation, as a library first and a CLI second.
//
// This file used to be a script: it ran on require, printed to stdout, and called
// process.exit() on the first missing schema. That made it correct and unusable - the
// dashboard could not ask "are this project's artifacts valid?" without spawning a
// child process and scraping its output, so the validation panel was left unbuilt.
//
// The rules now live in validateArtifacts(), which returns structured verdicts and never
// prints or exits. The CLI at the bottom is a thin wrapper that adds the console output
// and the exit code. Same checks, same messages, one implementation.

const Ajv = require("ajv");
const addFormats = require("ajv-formats");
const fs = require("fs");
const path = require("path");

const SPEC_STAGES = ["backlog", "ongoing", "completed"];

const FEATURE_FILES = [
  { file: "spec.json", schema: "spec.schema.json" },
  { file: "plan.json", schema: "plan.schema.json" },
  { file: "tasks.json", schema: "tasks.schema.json" },
  { file: "traceability.json", schema: "traceability.schema.json" },
  { file: "test-evidence.json", schema: "test-evidence.schema.json" },
  { file: "review.json", schema: "review.schema.json" }
];

// The installed script lives at <project>/.sdrive/scripts/, so two levels up is the
// project. The CLI may also be invoked from the project root, which is more explicit -
// prefer that when it actually looks like a SpecDrive project.
function defaultRoot() {
  const fromCwd = process.cwd();

  if (fs.existsSync(path.join(fromCwd, ".sdrive"))) {
    return fromCwd;
  }

  return path.join(__dirname, "..", "..");
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(filePath, "utf8"));
}

// Returns null when the document is valid, or the Ajv error list when it is not.
function checkAgainstSchema(data, schema) {
  const ajv = new Ajv({ allErrors: true });
  addFormats(ajv);

  const validate = ajv.compile(schema);
  return validate(data) ? null : validate.errors;
}

function discoverFeatures(root) {
  const features = new Set();
  const governanceDir = path.join(root, ".sdrive", "governance");
  const specsDir = path.join(root, ".sdrive", "specs");

  if (fs.existsSync(governanceDir)) {
    fs.readdirSync(governanceDir, { withFileTypes: true })
      .filter((d) => d.isDirectory())
      .forEach((d) => features.add(d.name));
  }

  SPEC_STAGES.forEach((lifecycle) => {
    const lifecyclePath = path.join(specsDir, lifecycle);

    if (fs.existsSync(lifecyclePath)) {
      fs.readdirSync(lifecyclePath, { withFileTypes: true })
        .filter((d) => d.isDirectory())
        .forEach((d) => features.add(d.name));
    }
  });

  return Array.from(features);
}

/**
 * Validate a project's governance artifacts.
 *
 * @param {string} [root]  Project root (the folder containing `.sdrive/`).
 * @param {object} [options]
 * @param {Function} [options.log]      Receives human-readable lines. Default: silent.
 * @param {string[]} [options.only]     Subset of `["config", "workflow-state", "features"]`.
 * @param {string} [options.feature]    Validate a single feature instead of discovering.
 * @returns {{ok: boolean, results: object[], skipped: string[]}}
 *          `results[]` is `{ file, label, status: "valid"|"invalid"|"skipped", errors[] }`
 *          with `file` relative to the project root, so a UI can link straight to it.
 */
function validateArtifacts(root = defaultRoot(), options = {}) {
  const emit = options.log || (() => {});
  const only = options.only || null;
  const wants = (name) => !only || only.includes(name);

  const schemaDir = path.join(root, ".sdrive", "schemas");
  const governanceDir = path.join(root, ".sdrive", "governance");

  const results = [];
  const skipped = [];
  let failed = false;

  const record = (file, label, status, errors = []) => {
    results.push({ file, label, status, errors });

    if (status === "invalid") {
      failed = true;
    }

    return status !== "invalid";
  };

  const skip = (file, label, message) => {
    emit(message);
    skipped.push(label);
    return record(file, label, "skipped");
  };

  // Validate one JSON file against one schema. Absent documents are skipped (a project
  // that has not reached a stage yet is not invalid); present-but-broken ones fail.
  const validateFile = (filePath, schemaPath, label, relativeFile) => {
    if (!fs.existsSync(filePath)) {
      return skip(relativeFile, label, `⚠️  ${label} not found. Skipping.`);
    }

    let schema;

    try {
      schema = readJson(schemaPath);
    } catch (error) {
      emit(`❌ Schema/file not found: ${schemaPath}`);
      return record(relativeFile, label, "invalid", [
        { message: `Schema not found: ${schemaPath}` }
      ]);
    }

    let data;

    try {
      data = readJson(filePath);
    } catch (error) {
      const errorList = [{ message: `Malformed JSON: ${error.message}` }];
      emit(`❌ ${label} is INVALID:`);
      emit(errorList);
      return record(relativeFile, label, "invalid", errorList);
    }

    const errors = checkAgainstSchema(data, schema);

    if (!errors) {
      emit(`✅ ${label} is valid.`);
      return record(relativeFile, label, "valid");
    }

    emit(`❌ ${label} is INVALID:`);
    emit(errors);
    return record(relativeFile, label, "invalid", errors);
  };

  if (wants("config")) {
    const configPath = path.join(root, ".sdrive", "config.json");
    const configSchemaPath = path.join(schemaDir, "config.schema.json");

    if (!fs.existsSync(configPath)) {
      skip("config.json", "config.json", "⚠️  config.json not found. Skipping.");
    } else if (!fs.existsSync(configSchemaPath)) {
      skip(
        "config.json",
        "config.json",
        "⚠️  config.schema.json not found. Skipping config validation."
      );
    } else {
      validateFile(configPath, configSchemaPath, "config.json", "config.json");
    }
  }

  if (wants("workflow-state")) {
    const workflowStatePath = path.join(root, ".sdrive", "workflow-state.json");
    const workflowSchemaPath = path.join(schemaDir, "workflow-state.schema.json");

    if (!fs.existsSync(workflowStatePath)) {
      skip(
        "workflow-state.json",
        "workflow-state.json",
        "⚠️  workflow-state.json not found. Skipping."
      );
    } else {
      validateFile(
        workflowStatePath,
        workflowSchemaPath,
        "workflow-state.json",
        "workflow-state.json"
      );
    }
  }

  if (wants("features")) {
    const features = options.feature ? [options.feature] : discoverFeatures(root);

    if (features.length === 0) {
      emit("⚠️  No features found under .sdrive/governance or .sdrive/specs.");
    }

    features.forEach((featureName) => {
      emit(`\n📁 Validating feature: ${featureName}`);

      const featureDir = path.join(governanceDir, featureName);

      if (!fs.existsSync(featureDir)) {
        skip(
          path.relative(root, featureDir),
          `${featureName} (governance)`,
          `⚠️  Feature "${featureName}" has no governance folder. Skipping.`
        );
        return;
      }

      // Governance JSON lives in a version folder (`.sdrive/governance/<feature>/<version>/`).
      // Older projects may still use the flat layout (`.sdrive/governance/<feature>/`), so
      // every version folder is validated and the flat layout is the fallback when none exist.
      const versionDirs = fs
        .readdirSync(featureDir, { withFileTypes: true })
        .filter((d) => d.isDirectory())
        .map((d) => d.name);

      const targets =
        versionDirs.length > 0
          ? versionDirs.map((version) => ({
              label: `${featureName}/${version}`,
              dir: path.join(featureDir, version)
            }))
          : [{ label: featureName, dir: featureDir }];

      targets.forEach(({ label, dir }) => {
        FEATURE_FILES.forEach(({ file, schema }) => {
          const filePath = path.join(dir, file);
          const relativeFile = path.relative(root, filePath);

          if (!fs.existsSync(filePath)) {
            skip(relativeFile, `${label}/${file}`, `⚠️  ${label}/${file} not found. Skipping.`);
            return;
          }

          validateFile(filePath, path.join(schemaDir, schema), `${label}/${file}`, relativeFile);
        });
      });
    });
  }

  return { ok: !failed, results, skipped };
}

// ---------- CLI ----------

function main() {
  console.log("🔍 SpecDrive Governance Validation\n");

  const { ok } = validateArtifacts(defaultRoot(), { log: (line) => console.log(line) });

  console.log("\n----------------------------------------");

  if (!ok) {
    console.log("❌ Validation FAILED. Fix the errors above.");
    process.exit(1);
  }

  console.log("✅ Governance validation complete. All files are valid.");
  process.exit(0);
}

if (require.main === module) {
  main();
}

module.exports = { validateArtifacts, defaultRoot, discoverFeatures, FEATURE_FILES };