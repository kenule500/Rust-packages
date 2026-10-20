const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");

const projectRoot = process.cwd();

function getWorkflowState() {
  const workflowStatePath = path.join(projectRoot, ".sdrive", "workflow-state.json");

  if (!fs.existsSync(workflowStatePath)) {
    console.error("❌ workflow-state.json not found. Run `sdrive init` first.");
    process.exit(1);
  }

  return JSON.parse(fs.readFileSync(workflowStatePath, "utf8"));
}

function saveWorkflowState(state) {
  const workflowStatePath = path.join(projectRoot, ".sdrive", "workflow-state.json");
  fs.writeFileSync(workflowStatePath, JSON.stringify(state, null, 2));
}

// Gates live on the feature's CURRENT VERSION (`versions[].gates`), never on the
// feature itself. Older builds read `feature.gates`, which does not exist, so
// every gate looked unapproved and `sdrive approve` silently did nothing.
function resolveVersion(feature) {
  if (!feature || !Array.isArray(feature.versions) || feature.versions.length === 0) {
    return null;
  }

  return (
    feature.versions.find((v) => v.id === feature.currentVersion) || feature.versions[0]
  );
}

function findFeature(state, featureName) {
  return (state.features || []).find((f) => f.name === featureName) || null;
}

function getFeature(featureName) {
  return findFeature(getWorkflowState(), featureName);
}

// ---------- GATE STATE: THE ONLY WAY TO READ A GATE ----------
//
// A gate's stored value is a schema detail, not something callers should compare
// themselves. Before these accessors existed, "is this approved?" was open-coded in
// ten places across six files: archive, router, pre-commit, the approve command, and
// two dashboard endpoints. Every copy is a silent break waiting to happen - the day a
// gate record grows a shape (e.g. { status, by, at }) the copies that nobody updated
// start refusing valid approvals without throwing. Read gates through here.

const GATE_APPROVED = "approved";

function gateStatusFor(version, gate) {
  if (!version) {
    return "pending";
  }

  return (version.gates && version.gates[gate]) || "pending";
}

function isGateApprovedFor(version, gate) {
  return gateStatusFor(version, gate) === GATE_APPROVED;
}

// "Every recorded gate is approved". Deliberately false when nothing is recorded:
// "no gates yet" is not the same claim as "all gates cleared".
function allGatesApproved(version) {
  const gates = (version && version.gates) || {};
  const names = Object.keys(gates);

  return names.length > 0 && names.every((gate) => gateStatusFor(version, gate) === GATE_APPROVED);
}

function gateStatus(featureName, gate) {
  const feature = getFeature(featureName);

  if (!feature) {
    return "unknown";
  }

  const version = resolveVersion(feature);

  if (!version) {
    return "unknown";
  }

  return gateStatusFor(version, gate);
}

function approveGate(featureName, gate) {
  // Read ONCE and mutate that same object. The old implementation called
  // getFeature() — which re-reads the file — so the mutation was applied to a
  // throwaway object and discarded when `state` was saved.
  const state = getWorkflowState();
  const feature = findFeature(state, featureName);

  if (!feature) {
    console.error(`❌ Feature "${featureName}" not found.`);
    return false;
  }

  const version = resolveVersion(feature);

  if (!version) {
    console.error(`❌ Feature "${featureName}" has no versions.`);
    return false;
  }

  version.gates = version.gates || {};
  version.gates[gate] = GATE_APPROVED;

  version.history = version.history || [];
  version.history.push({
    event: `${gate}_approved`,
    timestamp: new Date().toISOString()
  });

  saveWorkflowState(state);
  console.log(`✅ Gate "${gate}" approved for feature "${featureName}".`);
  return true;
}

function isGateApproved(featureName, gate) {
  return gateStatus(featureName, gate) === GATE_APPROVED;
}

// ---------- GATE REQUIREMENT VERIFICATION ----------
//
// Approving a gate is an assertion that the work it certifies is actually done.
// Without these checks `sdrive approve` would happily mark an empty or unfinished
// feature as approved and write a history/audit entry saying so — a governance
// record that contradicts the repository. Verification happens BEFORE any write,
// so a failed check leaves the state file untouched.

