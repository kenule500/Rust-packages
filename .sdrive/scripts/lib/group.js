// Group expressions: `[wave1 p2]`, `[wave1]`, `[signup, multilanguage, video call]`.
//
// A bracketed argument to `/sdrive:propose` is not a feature name - it is a selection.
// Inside the brackets, comma separates items and space filters them:
//
//   [wave1 p2]                      → backlog items in milestone/wave 1 AND priority 2
//   [wave1]                         → everything in wave 1
//   [signup, multilanguage]         → two features by name
//   [wave1 p2, hotfix]              → the wave-1/P2 rows plus the feature `hotfix`
//
// The rule that makes both shapes work without ambiguity: a comma-separated segment is a
// selector QUERY only when every word in it is a recognised selector. Otherwise the whole
// segment is a feature name - which is why `video call` stays one name while `wave1 p2`
// is a query.
//
// Kept pure and dependency-free so the CLI, the agents, and the tests cannot disagree
// about what a group means. Nothing here reads the filesystem.

const MILESTONE_SELECTOR = /^(?:wave|w|m|milestone)(\d+)$/i;
const PRIORITY_SELECTOR = /^p(\d+)$/i;
const ID_SELECTOR = /^bp-(\d{3})$/i;
const BATCH_SELECTOR = /^b(\d+)$/i;
const STATUS_WORDS = new Set(["backlog", "deferred", "ongoing", "completed", "needs-discovery"]);
// What kind of work a backlog row is. A `decision` produces no test and a `release` step
// cannot run until the build lands, so neither can pass a coverage gate - excluding them at
// selection time is far cheaper than discovering it at archive time.
const KIND_WORDS = new Set(["build", "decision", "release"]);

// A row that cannot start without something from outside the repository: marked
// `needs discovery` / `deferred`, or its dependency cell names access that must be granted.
const GATED_STATUS = new Set(["needs-discovery", "deferred"]);
const GATED_DEPENDENCY = /access|discovery|partner|credential|onboarding|approval|agreement/i;

function toKebab(value) {
  return String(value == null ? "" : value)
    .toLowerCase()
    .trim()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "");
}

// One word → one selector, or null when the word is not a selector.
//
// `waveN` and `wN` select the DEPENDENCY-derived wave (what /sdrive:worktree calls a wave);
// `mN` and `milestoneN` select the Milestone column. They are different columns in the
// backlog and they do not always agree - in a product plan where M5 holds three crates the
// dependency wave may be 1 - so conflating them silently matched the wrong items.
function selectorFor(word) {
  const text = String(word || "").trim();
  if (!text) return null;

  const wave = text.match(/^(?:wave|w)(\d+)$/i);
  if (wave) return { kind: "wave", value: Number(wave[1]) };

  const milestone = text.match(/^(?:milestone|m)(\d+)$/i);
  if (milestone) return { kind: "milestone", value: Number(milestone[1]) };

  const priority = text.match(PRIORITY_SELECTOR);
  if (priority) return { kind: "priority", value: Number(priority[1]) };

  const id = text.match(ID_SELECTOR);
  if (id) return { kind: "id", value: `BP-${id[1]}` };

  const batch = text.match(BATCH_SELECTOR);
  if (batch) return { kind: "batch", value: `b${Number(batch[1])}` };

  const asKind = text.toLowerCase();
  if (KIND_WORDS.has(asKind)) return { kind: "kind", value: asKind };

  const status = text.toLowerCase().replace(/\s+/g, "-");
  if (STATUS_WORDS.has(status)) return { kind: "status", value: status };

  if (text.startsWith("@") && text.length > 1) {
    return { kind: "tag", value: text.slice(1).toLowerCase() };
  }

  return null;
}

// `[ ... ]` is a group; `[` is required on both ends to avoid guessing at half-syntax.
function parseExpression(raw) {
  const text = String(raw == null ? "" : raw).trim();
  const wrapped = /^\[(.*)\]$/.exec(text);

  if (!wrapped) return { isGroup: false, expression: null };

  return { isGroup: true, expression: wrapped[1].trim() };
}

function classifySegment(segment) {
  const words = String(segment || "")
    .trim()
    .split(/\s+/)
    .filter(Boolean);

  if (words.length === 0) return { kind: "empty", words };

  const selectors = words.map(selectorFor);

  // Every word must be a selector, otherwise the segment is a name. This is what keeps
  // `video call` a feature name and makes `wave1 p2` a query.
  if (selectors.every(Boolean)) {
    return { kind: "query", words, selectors };
  }

  return { kind: "name", words, name: words.join(" ") };
}

