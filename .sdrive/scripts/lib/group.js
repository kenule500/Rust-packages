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
const STATUS_WORDS = new Set(["backlog", "deferred", "ongoing", "completed", "needs-discovery"]);

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
  if (selector.kind === "status") return selector.value;
  if (selector.kind === "tag") return `@${selector.value}`;
  return selector.value;
}

function rowToCandidate(row) {
  return {
    id: row.id,
    title: row.feature || row.title || row.id,
    name: toKebab(row.feature || row.title || row.id),
    milestone: row.milestone || "",
    priority: row.priority || "",
    status: row.status || ""
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
          status: ""
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
    available: describeAvailable(backlog)
  };
}

// `feature/wave1-p2` from `wave1 p2`; the user can always override it at propose time.
function suggestBranchName(expression) {
  return toKebab(String(expression || "").replace(/,/g, " ")) || "wave";
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
  resolveGroup,
  suggestBranchName
};