// Each gate certifies everything before it, so order matters.
const GATE_ORDER = ["specification", "planning", "implementation", "review"];

// Specs are lifecycle-partitioned; governance is deliberately not.
const SPEC_STAGES = ["backlog", "ongoing", "completed"];

// Locate where a feature version actually lives under specs/.
function findSpecLocation(root, featureName, versionId) {
  for (const stage of SPEC_STAGES) {
    const dir = path.join(root, ".sdrive", "specs", stage, featureName, versionId);

    if (fs.existsSync(dir)) {
      return { stage, dir };
    }
  }

  return null;
}

function readJsonIfPresent(file) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    return null;
  }
}

// `sdrive test` writes test-evidence.json next to the governance artifacts.
// Projects initialised before evidence enforcement existed do not have the
// config key; they keep the old behaviour until they opt in (init writes true
// for new projects, `sdrive config:set governance.requireTestEvidence true`
// opts an existing one in).
function testEvidencePolicy(root) {
  const config = readJsonIfPresent(path.join(root, ".sdrive", "config.json")) || {};
  const governance = config.governance || {};

  return {
    required: governance.requireTestEvidence === true,
    allowStale: governance.allowStaleEvidence === true,
    structuredReview: governance.requireStructuredReview === true
  };
}

function headCommit(root) {
  try {
    return execFileSync("git", ["rev-parse", "HEAD"], {
      cwd: root,
      encoding: "utf8",
      stdio: ["ignore", "pipe", "ignore"]
    }).trim();
  } catch (error) {
    return "";
  }
}

function countTasks(tasksJson) {
  let total = 0;
  let done = 0;

  for (const phase of (tasksJson && tasksJson.phases) || []) {
    for (const task of phase.tasks || []) {
      total += 1;

      if (task.completed === true) {
        done += 1;
      }
    }
  }

  return { total, done };
}

// Acceptance criteria are declared on the user stories in spec.json. Coverage is
// always measured against THESE, never against the traceability matrix's own rows —
// otherwise a criterion that is missing from the matrix would silently stop counting
// and the feature would look fully covered.
function collectAcceptanceCriteria(specJson) {
  const ids = [];

  for (const story of (specJson && specJson.userStories) || []) {
    for (const ac of story.acceptanceCriteria || []) {
      if (ac && ac.id && !ids.includes(ac.id)) {
        ids.push(ac.id);
      }
    }
  }

  return ids;
}

// A criterion counts as tested only when a traceability row links it to at least one
// test AND that row is complete. A row with no test ID, or one still in progress, is
// not evidence that anything was verified.
function collectTestedAcceptanceCriteria(traceJson) {
  const ids = [];

  for (const row of (traceJson && traceJson.rows) || []) {
    const linkedTests = Array.isArray(row.testIds) ? row.testIds.length : 0;

    if (linkedTests === 0 || row.status !== "complete") {
      continue;
    }

    for (const ac of row.acceptanceCriteriaIds || []) {
      if (!ids.includes(ac)) {
        ids.push(ac);
      }
    }
  }

  return ids;
}

// Which criteria are planned for. spec.json links requirements -> ACs, and tasks link
// back to requirements (`linkedRequirements` is required on every task), so that chain
// is the reliable route. Tasks' own `linkedAcceptanceCriteria` is optional in the
// schema, so relying on it alone would falsely warn about correctly planned features.
function collectPlannedAcceptanceCriteria(specJson, tasksJson) {
  const acsByRequirement = new Map();

  for (const group of ["functionalRequirements", "nonFunctionalRequirements"]) {
    for (const requirement of (specJson && specJson[group]) || []) {
      if (requirement && requirement.id) {
        acsByRequirement.set(requirement.id, requirement.linkedAcceptanceCriteria || []);
      }
    }
  }

  const planned = new Set();

  for (const phase of (tasksJson && tasksJson.phases) || []) {
    for (const task of phase.tasks || []) {
      for (const ac of task.linkedAcceptanceCriteria || []) {
        planned.add(ac);
      }

      for (const requirementId of task.linkedRequirements || []) {
        for (const ac of acsByRequirement.get(requirementId) || []) {
          planned.add(ac);
        }
      }
    }
  }

  return planned;
}

