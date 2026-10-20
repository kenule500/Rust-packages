// Archive finalization.
//
// Closing a feature's lifecycle is the one operation that must never happen on trust.
// Before this module existed, `/sdrive:archive` was an agent prompt that read a single
// string (`Status: PASS` in review.md) and then wrote the gates itself — setting them
// all to "approved" whether or not anything had verified them. A feature that was never
// tested or reviewed could be merged AND recorded as verified, because the record was
// written by the same prose that was supposed to check it.
//
// The split is now:
//   `sdrive approve <feature> review`  -> CHECKS the evidence and writes the approval
//   `sdrive archive:finalize <feature>` -> CLOSES the lifecycle, but only if every
//                                          approval already exists and the evidence
//                                          behind them is still present
//
// This module never promotes a gate. An outstanding gate is a refusal, not a shortcut.

const fs = require("fs");
const path = require("path");

const { GATE_ORDER, GATE_APPROVED, SPEC_STAGES, findSpecLocation, checkGateRequirements, resolveVersion, isGateApprovedFor, gateStatusFor } = require("./gates");

function readJson(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    return null;
  }
}

function loadState(root) {
  return readJson(path.join(root, ".sdrive", "workflow-state.json"));
}

// checkGateRequirements() repeats a few verdicts this module already reaches on its own
// (phase, archived location, prerequisite gates). Filter those out so a refusal reads as
// one list of reasons rather than the same reason twice.
function isDuplicateVerdict(blocker) {
  return (
    /^Prerequisite gate /.test(blocker) ||
    /^Version ".*" is already completed\.$/.test(blocker) ||
    /is archived in specs\/completed\//.test(blocker) ||
    /^No spec folder for /.test(blocker)
  );
}

// Is this feature STACKED on another in-flight feature?
//
// Determined entirely from the records, so it needs no git call and gives the same
// answer every time: a feature is stacked when its recorded `base` is ANOTHER FEATURE'S
// own branch (`feature.branch` of a peer) and that peer has not landed yet.
//
// `base` naming something that is not any feature's branch - `main`, `develop`,
// `release/1.0` - is an integration branch. That is the normal case, not stacking, and
// this is why the test is "is it a peer's branch" rather than "does it equal main":
// a Git Flow project merges into `develop`, and calling that stacked would be wrong.
function findStackingBlocker(state, feature) {
  const base = feature.base;

  if (!base) return null;

  const peer = (state.features || []).find(
    (other) => other.name !== feature.name && other.branch === base
  );

  if (!peer) return null;

  const peerVersions = peer.versions || [];
  const peerVersion =
    peerVersions.find((v) => v.id === peer.currentVersion) || peerVersions[0];

  if (peerVersion && peerVersion.phase === "completed") {
    // The parent has landed, so this is historical metadata - not a live blocker.
    return null;
  }

  return (
    `"${feature.name}" is STACKED on "${peer.name}" - its branch "${base}" has not landed. ` +
    `Archive "${peer.name}" first. This feature then lands on the integration branch, never into "${base}"; ` +
    `merging it now would ship "${peer.name}"'s unaudited commits (CON-611).`
  );
}

