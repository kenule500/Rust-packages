// Hash-chained audit log.
//
// The audit log is the record that approvals happened. A plain JSON array can be
// edited by anyone with write access, so each entry commits to the previous one:
// changing an old entry breaks every hash after it. This is tamper-evident, not
// tamper-proof; teams that need the latter can wrap the chain with an HMAC.
const crypto = require("crypto");

const GENESIS = "GENESIS";

function canonicalize(value) {
  if (Array.isArray(value)) return value.map(canonicalize);

  if (value && typeof value === "object") {
    const out = {};
    Object.keys(value)
      .sort()
      .forEach((key) => {
        if (value[key] !== undefined) out[key] = canonicalize(value[key]);
      });
    return out;
  }

  return value;
}

function sha256(text) {
  return crypto.createHash("sha256").update(text, "utf8").digest("hex");
}

function computeHash(prevHash, event) {
  const body = { ...event };
  delete body.hash;
  body.prevHash = prevHash;
  return sha256(JSON.stringify(canonicalize(body)));
}

function lastHash(events) {
  for (let index = events.length - 1; index >= 0; index -= 1) {
    if (events[index] && events[index].hash) return events[index].hash;
  }
  return GENESIS;
}

function appendEvent(log, event) {
  const events = Array.isArray(log.events) ? log.events : [];
  const prevHash = lastHash(events);
  const entry = { ...event, prevHash };
  entry.hash = computeHash(prevHash, entry);
  events.push(entry);
  log.events = events;
  return entry;
}

// Legacy logs have entries with no hash. They are reported, not treated as
// broken: the chain begins at the first hashed entry.
function verifyChain(log) {
  const events = Array.isArray(log.events) ? log.events : [];

  if (events.length === 0) {
    return { ok: true, checked: 0, legacy: 0, brokenAt: null, reason: "audit log is empty" };
  }

  let checked = 0;
  let legacy = 0;
  let prevHash = GENESIS;

  for (let index = 0; index < events.length; index += 1) {
    const entry = events[index] || {};

    if (!entry.hash) {
      legacy += 1;
      continue;
    }

    const expected = computeHash(prevHash, entry);

    if (entry.prevHash !== prevHash) {
      return {
        ok: false,
        checked,
        legacy,
        brokenAt: index,
        reason: `entry ${index} does not link to the previous hash`
      };
    }

    if (entry.hash !== expected) {
      return {
        ok: false,
        checked,
        legacy,
        brokenAt: index,
        reason: `entry ${index} has been modified (hash mismatch)`
      };
    }

    prevHash = entry.hash;
    checked += 1;
  }

  return {
    ok: true,
    checked,
    legacy,
    brokenAt: null,
    reason: `${checked} hashed event(s) verified${legacy ? `, ${legacy} legacy event(s) unhashed` : ""}`
  };
}

module.exports = {
  GENESIS,
  canonicalize,
  computeHash,
  appendEvent,
  verifyChain
};