function splitSegments(expression) {
  return String(expression || "")
    .split(",")
    .map((segment) => segment.trim())
    .filter(Boolean);
}

function milestoneNumber(cell) {
  const match = String(cell || "").match(/(\d+)/);
  return match ? Number(match[1]) : null;
}

// The wave an item belongs to. `annotateBacklog()` already derives a numeric wave from the
// dependency graph (0 = "start", then 1, 2, ...), and `16-worktree.md` works from it, so a
// `waveN` selector uses that value when it is present and falls back to the milestone
// number only for a backlog that has not been annotated.
function waveValue(row) {
  if (row && row.wave !== undefined && row.wave !== null && String(row.wave).trim() !== "") {
    const wave = Number(row.wave);
    if (Number.isFinite(wave)) return wave;
  }

  return milestoneNumber(row && row.milestone);
}

function priorityNumber(cell) {
  const match = String(cell || "").match(/(\d+)/);
  return match ? Number(match[1]) : null;
}

function rowMatchesSelector(row, selector) {
  if (selector.kind === "wave") return waveValue(row) === selector.value;
  if (selector.kind === "milestone") return milestoneNumber(row.milestone) === selector.value;
  if (selector.kind === "priority") return priorityNumber(row.priority) === selector.value;
  if (selector.kind === "kind") {
    // A row with no `Kind` column is a build. That is the documented default, and it is what
    // keeps `[w2 p1 build]` usable on a backlog nobody has relabelled - otherwise the filter
    // would silently exclude every unlabelled row.
    const kind = toKebab(row.kind) || "build";

    return kind === selector.value;
  }
  if (selector.kind === "batch") return toKebab(row.batch) === selector.value;
  if (selector.kind === "id") return String(row.id).toUpperCase() === selector.value;
  if (selector.kind === "status") return toKebab(row.status) === selector.value;

  if (selector.kind === "tag") {
    const tags = Array.isArray(row.tags) ? row.tags.map(toKebab) : [];
    return tags.includes(selector.value);
  }

  return false;
}

function selectorLabel(selector) {
  if (selector.kind === "wave") return `wave${selector.value}`;
  if (selector.kind === "milestone") return `m${selector.value}`;
  if (selector.kind === "priority") return `p${selector.value}`;
  if (selector.kind === "kind") return selector.value;
  if (selector.kind === "batch") return selector.value;
  if (selector.kind === "status") return selector.value;
  if (selector.kind === "tag") return `@${selector.value}`;
  return selector.value;
}

// A row whose work cannot start without something from outside the repository. Two cheap
// signals: the plan status says so, or the dependency cell names access that must be
// granted. A shared branch is atomic, so one gated member holds the whole group - the
// selection should say so before a single spec exists.
function isGated(row) {
  if (!row) return false;
  if (row.gated === true) return true;
  if (GATED_STATUS.has(toKebab(row.status))) return true;

  // `parseBacklog` splits a dependency cell into ids (`dependsOn`) and outside blockers
  // (`externalDependencies`), and the blocker - "partner access" - is the actual signal.
  // An `??` chain would stop at the non-empty id array and never look at it.
  const dependencyText = [
    ...(Array.isArray(row.dependsOn) ? row.dependsOn : [row.dependsOn]),
    ...(Array.isArray(row.externalDependencies)
      ? row.externalDependencies
      : [row.externalDependencies]),
    row.depends,
    row.dependencies,
    row.external
  ]
    .filter((value) => value !== undefined && value !== null && value !== "")
    .join(" ");

  return GATED_DEPENDENCY.test(dependencyText);
}

function rowToCandidate(row) {
  return {
    id: row.id,
    title: row.feature || row.title || row.id,
    name: toKebab(row.feature || row.title || row.id),
    milestone: row.milestone || "",
    priority: row.priority || "",
    status: row.status || "",
    kind: row.kind || "",
    batch: toKebab(row.batch),
    // Carried so a declared batch can be checked for "all members in one wave" - members
    // from different waves can never be ready at the same time.
    wave: waveValue(row),
    gated: isGated(row)
  };
}