// Read-only verdict. Never writes anything, so it is safe to run over every feature.
function checkArchivable(root, featureName) {
  const blockers = [];
  const warnings = [];

  const state = loadState(root);

  if (!state) {
    return {
      ok: false,
      alreadyFinalized: false,
      blockers: ["No readable .sdrive/workflow-state.json - run `sdrive init` first."],
      warnings
    };
  }

  const feature = (state.features || []).find((f) => f.name === featureName);

  if (!feature) {
    return {
      ok: false,
      alreadyFinalized: false,
      blockers: [`Feature "${featureName}" is not in workflow-state.json.`],
      warnings
    };
  }

  const version = resolveVersion(feature);

  if (!version) {
    return {
      ok: false,
      alreadyFinalized: false,
      blockers: [`Feature "${featureName}" has no versions.`],
      warnings
    };
  }

  const location = findSpecLocation(root, featureName, version.id);
  const completedFeatureDir = path.join(root, ".sdrive", "specs", "completed", featureName);
  const completedVersionDir = path.join(completedFeatureDir, version.id);

  // Already done. Idempotent by design: a re-run must never duplicate history entries.
  if (version.phase === "completed" && fs.existsSync(completedVersionDir)) {
    return { ok: true, alreadyFinalized: true, feature, version, location, blockers, warnings };
  }

  if (!location) {
    blockers.push(
      `No spec folder for "${featureName}" ${version.id} under .sdrive/specs/{${SPEC_STAGES.join(", ")}}/.`
    );
  }

  // specs/completed/<feature>/ existing WITHOUT this version is normal: an earlier version
  // of the same feature is already released, and versions are archived individually, so
  // the two coexist. The contradiction is when the folder is there but the record does not
  // agree that this version is completed.
  if (fs.existsSync(completedVersionDir) && version.phase !== "completed") {
    blockers.push(
      `specs/completed/${featureName}/${version.id}/ already exists but the version is not marked completed. That state contradicts itself - resolve it by hand.`
    );
  }

  if (version.phase === "completed") {
    blockers.push(
      `Version "${version.id}" is already marked completed but its spec is not in specs/completed/. That state contradicts itself - resolve it by hand.`
    );
  } else if (version.phase !== "ongoing") {
    blockers.push(
      `Version "${version.id}" is in phase "${version.phase}", not "ongoing". Only an in-flight feature can be finalized.`
    );
  }

  if (location && location.stage !== "ongoing") {
    warnings.push(`Spec lives in specs/${location.stage}/ rather than specs/ongoing/.`);
  }

  // Every gate must ALREADY be approved. This command records a closure; it does not
  // grant approvals. If a gate is outstanding, that is the refusal. The test goes through
  // the shared accessor so this checker and `sdrive approve` cannot disagree.
  const outstanding = GATE_ORDER.filter((gate) => !isGateApprovedFor(version, gate));

  outstanding.forEach((gate) => {
    blockers.push(
      `Gate "${gate}" is "${gateStatusFor(version, gate)}", not "${GATE_APPROVED}" - run \`sdrive approve ${featureName} ${gate}\` first.`
    );
  });

  // Re-verify the evidence NOW. An approval granted yesterday is not proof that the
  // evidence still exists today: traceability.json could have been deleted, or
  // review.md edited, between the approval and this moment.
  const requirements = checkGateRequirements(root, feature, version, "review");

  requirements.blockers
    .filter((blocker) => !isDuplicateVerdict(blocker))
    .forEach((blocker) => blockers.push(`Evidence changed since approval: ${blocker}`));

  requirements.warnings.forEach((warning) => warnings.push(warning));

  // Test and review must actually have RUN, not merely have been recorded as if they
  // had. traceability.json can be hand-edited to say "complete" without a test ever
  // existing; the test report is written only by /sdrive:test, so its presence is the
  // evidence that the run happened. Enforced here as a blocker because archive is the
  // last moment it can be checked: once the feature is in specs/completed/ it is already
  // on the target branch, and no later gate can pull it back.
  const reportsDir = path.join(root, ".sdrive", "reports", "tests");
  const reportPrefix = `test-report-${featureName}-${version.id}-`;
  const testReports = fs.existsSync(reportsDir)
    ? fs.readdirSync(reportsDir).filter((file) => file.startsWith(reportPrefix))
    : [];

  if (testReports.length === 0) {
    blockers.push(
      `No test report for ${featureName} ${version.id} (expected ${reportPrefix}<date>.md in .sdrive/reports/tests/) - the tests have not been run. ` +
        `Run \`/sdrive:test ${featureName}\` first, then re-run \`sdrive archive:check ${featureName}\`.`
    );
  }

  // The branch this feature came from is what it must go back to. When `base` is missing
  // - every feature proposed before base was recorded - the agent falls back to the
  // integration branch, but the fallback is a guess and is reported as one.
  if (!feature.base) {
    warnings.push(
      `No recorded "base" for ${featureName} - the branch it came from is unknown. The archive agent will fall back to the integration branch. New features record it in /sdrive:propose.`
    );
  }

  // Ordering, not evidence: a stacked feature has perfectly good approvals and still
  // cannot be merged yet. Reported as a blocker because the merge is part of archiving,
  // and its wording says ORDER so it is not mistaken for missing evidence.
  const stacking = findStackingBlocker(state, feature);

  if (stacking) {
    blockers.push(stacking);
  }

  return {
    ok: blockers.length === 0,
    alreadyFinalized: false,
    feature,
    version,
    location,
    blockers,
    warnings
  };
}

