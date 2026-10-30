// Scope reduction: deferred and superseded work.
//
// The governance model records work as done or not done. Neither is true of work
// that was descoped - a provider that never published an API, a report nobody can
// query, a partner integration that never granted access. Deleting the record loses
// what was promised; marking it complete falsifies it. A `resolution` block is the
// third state, and it must state why.
//
// Shared by `commands/gates.js` (the implementation gate) and `scripts/verify.js`
// (the spec-to-code report) so the two cannot disagree about what "resolved" means.
// The same shape is used for tasks, plan components, plan entities, and API
// contracts; `schemas/*.schema.json` carries the long-form definition.

const RESOLUTION_KINDS = Object.freeze(["deferred", "superseded"]);

function reasonOf(item) {
  const resolution = item && item.resolution;

  if (!resolution || typeof resolution !== "object") return "";

  return String(resolution.reason || "").trim();
}

// Resolved means "explicitly descoped with a stated reason". A bare `{}` is not a
// resolution: honouring it would be a one-line way to skip any task, which is the
// bypass the gates exist to prevent. `sdrive validate` enforces the same rule with
// a minimum length; this checks presence, because the gates read the JSON directly
// and cannot assume the validator has run.
function isResolved(item) {
  return reasonOf(item).length > 0;
}

function resolutionOf(item) {
  const resolution = (item && item.resolution) || {};

  return {
    kind: RESOLUTION_KINDS.includes(resolution.kind) ? resolution.kind : "deferred",
    reason: reasonOf(item),
    at: String(resolution.at || "")
  };
}

// One sentence, so the implementation gate, the review gate, and `verify` describe
// the same list the same way. `label` is the caller's noun ("tasks", "plan items").
function describeResolved(items, label) {
  const names = items.map((item) => item.id || item.name || "(unnamed)");
  const shown = names.slice(0, 6).join(", ");
  const suffix = names.length > 6 ? ", ..." : "";

  return `${items.length} ${label} are descoped and excluded from completion (${shown}${suffix}). The reason for each is recorded in the governance files.`;
}

module.exports = {
  RESOLUTION_KINDS,
  reasonOf,
  isResolved,
  resolutionOf,
  describeResolved
};
