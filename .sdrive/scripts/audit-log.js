const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");
const { appendEvent } = require("./lib/audit-chain");

const projectRoot = process.cwd();
const auditLogPath = path.join(projectRoot, ".sdrive", "audit-log.json");

// Resolves the acting user so every audit event records WHO did it, not just
// what and when. Prefers the Git identity (the same source `check-permission.js`
// uses for role checks) and falls back to the OS username so events are never
// left anonymous.
function getActor() {
  if (process.env.SDRIVE_ACTOR) return process.env.SDRIVE_ACTOR.trim();

  try {
    const name = execSync("git config user.name", { encoding: "utf8", stdio: ["pipe", "pipe", "pipe"] }).trim();
    if (name) return name;
  } catch (error) {
    // Not a git repo, or git is unavailable — fall through to the OS user.
  }

  return process.env.USERNAME || process.env.USER || "unknown";
}

function loadAuditLog() {
  if (!fs.existsSync(auditLogPath)) {
    return { events: [] };
  }

  try {
    return JSON.parse(fs.readFileSync(auditLogPath, "utf8"));
  } catch (error) {
    console.error(`⚠️  Could not parse audit log: ${error.message}`);
    return { events: [] };
  }
}

function saveAuditLog(log) {
  fs.mkdirSync(path.dirname(auditLogPath), { recursive: true });

  const temp = `${auditLogPath}.${process.pid}.tmp`;
  fs.writeFileSync(temp, `${JSON.stringify(log, null, 2)}\n`);
  fs.renameSync(temp, auditLogPath);
}

function logEvent(event, details = {}) {
  const log = loadAuditLog();

  appendEvent(log, {
    timestamp: new Date().toISOString(),
    actor: getActor(),
    ...details,
    event
  });

  saveAuditLog(log);
  console.log(`📝 Audit: ${event}`);
}

function getRecentEvents(count = 10) {
  const log = loadAuditLog();
  return log.events.slice(-count).reverse();
}

module.exports = {
  logEvent,
  loadAuditLog,
  saveAuditLog,
  getRecentEvents,
  getActor
};
