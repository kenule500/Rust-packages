// Shared helpers for locating governance artifacts.
//
// SpecDrive stores governance JSON inside a version folder:
//   .sdrive/governance/<feature>/<version>/spec.json
//
// Older projects (and projects created before versioning was introduced) may
// still use the flat layout:
//   .sdrive/governance/<feature>/spec.json
//
// Every consumer should resolve paths through these helpers so both layouts
// keep working.

const fs = require("fs");
const path = require("path");

function governanceRoot(projectRoot) {
  return path.join(projectRoot, ".sdrive", "governance");
}

function featureDir(projectRoot, featureName) {
  return path.join(governanceRoot(projectRoot), featureName);
}

// Returns the version folder names for a feature, sorted so that v1, v2, ...
// come before v10 (numeric-aware). Returns an empty array when the feature has
// no version folders (flat layout) or does not exist.
function listVersions(projectRoot, featureName) {
  const dir = featureDir(projectRoot, featureName);
  if (!fs.existsSync(dir)) return [];

  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name)
    .sort((a, b) => {
      const na = parseInt(String(a).replace(/^v/i, ""), 10);
      const nb = parseInt(String(b).replace(/^v/i, ""), 10);
      if (Number.isNaN(na) || Number.isNaN(nb)) return a.localeCompare(b);
      return na - nb;
    });
}

// Resolves the directory that holds a feature's governance JSON.
//
// - When `version` is given and that version folder exists, it is used.
// - Otherwise the latest version folder is used.
// - When no version folders exist, the flat feature directory is returned.
//
// Returns null when the feature has no governance directory at all.
function resolveGovernanceDir(projectRoot, featureName, version) {
  const dir = featureDir(projectRoot, featureName);
  if (!fs.existsSync(dir)) return null;

  if (version) {
    const requested = path.join(dir, version);
    if (fs.existsSync(requested)) return requested;
  }

  const versions = listVersions(projectRoot, featureName);
  if (versions.length > 0) {
    return path.join(dir, versions[versions.length - 1]);
  }

  return dir;
}

// Resolves a single governance file, preferring the versioned layout and
// falling back to the flat layout. Returns null when the file is not found.
function resolveGovernanceFile(projectRoot, featureName, version, fileName) {
  const dir = resolveGovernanceDir(projectRoot, featureName, version);
  if (!dir) return null;

  const candidate = path.join(dir, fileName);
  if (fs.existsSync(candidate)) return candidate;

  // Fall back to the flat layout when a version folder was resolved but the
  // file only exists at the feature root.
  const flat = path.join(featureDir(projectRoot, featureName), fileName);
  if (fs.existsSync(flat)) return flat;

  return null;
}

module.exports = {
  governanceRoot,
  featureDir,
  listVersions,
  resolveGovernanceDir,
  resolveGovernanceFile
};