// Summarises what the backlog can be selected by, so an empty result can say what exists
// instead of only that nothing matched. Waves and milestones are reported separately
// because they are different columns.
function describeAvailable(backlog) {
  const waves = new Set();
  const milestones = new Set();
  const priorities = new Set();
  const statuses = new Set();

  for (const row of backlog || []) {
    const wave = waveValue(row);
    if (wave !== null) waves.add(wave);
    const milestone = milestoneNumber(row.milestone);
    if (milestone !== null) milestones.add(milestone);
    const priority = priorityNumber(row.priority);
    if (priority !== null) priorities.add(priority);
    if (row.status) statuses.add(toKebab(row.status));
  }

  return {
    waves: [...waves].sort((a, b) => a - b),
    milestones: [...milestones].sort((a, b) => a - b),
    priorities: [...priorities].sort((a, b) => a - b),
    statuses: [...statuses].sort()
  };
}

// The batch an expression names, when it names exactly one. `[b1]` alone is a declared
// promise, so it implies a single branch; `[b1, signup]` is a plain selection and the
// branch question is asked as usual.
function singleBatch(expression) {
  const segments = splitSegments(expression);

  if (segments.length !== 1) return null;

  const classified = classifySegment(segments[0]);

  if (classified.kind !== "query" || classified.selectors.length !== 1) return null;

  const [selector] = classified.selectors;

  return selector.kind === "batch" ? selector.value : null;
}

// Resolves a group expression against parsed backlog rows (product-plan.parseBacklog).
//
// Every segment contributes candidates; duplicates collapse by BP id (or by name when a
// segment names a feature that is not in the backlog). A query that matches nothing is
// reported in `emptyQueries` rather than being silently dropped, because "wave 3" with no
// items is different from "wave 3 does not exist".
function resolveGroup({ expression, backlog }) {
  const segments = splitSegments(expression);
  const matched = [];
  const names = [];
  const emptyQueries = [];
  const seen = new Set();

  const addCandidate = (candidate) => {
    const key = candidate.id || candidate.name;
    if (seen.has(key)) return;
    seen.add(key);
    matched.push(candidate);
  };

  for (const segment of segments) {
    const classified = classifySegment(segment);

    if (classified.kind === "empty") continue;

    if (classified.kind === "name") {
      const name = toKebab(classified.name);
      const row = (backlog || []).find(
        (entry) => toKebab(entry.feature || entry.title) === name
      );

      names.push(name);

      if (row) addCandidate(rowToCandidate(row));
      else {
        addCandidate({
          id: null,
          title: classified.name,
          name,
          milestone: "",
          priority: "",
          status: "",
          kind: "",
          batch: "",
          wave: null,
          // A name with no backlog row has no declared dependency, so it cannot be flagged
          // as gated - the user is naming it directly, which is already a decision.
          gated: false
        });
      }

      continue;
    }

    const hits = (backlog || []).filter((row) =>
      classified.selectors.every((selector) => rowMatchesSelector(row, selector))
    );

    if (hits.length === 0) {
      emptyQueries.push(classified.selectors.map(selectorLabel).join(" "));
      continue;
    }

    hits.forEach((row) => addCandidate(rowToCandidate(row)));
  }

  return {
    expression,
    segments: segments.length,
    matched,
    names,
    emptyQueries,
    // Set when the expression named exactly one batch: a declaration, not a preference.
    batch: singleBatch(expression),
    // The subset a shared branch cannot afford: one gated member holds the whole group,
    // because the branch merges only when every member passes review.
    gated: matched.filter((item) => item.gated),
    available: describeAvailable(backlog)
  };
}

// Answers the question the branch decision actually asks: can these features share ONE
// branch? A shared branch is atomic (CON-611), so a single blocked member stalls every
// other member. `allowGated` is the explicit opt-out for a user who knows the access is
// coming; the size warning is advisory because 5+ members is a judgement call, not a
// correctness problem.
function checkGroupShareable(resolved, options = {}) {
  const { allowGated = false, maxFeatures = 4 } = options;
  const matched = (resolved && resolved.matched) || [];
  const blockers = [];
  const warnings = [];

  if (!allowGated) {
    const gated = matched.filter((item) => item.gated);

    if (gated.length > 0) {
      blockers.push(
        `${gated.length} of ${matched.length} selected item(s) cannot start without access from outside the repository (${gated
          .map((item) => item.id || item.name)
          .join(", ")}). A shared branch merges only when every member passes review, so these would hold the whole group. Use one branch each, exclude them with --exclude-gated, or confirm the access first.`
      );
    }
  }

  if (matched.length > maxFeatures) {
    warnings.push(
      `${matched.length} features on one branch means ${matched.length * 2} approval stops, and one failure freezes all of them. Consider splitting the wave.`
    );
  }

  return { ok: blockers.length === 0, blockers, warnings };
}