// Perform the closure. Only ever called after checkArchivable() returns ok.
function finalizeArchive(root, featureName, options = {}) {
  const verdict = checkArchivable(root, featureName);

  if (!verdict.ok) {
    return { ok: false, blockers: verdict.blockers, warnings: verdict.warnings };
  }

  if (verdict.alreadyFinalized) {
    return { ok: true, alreadyFinalized: true, version: verdict.version, warnings: verdict.warnings };
  }

  const { version, location } = verdict;
  const sourceVersionDir = location.dir;
  const sourceFeatureDir = path.dirname(sourceVersionDir);
  const destFeatureDir = path.join(root, ".sdrive", "specs", "completed", featureName);
  const destVersionDir = path.join(destFeatureDir, version.id);

  fs.mkdirSync(destFeatureDir, { recursive: true });

  try {
    // Only THIS version's folder moves. An earlier release of the same feature stays
    // exactly where it is, so archiving v2 never disturbs a released v1. A plain
    // filesystem move rather than `git mv`: it also works when the spec folder is
    // untracked, and `git add -A` picks the rename up for the commit afterwards.
    fs.renameSync(sourceVersionDir, destVersionDir);
  } catch (error) {
    return {
      ok: false,
      blockers: [`Could not move "${sourceVersionDir}" to "${destVersionDir}": ${error.message}`],
      warnings: verdict.warnings
    };
  }

  // Verify the move BEFORE writing the record. A lifecycle marked complete whose spec
  // never actually moved is precisely the false record this module exists to prevent.
  const moved = fs.existsSync(destVersionDir) && !fs.existsSync(sourceVersionDir);

  if (!moved) {
    try {
      if (fs.existsSync(destVersionDir) && !fs.existsSync(sourceVersionDir)) {
        fs.renameSync(destVersionDir, sourceVersionDir);
      }
    } catch (error) {
      // Reported by the message below; restoring is best-effort.
    }

    return {
      ok: false,
      blockers: [
        "The spec folder did not end up in specs/completed/ as expected. The lifecycle was left unchanged."
      ],
      warnings: verdict.warnings
    };
  }

  // The source feature folder is only removed when this was its last version. A released
  // sibling version keeps its folder.
  try {
    if (fs.existsSync(sourceFeatureDir) && fs.readdirSync(sourceFeatureDir).length === 0) {
      fs.rmdirSync(sourceFeatureDir);
    }
  } catch (error) {
    // An empty folder left behind is untidy, not a false record.
  }

  // Only now, with the folder verified in place, is the record written.
  const statePath = path.join(root, ".sdrive", "workflow-state.json");
  const state = loadState(root);
  const target = (state.features || []).find((f) => f.name === featureName);
  const targetVersion = (target.versions || []).find((v) => v.id === version.id);

  targetVersion.phase = "completed";
  // Gates are deliberately NOT touched here. checkArchivable() already established that
  // all four are approved, so writing them would be a no-op - and writing them anyway is
  // how archive used to manufacture approvals that never happened.
  targetVersion.history = targetVersion.history || [];
  targetVersion.history.push({
    event: "feature_archived",
    by: options.actor || "",
    from: location.stage,
    version: targetVersion.id,
    mergeCommit: options.mergeCommit || "",
    timestamp: new Date().toISOString()
  });

  // Atomic replace: a crash must not leave a truncated workflow-state.json.
  const tempStatePath = `${statePath}.${process.pid}.tmp`;
  fs.writeFileSync(tempStatePath, `${JSON.stringify(state, null, 2)}\n`);
  fs.renameSync(tempStatePath, statePath);

  // Governance deliberately stays at .sdrive/governance/<feature>/<version>/ (flat).
  // The dashboard reads that path, and validate-governance.js would treat a
  // governance/completed/ folder as a feature named "completed". Archival is recorded
  // by the version's phase and by the spec's location, not by moving governance.
  return {
    ok: true,
    alreadyFinalized: false,
    version: targetVersion.id,
    from: location.stage,
    governanceMoved: false,
    warnings: verdict.warnings
  };
}

module.exports = { checkArchivable, finalizeArchive };