// Returns { ok, blockers, warnings, location, current, tasks }.
// `ok === false` means the gate must NOT be approved and nothing may be written.
function checkGateRequirements(root, feature, version, gate) {
  const blockers = [];
  const warnings = [];
  const gates = version.gates || {};
  const location = findSpecLocation(root, feature.name, version.id);
  const gov = path.join(root, ".sdrive", "governance", feature.name, version.id);
  const govFile = (name) => path.join(gov, name);

  if (!location) {
    blockers.push(
      `No spec folder for "${feature.name}" ${version.id} under .sdrive/specs/{${SPEC_STAGES.join(", ")}}/`
    );
  }

  if (location && location.stage === "completed") {
    blockers.push(
      `"${feature.name}" is archived in specs/completed/. Gates on a completed feature are history - run \`sdrive amend ${feature.name}\` first.`
    );
  }

  if (version.phase === "completed") {
    blockers.push(`Version "${version.id}" is already completed.`);
  }

  // Prerequisite gates must already be approved.
  const gateIndex = GATE_ORDER.indexOf(gate);

  for (let i = 0; i < gateIndex; i += 1) {
    const earlier = GATE_ORDER[i];

    if (!isGateApprovedFor(version, earlier)) {
      blockers.push(
        `Prerequisite gate "${earlier}" is "${gateStatusFor(version, earlier)}", not "approved".`
      );
    }
  }

  const requireSpecFile = (name, why) => {
    if (location && !fs.existsSync(path.join(location.dir, name))) {
      blockers.push(`Missing ${name} in the version folder (${why}).`);
    }
  };

  const requireGovFile = (name, why) => {
    if (!fs.existsSync(govFile(name))) {
      blockers.push(`Missing ${name} in .sdrive/governance/${feature.name}/${version.id}/ (${why}).`);
    }
  };

  let tasks = null;

  if (gate === "specification") {
    requireSpecFile("spec.md", "the specification being approved");
    requireGovFile("spec.json", "machine-readable specification");
  }

  if (gate === "planning") {
    requireSpecFile("plan.md", "the technical plan");
    requireSpecFile("tasks.md", "the task list");
    requireGovFile("plan.json", "machine-readable plan");
    requireGovFile("tasks.json", "machine-readable tasks");
    requireGovFile("traceability.json", "requirement-to-task traceability");

    // Every acceptance criterion has to be planned for, otherwise the review gate's
    // coverage requirement can never be met and the feature dead-ends at review.
    // Advisory rather than blocking: `linkedAcceptanceCriteria` is optional in the
    // tasks schema, so a plan that links only requirements is warned about, not rejected.
    const specJson = readJsonIfPresent(govFile("spec.json"));
    const tasksJson = readJsonIfPresent(govFile("tasks.json"));

    if (specJson && tasksJson) {
      const planned = collectPlannedAcceptanceCriteria(specJson, tasksJson);
      const unplanned = collectAcceptanceCriteria(specJson).filter((ac) => !planned.has(ac));

      if (unplanned.length > 0) {
        warnings.push(
          `${unplanned.length} acceptance criteria are not linked to any task (${unplanned
            .slice(0, 6)
            .join(", ")}${unplanned.length > 6 ? ", ..." : ""}). The review gate needs a passing test for every one of them - plan them explicitly.`
        );
      }
    }
  }

  if (gate === "implementation") {
    const tasksPath = govFile("tasks.json");

    if (!fs.existsSync(tasksPath)) {
      blockers.push("Missing tasks.json, so finished work cannot be confirmed.");
    } else {
      tasks = countTasks(readJsonIfPresent(tasksPath));

      if (tasks.total === 0) {
        blockers.push("tasks.json contains no tasks, so there is no work to certify.");
      } else if (tasks.done < tasks.total) {
        blockers.push(
          `${tasks.total - tasks.done} of ${tasks.total} tasks are still incomplete in tasks.json.`
        );
      }
    }

    const verificationPath = govFile("verification.json");

    if (fs.existsSync(verificationPath)) {
      const status = (readJsonIfPresent(verificationPath) || {}).status;

      if (status === "fail") {
        blockers.push(
          `verification.json reports status "fail" - run \`sdrive verify ${feature.name}\` after fixing it.`
        );
      } else if (status && status !== "pass" && status !== "warn") {
        warnings.push(`verification.json has an unrecognised status "${status}".`);
      }
    } else {
      warnings.push(`No verification.json yet - run \`sdrive verify ${feature.name}\` for a spec-to-code check.`);
    }
  }

  if (gate === "review") {
    const reviewPolicy = testEvidencePolicy(root);
    const reviewPath = location ? path.join(location.dir, "review.md") : null;

    if (!reviewPath || !fs.existsSync(reviewPath)) {
      blockers.push("Missing review.md in the version folder, so there is no verdict to approve.");
    } else if (!/Status:\s*PASS/i.test(fs.readFileSync(reviewPath, "utf8"))) {
      blockers.push(
        `review.md does not record \`Status: PASS\` - a failing or torn verdict cannot be approved.`
      );
    }

    // Structured review record. New projects require it; existing projects may
    // still approve a prose-only verdict until they opt in with
    // `sdrive config:set governance.requireStructuredReview true`.
    const reviewJson = readJsonIfPresent(govFile("review.json"));

    if (!reviewJson && reviewPolicy.structuredReview) {
      blockers.push(
        `Missing review.json for ${feature.name} ${version.id} - run /sdrive:review ${feature.name} to record the structured verdict.`
      );
    }

    if (reviewJson) {
      if (reviewJson.status !== "PASS") {
        blockers.push(
          `review.json records status "${reviewJson.status || "missing"}", not "PASS".`
        );
      }

      if (reviewJson.feature !== feature.name || reviewJson.version !== version.id) {
        blockers.push(
          `review.json belongs to "${reviewJson.feature || "unknown"}" ${reviewJson.version || "unknown"}, not "${feature.name}" ${version.id}.`
        );
      }

      if (!reviewJson.reviewedBy) {
        blockers.push("review.json does not record who performed the review.");
      }

      const head = headCommit(root);

      if (head && reviewJson.commit !== head) {
        blockers.push(
          `review.json is for commit ${String(reviewJson.commit || "unknown").slice(0, 8)}, HEAD is ${head.slice(0, 8)} - re-run the review.`
        );
      }

      if (feature.assignee && reviewJson.reviewedBy === feature.assignee) {
        warnings.push(
          `"${reviewJson.reviewedBy}" is both the assignee and the reviewer. Prefer an independent reviewer for the record.`
        );
      }
    }

    // Coverage is the whole point of this gate. Approving review is what unlocks
    // /sdrive:archive, and archive is what merges to the target branch and moves the
    // feature to completed/. An untested feature that reaches completed/ is already on
    // the target branch, so its tests can no longer gate anything - the window to
    // verify it has closed. That makes coverage a BLOCKER here, enforced in code.
    // It used to be asked for only in the review agent's prompt, and a prompt is not
    // a gate: a review could write `Status: PASS` at 0% coverage and be approved.
    const traceJson = readJsonIfPresent(govFile("traceability.json"));

    if (!traceJson) {
      blockers.push(
        "Missing or unreadable traceability.json, so test coverage cannot be verified."
      );
    } else {
      const allAcs = collectAcceptanceCriteria(readJsonIfPresent(govFile("spec.json")));
      const testedAcs = collectTestedAcceptanceCriteria(traceJson);
      const untested = allAcs.filter((ac) => !testedAcs.includes(ac));

      if (allAcs.length === 0) {
        warnings.push(
          "spec.json declares no acceptance criteria, so coverage could not be computed."
        );
      } else if (untested.length > 0) {
        blockers.push(
          `${untested.length} of ${allAcs.length} acceptance criteria have no passing test (${untested
            .slice(0, 6)
            .join(", ")}${untested.length > 6 ? ", ..." : ""}) - run \`/sdrive:test ${feature.name}\` first, then re-run the review.`
        );
      }
    }

    // Evidence is a fact, not a claim: `sdrive test` records the command, exit
    // code, counts, commit, and output hash. When the project opts in, a review
    // cannot be approved on hand-written JSON alone.
    const policy = reviewPolicy;

    if (policy.required) {
      const evidence = readJsonIfPresent(govFile("test-evidence.json"));

      if (!evidence) {
        blockers.push(
          `No test-evidence.json for ${feature.name} ${version.id} - run \`sdrive test ${feature.name}\` before approving review.`
        );
      } else {
        if (evidence.exitCode !== 0) {
          blockers.push(
            `test-evidence.json records a failing test run (exit ${evidence.exitCode}${evidence.timedOut ? ", timed out" : ""}).`
          );
        }

        if (evidence.summary && Number(evidence.summary.failed) > 0) {
          blockers.push(
            `test-evidence.json records ${evidence.summary.failed} failing test(s).`
          );
        }

        const noResults =
          (!evidence.summary || Number(evidence.summary.total) === 0) &&
          (!Array.isArray(evidence.testIds) || evidence.testIds.length === 0);

        if (noResults) {
          blockers.push(
            "test-evidence.json contains no recognisable test results - configure a reporter (JUnit/Jest/pytest/node:test output) or print TEST-### IDs."
          );
        }

        if (!policy.allowStale) {
          const head = headCommit(root);

          if (head && evidence.commit !== head) {
            blockers.push(
              `test-evidence.json is from commit ${String(evidence.commit || "unknown").slice(0, 8)}, HEAD is ${head.slice(0, 8)} - re-run \`sdrive test ${feature.name}\`.`
            );
          }
        }
      }
    }

    // The coverage report is a human-readable artifact. Its absence weakens the audit
    // trail, but traceability.json above is the machine-checkable source of truth, so
    // a missing report warns rather than blocks.
    const reportsDir = path.join(root, ".sdrive", "reports", "tests");
    const reportPrefix = `test-report-${feature.name}-${version.id}-`;

    if (!fs.existsSync(reportsDir)) {
      warnings.push("No .sdrive/reports/tests/ directory, so no test report was found.");
    } else if (!fs.readdirSync(reportsDir).some((f) => f.startsWith(reportPrefix))) {
      warnings.push(
        `No test report matching ${reportPrefix}<date>.md in .sdrive/reports/tests/.`
      );
    }
  }

  // Overwriting a decision is a deliberate act and is called out, never silent.
  const current = gates[gate];

  if (current === "rejected") {
    warnings.push(
      `Gate "${gate}" is currently "rejected". Approving overwrites that rejection.`
    );
  } else if (current === "in_progress") {
    warnings.push(`Gate "${gate}" is currently "in_progress".`);
  } else if (isGateApprovedFor(version, gate)) {
    warnings.push(`Gate "${gate}" is already approved.`);
  }

  // `approved` is returned so callers never have to re-test the stored string. `current`
  // keeps its original "undefined when unset" shape for existing readers.
  return {
    ok: blockers.length === 0,
    blockers,
    warnings,
    location,
    current,
    approved: isGateApprovedFor(version, gate),
    tasks
  };
}

module.exports = {
  getWorkflowState,
  saveWorkflowState,
  resolveVersion,
  findFeature,
  getFeature,
  GATE_APPROVED,
  gateStatusFor,
  isGateApprovedFor,
  allGatesApproved,
  gateStatus,
  approveGate,
  isGateApproved,
  GATE_ORDER,
  SPEC_STAGES,
  findSpecLocation,
  checkGateRequirements
};