// A declared batch is a promise, so its invariants are refusals rather than warnings: a
// batch that cannot finish should be fixed in the backlog, not discovered at archive time.
//   - every member is a `build`   (a decision has no test; a release postdates the merge)
//   - no member is gated          (the branch merges only when every member is ready)
//   - every member is in one wave (members from different waves cannot be ready together)
function checkBatch(resolved, options = {}) {
  const { maxFeatures = 4 } = options;
  const matched = (resolved && resolved.matched) || [];
  const blockers = [];
  const warnings = [];

  if (matched.length === 0) {
    return { ok: false, blockers: ["The batch selects no items."], warnings };
  }

  const gated = matched.filter((item) => item.gated);

  if (gated.length > 0) {
    blockers.push(
      `${gated.length} member(s) need access from outside the repository (${gated
        .map((item) => item.id || item.name)
        .join(", ")}). A batch lands atomically, so the whole branch waits on them.`
    );
  }

  const notBuild = matched.filter((item) => item.kind && item.kind !== "build");

  if (notBuild.length > 0) {
    blockers.push(
      `${notBuild.length} member(s) are not builds (${notBuild
        .map((item) => `${item.id || item.name}: ${item.kind}`)
        .join(", ")}). A decision has no test to pass and a release cannot run before the batch lands, so neither can clear review.`
    );
  }

  const waves = [...new Set(matched.map((item) => item.wave))].filter(
    (wave) => wave !== null && wave !== undefined
  );

  if (waves.length > 1) {
    blockers.push(
      `Members span ${waves.length} waves (${waves
        .map((wave) => `w${wave}`)
        .join(", ")}). A batch must be one wave, or its members can never all be ready at once.`
    );
  }

  if (matched.length > maxFeatures) {
    warnings.push(
      `${matched.length} members means ${matched.length * 2} approval stops, and one failure freezes all of them.`
    );
  }

  return { ok: blockers.length === 0, blockers, warnings };
}

// Which branch topologies can actually finish?
//
// The resolver used to hand the agent a set and a verdict, and the agent then invented
// options — which is how a deadlocked topology ("one shared branch for all four") got
// offered as a choice. This returns only what can reach archive:
//
//   1. a non-build member cannot pass a review gate in ANY topology, so it is never a
//      feature: it is excluded, with the reason.
//   2. a gated member (access outside the repository) makes a shared branch unarchivable
//      until that access exists, so it is only offered on its own branch.
//   3. everything else may share a branch, which merges once when every member is ready.
function topologies(resolved) {
  const matched = (resolved && resolved.matched) || [];
  const builds = matched.filter((item) => !item.kind || item.kind === "build");
  const nonBuilds = matched.filter((item) => item.kind && item.kind !== "build");
  const gated = builds.filter((item) => item.gated);
  const clean = builds.filter((item) => !item.gated);

  const names = (items) => items.map((item) => item.id || item.name).join(", ");
  const options = [];

  if (clean.length >= 2) {
    options.push({
      id: "shared-clean",
      ok: true,
      label:
        `One shared branch for ${clean.length} (${names(clean)})` +
        (gated.length > 0 ? `, and a separate branch for ${names(gated)}` : ""),
      shared: clean.map((item) => item.id || item.name),
      separate: gated.map((item) => item.id || item.name),
      note: "The shared branch merges once, when every member passes review."
    });
  }

  if (builds.length > 1) {
    options.push({
      id: "shared-all-builds",
      ok: gated.length === 0,
      label: `One shared branch for all ${builds.length} builds (${names(builds)})`,
      reasons:
        gated.length > 0
          ? [
              `${gated.length} gated member(s) (${names(
                gated
              )}) would hold the merge until that access is granted.`
            ]
          : [],
      shared: builds.map((item) => item.id || item.name),
      separate: [],
      note: "The shared branch merges once, when every member passes review."
    });
  }

  if (builds.length > 1) {
    options.push({
      id: "per-feature",
      ok: true,
      label: `One branch per item (${names(builds)})`,
      reasons: [],
      shared: [],
      separate: builds.map((item) => item.id || item.name),
      note: "Independent merges: a stalled item blocks nobody."
    });
  }

  return {
    options,
    // Members for which no topology helps, stated rather than quietly dropped.
    excluded: nonBuilds.map((item) => ({
      item: item.id || item.name,
      kind: item.kind,
      reason:
        item.kind === "decision"
          ? "A decision has no test to pass, so it cannot clear review. Record it in the plan, or give it testable criteria (a decision record that must exist and name the choice)."
          : "A release step runs after the merge, so it cannot be gated by the merge it belongs to."
    }))
  };
}

// The branch-topology question `/sdrive:worktree` asks, answered for the set of items that
// are ready RIGHT NOW rather than for an expression that is already chosen.
//
// `topologies()` returns what a selection can do; this returns what a ready set can do, and it
// is deliberately narrow. Only groupings a user can actually name are offered:
//
//   1. every declared batch inside the set (a `Batch` column is a promise, so it is listed first)
//   2. the whole set on one branch, when it is small enough to merge (`maxFeatures`)
//   3. one branch per item - always valid, listed last as the safe fallback
//
// Arbitrary sub-sets are NOT generated: with six ready items there are 57 non-trivial ones,
// which is a menu nobody reads, and the user can name any sub-set exactly with `--items`.
// A grouping that cannot reach archive is never offered as a choice - it is reported in
// `rejected` with the reason, so the agent can explain why the obvious option is missing
// instead of inventing one.
function branchOptions(candidates, options = {}) {
  const { maxFeatures = 4 } = options;
  const matched = (candidates || []).filter(Boolean);
  const idOf = (item) => item.id || item.name;
  const names = (items) => items.map(idOf).join(", ");
  const offered = [];
  const rejected = [];

  // A non-build cannot pass a review gate in ANY topology, so it is never a member. The
  // wording comes from `topologies()` so the two callers cannot describe the same rule
  // differently.
  const { excluded } = topologies({ matched });
  const excludedIds = new Set(excluded.map((entry) => entry.item));
  const builds = matched.filter((item) => !excludedIds.has(idOf(item)));

  const consider = (id, label, members, note) => {
    const verdict = checkBatch({ matched: members }, { maxFeatures });
    const entry = {
      id,
      ok: verdict.ok,
      label,
      shared: members.map(idOf),
      separate: [],
      note: verdict.ok ? note : "",
      reasons: verdict.blockers,
      warnings: verdict.warnings
    };

    if (verdict.ok) offered.push(entry);
    else rejected.push(entry);
  };

  const batches = new Map();
  for (const item of builds) {
    if (!item.batch) continue;
    const members = batches.get(item.batch) || [];
    members.push(item);
    batches.set(item.batch, members);
  }

  for (const [batch, members] of batches) {
    if (members.length < 2) continue;
    consider(
      `batch:${batch}`,
      `${batch} on one branch (${names(members)})`,
      members,
      "The backlog already declares these as one batch, so they land together."
    );
  }

  if (builds.length > 1 && builds.length <= maxFeatures) {
    consider(
      "shared-all",
      `One branch for all ${builds.length} ready items (${names(builds)})`,
      builds,
      "Merges once, when every member passes review."
    );
  } else if (builds.length > maxFeatures) {
    // Stated rather than omitted: "why can't I put all of them on one branch" deserves an
    // answer, and the answer is the cap, not the agent's mood.
    rejected.push({
      id: "shared-all",
      ok: false,
      label: `One branch for all ${builds.length} ready items (${names(builds)})`,
      shared: builds.map(idOf),
      separate: [],
      note: "",
      reasons: [
        `${builds.length} features is over the ${maxFeatures}-feature cap for one branch: ${builds.length * 2} approval stops, and one failure freezes all of them.`
      ],
      warnings: []
    });
  }

  if (builds.length > 1) {
    offered.push({
      id: "per-feature",
      ok: true,
      label: `One branch per item (${builds.length} branches)`,
      shared: [],
      separate: builds.map(idOf),
      note: "Independent merges: a stalled item blocks nobody.",
      reasons: [],
      warnings: []
    });
  }

  return { options: offered, rejected, excluded, maxFeatures };
}


// `feature/wave1-p2` from `wave1 p2`; the user can always override it at propose time.
// Segments are deduped: `[BP-011, BP-011, BP-017]` is the same set as `[BP-011, BP-017]`, and
// the suggestion should not advertise a duplicate.
function suggestBranchName(expression) {
  const unique = [...new Set(splitSegments(expression))];

  return toKebab(unique.join(" ").replace(/,/g, " ")) || "wave";
}

module.exports = {
  toKebab,
  selectorFor,
  selectorLabel,
  parseExpression,
  classifySegment,
  splitSegments,
  rowMatchesSelector,
  rowToCandidate,
  describeAvailable,
  waveValue,
  isGated,
  resolveGroup,
  checkGroupShareable,
  checkBatch,
  singleBatch,
  topologies,
  branchOptions,
  suggestBranchName
};
