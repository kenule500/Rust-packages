#!/usr/bin/env node

const http = require("http");
const fs = require("fs");
const os = require("os");
const path = require("path");
const { execFile } = require("child_process");

let projectRoot = process.cwd();
let sdriveRoot = path.join(projectRoot, ".sdrive");

// git runs asynchronously on purpose. execFileSync blocks the event loop, and this
// machine spawns git in 1-2 seconds, so building the header froze the entire server -
// it could not serve the HTML, an image, or another API call until every probe finished.
//
// The memo keeps a single page load from paying for the same answer repeatedly (the
// branch is asked for three times, origin/HEAD twice) and shares one in-flight spawn
// between callers instead of racing duplicates.
// A single git spawn can cost several seconds on a machine with an aggressive virus
// scanner, so the ceiling is set well above that. Too tight a limit does not make the
// dashboard faster - it makes it discard real answers and show less.
const GIT_TIMEOUT_MS = 20000;
const GIT_CACHE_TTL_MS = 10000;
const gitCache = new Map();

// Resolves to { ok, out }.
//
// A failed probe is NOT an empty answer. Resolving "" on error made a timed-out
// `git status` indistinguishable from a clean tree, and because the empty answer was
// cached, the dashboard confidently reported "Clean" while 76 files were modified.
// The caller now gets to tell those two facts apart.
function runGit(args) {
  return new Promise((resolve) => {
    execFile(
      "git",
      args,
      { cwd: projectRoot, encoding: "utf8", timeout: GIT_TIMEOUT_MS, windowsHide: true },
      (error, stdout) => resolve({ ok: !error, out: error ? "" : String(stdout).trim() })
    );
  });
}

// Cached probe. Successful answers are reused for a few seconds; failures are returned
// to the caller but dropped from the memo straight away, so one slow spawn cannot
// poison every request for the next ten seconds.
async function gitProbe(args) {
  const key = args.join("\u0000");
  const cached = gitCache.get(key);

  if (cached && Date.now() - cached.at < GIT_CACHE_TTL_MS) {
    return cached.promise;
  }

  const promise = runGit(args);
  gitCache.set(key, { at: Date.now(), promise });
  promise.then((result) => {
    if (!result.ok) gitCache.delete(key);
  });
  return promise;
}

// String form, for the many probes where "nothing to report" and "failed" lead to the
// same place. Use gitProbe directly wherever the difference changes what the UI says.
async function git(args) {
  return (await gitProbe(args)).out;
}

async function apiGitInfo() {
  // Everything here except the branching read-out is independent of the branch name,
  // so all five probes go out together. Asking for the branch first and the rest
  // afterwards cost a whole extra round of slow spawns for no extra information.
  const [branchProbe, logLine, statusProbe, upstream, counts] = await Promise.all([
    gitProbe(["rev-parse", "--abbrev-ref", "HEAD"]),
    git(["log", "-1", "--pretty=%h%x1f%s%x1f%an%x1f%cI"]),
    gitProbe(["status", "--porcelain"]),
    git(["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{upstream}"]),
    git(["rev-list", "--left-right", "--count", "@{upstream}...HEAD"])
  ]);

  const branch = branchProbe.out;
  if (!branch) {
    return { available: false, branching: await apiBranching() };
  }

  const [commit = "", subject = "", author = "", committedAt = ""] = logLine.split("\u001f");

  // null means "git could not be asked", which is not the same as false ("clean").
  const dirty = statusProbe.ok ? statusProbe.out.length > 0 : null;

  // Determine the branch this one was merged from / branched off.
  // Prefer the upstream tracking branch, then the default branch, then the
  // most recent merged-in branch recorded in the reflog.
  let mergedFrom = "";
  let ahead = 0;
  let behind = 0;

  if (upstream && upstream !== branch) {
    mergedFrom = upstream.replace(/^origin\//, "");
  }

  if (upstream) {
    // --left-right gives "<upstream-only> <HEAD-only>", i.e. behind then ahead.
    const [left, right] = counts.split(/\s+/);
    behind = parseInt(left, 10) || 0;
    ahead = parseInt(right, 10) || 0;
  }

  const branching = await apiBranching({ currentBranch: branch });

  if (!mergedFrom) {
    const defaultBranch = branching.defaultBranch;
    if (defaultBranch && defaultBranch !== branch) {
      mergedFrom = defaultBranch;
    }
  }

  if (!mergedFrom) {
    const reflog = await git(["reflog", "--grep-reflog=merge", "--pretty=%gs", "-n", "1"]);
    const match = reflog.match(/merge\s+([^:]+):/);
    if (match) {
      mergedFrom = match[1].trim();
    }
  }

  return {
    available: true,
    branch,
    commit,
    subject,
    author,
    committedAt,
    dirty,
    mergedFrom: mergedFrom || null,
    ahead,
    behind,
    branching
  };
}

// Which integration branch features land on, and the branching model that implies.
//
// This is DERIVED, never stored. The default branch is read from the repository and
// each landing target is read from the `base` recorded on that feature in
// workflow-state.json. There is no config field and no setting to change, so showing
// this cannot alter behaviour: it is a read-out of the setup a project already has,
// not a switch that enables one.
//
// Two rules keep the label honest:
//   1. The CURRENT branch is never used as the integration branch. Sitting on a
//      feature branch is the normal state of a developer checkout, and treating it
//      as "where features land" would invert the meaning of the read-out.
//   2. A base that is another feature's branch is a STACK, not an integration branch,
//      so it is reported separately rather than counted as a landing target.
async function resolveDefaultBranch() {
  const originHead = (await git(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"]) || "")
    .replace(/^origin\//, "");
  if (originHead && originHead !== "HEAD") {
    return { name: originHead, source: "origin/HEAD" };
  }

  // origin/HEAD is only populated once the remote is known and `remote set-head` has
  // run, so fall back to what the remote actually has, then to a local conventional
  // name. `develop` is only accepted from the remote, where its existence is a real
  // signal rather than an ambiguous local coincidence.
  //
  // One for-each-ref instead of a rev-parse probe per candidate: seven probes is seven
  // process launches for one answer.
  const refs = await git([
    "for-each-ref",
    "--format=%(refname:short)",
    "refs/remotes/origin",
    "refs/heads"
  ]);
  const known = new Set(refs.split("\n").map((line) => line.trim()).filter(Boolean));

  for (const candidate of ["main", "master", "trunk", "develop"]) {
    if (known.has(`origin/${candidate}`)) {
      return { name: candidate, source: `origin/${candidate}` };
    }
  }

  for (const candidate of ["main", "master", "trunk"]) {
    if (known.has(candidate)) {
      return { name: candidate, source: `local ${candidate}` };
    }
  }

  return { name: null, source: null };
}

async function apiBranching(context = {}) {
  // Resolving the default branch and detecting the current one touch no shared
  // state, so both go out together rather than one after the other.
  const [resolved, detectedBranch] = await Promise.all([
    context.defaultBranch
      ? { name: context.defaultBranch, source: context.defaultBranchSource || null }
      : resolveDefaultBranch(),
    context.currentBranch !== undefined
      ? context.currentBranch
      : git(["rev-parse", "--abbrev-ref", "HEAD"])
  ]);

  const { name: defaultBranch, source: defaultBranchSource } = resolved;
  const currentBranch = detectedBranch || null;

  const state = loadJson(path.join(sdriveRoot, "workflow-state.json"), { features: [] });
  const features = Array.isArray(state.features) ? state.features : [];

  // A feature's branch is a stack parent when a peer points its base at it.
  const featureBranches = new Set(
    features.map((f) => (typeof f.branch === "string" ? f.branch.trim() : "")).filter(Boolean)
  );

  const bases = [];
  const integration = [];
  const stacked = [];
  let stackedCount = 0;
  let unrecordedCount = 0;

  for (const feature of features) {
    const base = typeof feature.base === "string" ? feature.base.trim() : "";
    if (!base) {
      unrecordedCount += 1;
      continue;
    }
    if (!bases.includes(base)) bases.push(base);

    if (featureBranches.has(base)) {
      stackedCount += 1;
      if (!stacked.includes(base)) stacked.push(base);
    } else if (!integration.includes(base)) {
      integration.push(base);
    }
  }

  const { label, detail } = describeBranchingModel({
    defaultBranch, integration, stackedCount, unrecordedCount,
    hasFeatureBranches: featureBranches.size > 0
  });

  return {
    defaultBranch,
    defaultBranchSource,
    currentBranch,
    bases,
    integration,
    stacked,
    stackedCount,
    unrecordedCount,
    label,
    detail
  };
}

// Exactly ONE model is named. Two models in one label read as if a project were
// running both at once, which is never true.
function describeBranchingModel(info) {
  const { defaultBranch, integration, stackedCount, unrecordedCount, hasFeatureBranches } = info;

  const stackNote = stackedCount
    ? ` ${stackedCount} feature(s) are stacked on a peer branch rather than landing on an integration branch directly.`
    : "";
  const pendingNote = unrecordedCount
    ? ` ${unrecordedCount} feature(s) have not started and have recorded no base yet.`
    : "";

  if (!defaultBranch) {
    return {
      label: "Unknown",
      detail: "No default branch could be resolved from this repository." + stackNote
    };
  }

  if (integration.length === 0) {
    if (stackedCount > 0) {
      return {
        label: "Stacked features",
        detail:
          `Every started feature is based on a peer's branch rather than on ${defaultBranch}. The chain lands ` +
          `on ${defaultBranch} once the bottom of the stack lands.`
      };
    }
    return {
      label: "Not started",
      detail: `The default branch is ${defaultBranch}, but no started feature has recorded a landing target yet.`
    };
  }

  if (integration.length > 1) {
    return {
      label: "Per-feature targets",
      detail:
        `Started features land on more than one branch (${integration.join(", ")}). SpecDrive reads each ` +
        `feature's target from its own recorded base, so there is no single integration branch.` + stackNote + pendingNote
    };
  }

  const only = integration[0];

  if (only === "develop") {
    return {
      label: "Git Flow",
      detail: `Started features land on ${only}; releases go to ${defaultBranch}.` + stackNote
    };
  }

  if (only !== defaultBranch) {
    return {
      label: "Custom",
      detail: `Started features land on ${only}, which is not the default branch (${defaultBranch}).` + stackNote
    };
  }

  if (!hasFeatureBranches) {
    return {
      label: "Trunk-based",
      detail:
        `Started features land on ${defaultBranch} and no feature has recorded a branch of its own, so work ` +
        `appears to go straight to the trunk. If this project predates branch recording, the branches may ` +
        `exist without having been recorded.` + stackNote
    };
  }

  return {
    label: "GitHub Flow",
    detail:
      `Started features land on the default branch ${defaultBranch}, one branch per feature. Trunk-based ` +
      `development with short-lived branches resolves to this identical shape, so the branch name is the ` +
      `reliable fact and GitHub Flow is reported whenever features have branches of their own - which is ` +
      `also the shape SpecDrive enforces (CON-606).` + stackNote
  };
}

// List every local branch with its tip commit, author, and merge status so the
// Team & Branch page can show the full branch landscape alongside members.
async function apiBranches() {
  // The current branch, the branch list, and origin/HEAD are three unrelated reads, so
  // they are issued together. Only the merge check has to wait, because it needs the
  // default branch name first.
  const [current, raw, originHead] = await Promise.all([
    git(["rev-parse", "--abbrev-ref", "HEAD"]),
    git([
      "for-each-ref",
      "--sort=-committerdate",
      "--format=%(refname:short)%09%(objectname:short)%09%(committerdate:iso-strict)%09%(authorname)%09%(subject)",
      "refs/heads"
    ]),
    git(["symbolic-ref", "--short", "refs/remotes/origin/HEAD"])
  ]);

  if (!raw) {
    return { available: false, current: current || null, branches: [] };
  }

  // Branches already merged into the default branch (main/master).
  const defaultBranch = (originHead || "").replace(/^origin\//, "") || "main";
  const mergedRaw = await git(["branch", "--merged", defaultBranch, "--format=%(refname:short)"]);
  const mergedSet = new Set(
    mergedRaw.split("\n").map((b) => b.trim()).filter(Boolean)
  );

  const branches = raw
    .split("\n")
    .map((line) => line.split("\t"))
    .filter((parts) => parts.length >= 5 && parts[0])
    .map(([name, commit, committedAt, author, subject]) => ({
      name,
      commit,
      committedAt,
      author,
      subject,
      current: name === current,
      merged: mergedSet.has(name)
    }));

  return {
    available: true,
    current: current || null,
    defaultBranch,
    branches
  };
}

function loadJson(filePath, fallback = null) {
  if (!fs.existsSync(filePath)) return fallback;
  try {
    return JSON.parse(fs.readFileSync(filePath, "utf8"));
  } catch (error) {
    return fallback;
  }
}

// The dashboard is launched from the project root, so the folder name is the
// project name (e.g. "allymagax"). Surfaced in the sidebar and topbar so it is
// obvious which project a dashboard belongs to when several are running.
function apiProjectInfo() {
  const name = path.basename(projectRoot) || "project";
  return { name, root: projectRoot };
}

// Classify the project so the dashboard can tell the user which workflow they
// are on. Greenfield = built from scratch with SpecDrive; brownfield = an
// existing codebase adopted via onboarding. We prefer the explicit onboarding
// record, then fall back to heuristics (existing source + no onboarding record
// means it was almost certainly adopted).
function apiProductType() {
  const state = loadJson(path.join(sdriveRoot, "workflow-state.json"), {});
  const onboarding = state.onboarding || null;

  const hasConstitution = fs.existsSync(path.join(sdriveRoot, "constitution.md"));
  const hasProductPlan = fs.existsSync(path.join(sdriveRoot, "product", "overview.md"));
  const featureCount = Array.isArray(state.features) ? state.features.length : 0;

  // Heuristic: a project with source code but no onboarding record predates
  // SpecDrive, i.e. it was adopted (brownfield).
  const sourceDirs = ["app", "src", "components", "lib", "pages", "server"];
  const hasSource = sourceDirs.some((dir) => fs.existsSync(path.join(projectRoot, dir)));

  let type = "unknown";
  let source = "heuristic";

  if (onboarding && onboarding.mode) {
    type = onboarding.mode;
    source = "onboarding record";
  } else if (hasSource) {
    type = "brownfield";
  } else {
    type = "greenfield";
  }

  const meta = {
    greenfield: {
      label: "Greenfield",
      tagline: "Built from scratch",
      description:
        "A brand-new project started inside SpecDrive. Every feature is specified before it is built, so the spec and the code stay in lockstep from day one.",
      icon: "✦"
    },
    brownfield: {
      label: "Brownfield",
      tagline: "Adopted existing code",
      description:
        "An existing codebase brought under SpecDrive governance. Existing work is inventoried, retroactive specs are generated, and gaps are marked as deferred instead of breaking anything.",
      icon: "◈"
    },
    unknown: {
      label: "Unclassified",
      tagline: "Not yet determined",
      description:
        "SpecDrive has not recorded how this project started. Run onboarding or the constitution step to classify it.",
      icon: "○"
    }
  }[type];

  return {
    type,
    source,
    label: meta.label,
    tagline: meta.tagline,
    description: meta.description,
    icon: meta.icon,
    signals: {
      onboarding: Boolean(onboarding),
      onboardingDate: onboarding?.date || null,
      constitution: hasConstitution,
      productPlan: hasProductPlan,
      features: featureCount,
      existingSource: hasSource
    }
  };
}

function apiWorkflowState() {
  return loadJson(path.join(sdriveRoot, "workflow-state.json"), { features: [] });
}

function apiAuditLog() {
  return loadJson(path.join(sdriveRoot, "audit-log.json"), { events: [] });
}

// The Activity page is fed from two sources:
//   1. .sdrive/audit-log.json  (explicit governance events, if the project has one)
//   2. workflow-state.json history (every feature/version records its own timeline)
// Most projects never create audit-log.json, so without the second source the
// Activity page would always be empty. We merge both, newest first.
function apiActivity() {
  const events = [];

  const log = loadJson(path.join(sdriveRoot, "audit-log.json"), { events: [] });
  (log.events || []).forEach((e) => {
    events.push({
      event: e.event || "event",
      timestamp: e.timestamp || null,
      actor: e.actor || "",
      feature: e.feature || "",
      version: e.version || "",
      details: e.details || {},
      source: "audit-log"
    });
  });

  const state = loadJson(path.join(sdriveRoot, "workflow-state.json"), { features: [] });
  (state.features || []).forEach((feature) => {
    (feature.versions || []).forEach((version) => {
      (version.history || []).forEach((h) => {
        events.push({
          event: h.event || "event",
          timestamp: h.timestamp || null,
          actor: h.actor || "",
          feature: feature.name || "",
          version: version.id || "",
          details: {},
          source: "workflow-state"
        });
      });
    });
  });

  events.sort((a, b) => {
    const ta = a.timestamp ? Date.parse(a.timestamp) : 0;
    const tb = b.timestamp ? Date.parse(b.timestamp) : 0;
    return tb - ta;
  });

  return { events };
}

// Skills live at .sdrive/skills/<name>/SKILL.md with YAML frontmatter.
// We parse the frontmatter (name, description, version, tags, source, license,
// last_updated) plus a rough size/line count so the dashboard can show what
// guidance the project has provisioned.
function parseFrontmatter(content) {
  const match = content.match(/^---\r?\n([\s\S]*?)\r?\n---/);
  if (!match) return {};

  const meta = {};
  for (const line of match[1].split(/\r?\n/)) {
    const kv = line.match(/^([A-Za-z0-9_-]+):\s*(.*)$/);
    if (!kv) continue;
    const key = kv[1];
    let value = kv[2].trim();

    if (value.startsWith("[") && value.endsWith("]")) {
      value = value
        .slice(1, -1)
        .split(",")
        .map((v) => v.trim().replace(/^["']|["']$/g, ""))
        .filter(Boolean);
    } else {
      value = value.replace(/^["']|["']$/g, "");
    }
    meta[key] = value;
  }
  return meta;
}

function apiSkills() {
  const skillsDir = path.join(sdriveRoot, "skills");
  if (!fs.existsSync(skillsDir)) {
    return { exists: false, skills: [] };
  }

  const skills = [];
  for (const entry of fs.readdirSync(skillsDir, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;

    const skillFile = path.join(skillsDir, entry.name, "SKILL.md");
    if (!fs.existsSync(skillFile)) continue;

    let content = "";
    try {
      content = fs.readFileSync(skillFile, "utf8");
    } catch (error) {
      continue;
    }

    const meta = parseFrontmatter(content);
    const lines = content.split(/\r?\n/).length;

    // Count optional supporting files (examples/, references/, etc.)
    let extraFiles = 0;
    try {
      const walk = (dir) => {
        for (const child of fs.readdirSync(dir, { withFileTypes: true })) {
          if (child.isDirectory()) walk(path.join(dir, child.name));
          else if (child.name !== "SKILL.md") extraFiles += 1;
        }
      };
      walk(path.join(skillsDir, entry.name));
    } catch (error) {
      /* ignore */
    }

    // A skill may ship its own logo. We look for a conventional filename in the
    // skill folder (and an optional assets/ subfolder) so the dashboard can show
    // the real brand mark instead of the generic gradient icon.
    const logo = findSkillLogo(path.join(skillsDir, entry.name));

    skills.push({
      name: meta.name || entry.name,
      folder: entry.name,
      description: meta.description || "",
      version: meta.version || "",
      lastUpdated: meta.last_updated || "",
      tags: Array.isArray(meta.tags) ? meta.tags : [],
      source: meta.source || "",
      license: meta.license || "",
      installed: meta.installed || "",
      logo: logo ? `/api/skill-logo/${encodeURIComponent(entry.name)}` : "",
      lines,
      extraFiles
    });
  }

  skills.sort((a, b) => a.name.localeCompare(b.name));
  return { exists: true, skills };
}

// Conventional logo filenames, checked in order. The first match wins.
const SKILL_LOGO_NAMES = [
  "logo.svg",
  "logo.png",
  "logo.webp",
  "logo.jpg",
  "logo.jpeg",
  "icon.svg",
  "icon.png",
  "icon.webp",
  "icon.jpg",
  "icon.jpeg"
];

const SKILL_LOGO_TYPES = {
  ".svg": "image/svg+xml",
  ".png": "image/png",
  ".webp": "image/webp",
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg"
};

// Returns the absolute path to a skill's logo, or null if none is present.
// Looks in the skill root and an optional assets/ subfolder.
function findSkillLogo(skillDir) {
  const searchDirs = [skillDir, path.join(skillDir, "assets")];
  for (const dir of searchDirs) {
    for (const name of SKILL_LOGO_NAMES) {
      const candidate = path.join(dir, name);
      if (fs.existsSync(candidate)) return candidate;
    }
  }
  return null;
}

// Serves a skill's logo by folder name. The folder is validated against the
// skills directory listing so a crafted path cannot escape the skills root.
function serveSkillLogo(res, folder) {
  const skillsDir = path.join(sdriveRoot, "skills");
  const skillDir = path.join(skillsDir, folder);

  // Guard against path traversal: the resolved dir must sit directly under skills/.
  const resolved = path.resolve(skillDir);
  if (path.dirname(resolved) !== path.resolve(skillsDir)) {
    res.writeHead(400);
    return res.end("Bad request");
  }

  const logoPath = findSkillLogo(resolved);
  if (!logoPath) {
    res.writeHead(404);
    return res.end("Not found");
  }

  const type = SKILL_LOGO_TYPES[path.extname(logoPath).toLowerCase()] || "application/octet-stream";
  return serveStatic(res, logoPath, type);
}

function apiTeam() {
  return loadJson(path.join(sdriveRoot, "team.json"), { members: [] });
}

// Governance files may live in either layout:
//   versioned: .sdrive/governance/<feature>/<version>/<file>
//   legacy:    .sdrive/governance/<feature>/<file>
// Resolve the versioned path first, then fall back to the flat legacy path so
// projects created before versioned governance still render correctly.
function governanceFile(feature, version, fileName) {
  const versioned = path.join(sdriveRoot, "governance", feature, version, fileName);
  if (fs.existsSync(versioned)) return versioned;
  const legacy = path.join(sdriveRoot, "governance", feature, fileName);
  if (fs.existsSync(legacy)) return legacy;
  return versioned;
}

function apiTraceability(feature, version) {
  return loadJson(governanceFile(feature, version, "traceability.json"), { rows: [] });
}

// Execution tasks for a feature version. Tasks are grouped into phases, and
// each task links back to the requirements it satisfies. The dashboard uses
// this to show pending/completed counts and the full task breakdown.
function apiTasks(feature, version) {
  const data = loadJson(governanceFile(feature, version, "tasks.json"), {
    feature,
    version: Number(version) || 1,
    phases: []
  });

  const phases = Array.isArray(data.phases) ? data.phases : [];
  let total = 0;
  let completed = 0;

  const shaped = phases.map((phase) => {
    const tasks = Array.isArray(phase.tasks) ? phase.tasks : [];
    const done = tasks.filter((t) => t.completed === true).length;
    total += tasks.length;
    completed += done;
    return {
      phaseId: phase.phaseId || "",
      name: phase.name || "",
      dependencies: phase.dependencies || [],
      total: tasks.length,
      completed: done,
      tasks: tasks.map((t) => ({
        id: t.id || "",
        title: t.title || "",
        description: t.description || "",
        linkedRequirements: t.linkedRequirements || [],
        linkedAcceptanceCriteria: t.linkedAcceptanceCriteria || [],
        completed: t.completed === true
      }))
    };
  });

  return {
    feature: data.feature || feature,
    version: data.version || Number(version) || 1,
    summary: {
      total,
      completed,
      pending: total - completed,
      percent: total === 0 ? 0 : Math.round((completed / total) * 100)
    },
    phases: shaped
  };
}

function apiVerification(feature, version) {
  return loadJson(governanceFile(feature, version, "verification.json"), {
    status: "not_run",
    summary: {},
    items: {}
  });
}

function apiTestReport(feature, version) {
  const reportDir = path.join(sdriveRoot, "reports", "tests");

  if (!fs.existsSync(reportDir)) {
    return { status: "not_run", summary: {}, tests: [] };
  }

  const files = fs.readdirSync(reportDir)
    .filter((f) => f.startsWith(`test-report-${feature}-${version}-`))
    .sort()
    .reverse();

  if (files.length === 0) {
    return { status: "not_run", summary: {}, tests: [] };
  }

  const latest = path.join(reportDir, files[0]);
  const content = fs.readFileSync(latest, "utf8");

  const totalMatch = content.match(/Total ACs:\*\*\s*(\d+)/);
  const testedMatch = content.match(/Tested ACs:\*\*\s*(\d+)/);
  const coverageMatch = content.match(/Coverage:\*\*\s*(\d+)/);
  const passedMatch = content.match(/Tests Passed:\*\*\s*(\d+)\/(\d+)/);

  return {
    status:
      testedMatch && totalMatch && testedMatch[1] === totalMatch[1]
        ? "complete"
        : "partial",
    file: files[0],
    summary: {
      totalACs: totalMatch ? parseInt(totalMatch[1]) : 0,
      testedACs: testedMatch ? parseInt(testedMatch[1]) : 0,
      coverage: coverageMatch ? parseInt(coverageMatch[1]) : 0,
      passed: passedMatch ? parseInt(passedMatch[1]) : 0,
      total: passedMatch ? parseInt(passedMatch[2]) : 0
    }
  };
}

// Report types produced by the agents. Each agent writes to
// .sdrive/reports/<dir>/ using the filename pattern
// <prefix>-report-<feature>-<version>-<date>.md (or -all-<date>.md).
// The dashboard surfaces whichever of these exist for a feature.
const REPORT_TYPES = [
  { id: "tests", dir: "tests", prefix: "test", label: "Tests" },
  { id: "performance", dir: "performance", prefix: "performance", label: "Performance" },
  { id: "security", dir: "security", prefix: "security", label: "Security" },
  { id: "reviews", dir: "reviews", prefix: "review", label: "Review" },
  { id: "docs", dir: "docs", prefix: "coverage", label: "Documentation" },
  { id: "debug", dir: "debug", prefix: "debug", label: "Debug" }
];

// Read the first markdown heading of a report so the UI can show a title
// without shipping the whole file. Falls back to null when absent.
function readReportTitle(filePath) {
  try {
    const content = fs.readFileSync(filePath, "utf8");
    const match = content.match(/^#\s+(.+)$/m);
    return match ? match[1].trim() : null;
  } catch (error) {
    return null;
  }
}

// Extract a "Status: PASS" / "**Status:** PASS" style verdict if present.
function readReportStatus(filePath) {
  try {
    const content = fs.readFileSync(filePath, "utf8");
    const match = content.match(/\*{0,2}Status:?\*{0,2}\s*:?\s*\*{0,2}\s*(PASS|FAIL|PASSED|FAILED|CLEAN|IMPROVED|REGRESSED)/i);
    return match ? match[1].toUpperCase() : null;
  } catch (error) {
    return null;
  }
}

// Collect every report for a feature across all report types.
// Feature-scoped reports are matched by "<prefix>-report-<feature>-<version>-".
// Global reports (e.g. debug-report-all-<date>.md) are matched separately and
// flagged so the UI can distinguish them.
function apiReports(feature, version) {
  const reportsRoot = path.join(sdriveRoot, "reports");
  const types = [];

  for (const type of REPORT_TYPES) {
    const dir = path.join(reportsRoot, type.dir);
    if (!fs.existsSync(dir)) {
      types.push({ id: type.id, label: type.label, status: "missing", reports: [] });
      continue;
    }

    let files = [];
    try {
      files = fs.readdirSync(dir).filter((f) => f.endsWith(".md"));
    } catch (error) {
      files = [];
    }

    const scoped = files
      .filter((f) => f.startsWith(`${type.prefix}-report-${feature}-${version}-`))
      .sort()
      .reverse();

    const global = files
      .filter((f) => f.startsWith(`${type.prefix}-report-all-`))
      .sort()
      .reverse();

    const reports = [];

    if (scoped.length) {
      const latest = scoped[0];
      const full = path.join(dir, latest);
      reports.push({
        file: latest,
        scope: "feature",
        date: latest.replace(/\.md$/, "").split("-").slice(-3).join("-"),
        title: readReportTitle(full),
        status: readReportStatus(full),
        history: scoped.length
      });
    }

    if (global.length) {
      const latest = global[0];
      const full = path.join(dir, latest);
      reports.push({
        file: latest,
        scope: "all",
        date: latest.replace(/\.md$/, "").split("-").slice(-3).join("-"),
        title: readReportTitle(full),
        status: readReportStatus(full),
        history: global.length
      });
    }

    types.push({
      id: type.id,
      label: type.label,
      status: reports.length ? "present" : "missing",
      reports
    });
  }

  return { feature, version, types };
}

function parseFeatureVersion(url) {
  // Strip the query string, then drop the leading "/api/<resource>" segments so
  // only the feature/version tail remains. Without this, "/api/reports/foo"
  // would yield feature="reports", version="foo".
  const clean = url.split("?")[0];
  const parts = clean.split("/").filter(Boolean);
  const tail = parts.slice(2);
  return {
    feature: tail.length >= 2 ? tail[tail.length - 2] : tail[0],
    version: tail.length >= 2 ? tail[tail.length - 1] : null
  };
}

// Spec documents live under .sdrive/specs/<stage>/<feature>/<version>/.
// A feature moves from backlog -> ongoing -> completed, so we search every
// stage and return the first folder that actually holds the version. This lets
// the dashboard read the raw spec/plan/tasks/traceability markdown without the
// user having to open the repo.
const SPEC_STAGES = ["ongoing", "completed", "backlog"];
const SPEC_DOCS = [
  { id: "spec", label: "Specification", file: "spec.md" },
  { id: "plan", label: "Plan", file: "plan.md" },
  { id: "tasks", label: "Tasks", file: "tasks.md" },
  { id: "traceability", label: "Traceability", file: "traceability.md" },
  { id: "review", label: "Review", file: "review.md" }
];

function resolveSpecDir(feature, version) {
  for (const stage of SPEC_STAGES) {
    const dir = path.join(sdriveRoot, "specs", stage, feature, version);
    if (fs.existsSync(dir)) return { stage, dir };
  }
  return { stage: null, dir: null };
}

// Returns the raw markdown for every spec document that exists for a feature
// version, plus which stage (ongoing/completed/backlog) it was found in.
function apiSpecFiles(feature, version) {
  const { stage, dir } = resolveSpecDir(feature, version);

  if (!dir) {
    return { feature, version, stage: null, docs: [] };
  }

  const docs = [];
  for (const doc of SPEC_DOCS) {
    const full = path.join(dir, doc.file);
    if (!fs.existsSync(full)) continue;
    let content = "";
    try {
      content = fs.readFileSync(full, "utf8");
    } catch (error) {
      content = "";
    }
    docs.push({
      id: doc.id,
      label: doc.label,
      file: doc.file,
      path: `.sdrive/specs/${stage}/${feature}/${version}/${doc.file}`,
      bytes: Buffer.byteLength(content, "utf8"),
      lines: content ? content.split(/\r?\n/).length : 0,
      content
    });
  }

  return { feature, version, stage, docs };
}

// ---------- FILE VIEWER ----------
// Serves one file so a path referenced inside a spec can be opened without leaving
// the dashboard. Two guards here are not optional:
//
//   1. The resolved path must stay INSIDE the project. A spec can contain any text,
//      and resolving without this check would let it read the whole drive.
//   2. Secret-shaped files are refused by NAME, before anything is read. This
//      dashboard only listens on localhost, but "local" is not a reason to page
//      through a .env.
const FILE_DENY_PATTERNS = [
  /(^|[\\/])\.env(\.[^\\/]*)?$/i,
  /\.[Pp][Ee][Mm]$/,
  /\.key$/i,
  /(^|[\\/])secrets?\.json$/i,
  /(^|[\\/])id_(rsa|ed25519)/i,
  /(^|[\\/])\.npmrc$/i,
  /(^|[\\/])\.git[\\/]/i
];
const FILE_MAX_BYTES = 512 * 1024;

function apiFile(requestedPath, checkOnly) {
  const requested = String(requestedPath || "").trim();
  if (!requested) return { found: false, reason: "no path given" };

  if (FILE_DENY_PATTERNS.some((pattern) => pattern.test(requested))) {
    return { found: false, reason: "refused: this looks like a secret or internal file" };
  }
  if (/secret|credential|private[-_]?key/i.test(requested)) {
    return { found: false, reason: "refused: this looks like a secret or internal file" };
  }

  const root = path.resolve(projectRoot);
  const resolved = path.resolve(root, requested);

  // Compare with a trailing separator so `/project-evil` cannot pass as `/project`.
  if (resolved !== root && !resolved.startsWith(root + path.sep)) {
    return { found: false, reason: "outside the project" };
  }

  let stat;
  try {
    stat = fs.statSync(resolved);
  } catch (error) {
    return { found: false, reason: "no such file" };
  }
  if (!stat.isFile()) return { found: false, reason: "not a file" };

  // Existence-only mode: the client uses this to decide whether a path-shaped
  // token in a spec is really a file before offering it as clickable.
  if (checkOnly) return { found: true, path: requested, exists: true };

  let buffer;
  try {
    buffer = fs.readFileSync(resolved);
  } catch (error) {
    return { found: false, reason: "unreadable" };
  }

  // A NUL byte near the start is the cheapest reliable binary test there is.
  if (buffer.subarray(0, 8192).includes(0)) {
    return { found: true, path: requested, binary: true, bytes: stat.size, content: "" };
  }

  const truncated = buffer.length > FILE_MAX_BYTES;
  const content = buffer.subarray(0, FILE_MAX_BYTES).toString("utf8");

  return {
    found: true,
    path: requested,
    binary: false,
    bytes: stat.size,
    truncated,
    lines: content.split(/\r?\n/).length,
    markdown: /\.(md|markdown)$/i.test(requested),
    content
  };
}

function serveStatic(res, filePath, contentType) {
  if (!fs.existsSync(filePath)) {
    res.writeHead(404);
    res.end("Not found");
    return;
  }

  res.writeHead(200, { "Content-Type": contentType });
  res.end(fs.readFileSync(filePath));
}

// ---------- LIVE REFRESH STAMP ----------
// A cheap fingerprint of everything the dashboard renders. The client polls this
// - a few hundred bytes - and re-fetches the real data only when it changes, so a
// poll that finds nothing new cannot disturb the page someone is reading. Polling
// the full APIs on a timer would be wasteful and would repaint regardless.
// Stamps are read asynchronously. The walk below touches every file under six trees,
// and on a cold filesystem that first pass costs over a second - more than long enough
// to freeze every other request if it ran on the event loop.
async function fileStamp(filePath) {
  try {
    const stat = await fs.promises.stat(filePath);
    return `${stat.size}:${Math.round(stat.mtimeMs)}`;
  } catch (error) {
    return "0";
  }
}

// Newest mtime plus a file count for a tree. The count matters as much as the
// time: deleting a file can leave the newest mtime untouched.
async function treeStamp(dir) {
  let newest = 0;
  let count = 0;

  const walk = async (current) => {
    let entries;
    try {
      entries = await fs.promises.readdir(current, { withFileTypes: true });
    } catch (error) {
      return;
    }

    // Siblings are independent, so they are read together rather than one at a time.
    await Promise.all(
      entries.map(async (entry) => {
        const full = path.join(current, entry.name);
        if (entry.isDirectory()) {
          return walk(full);
        }

        count += 1;
        try {
          const stat = await fs.promises.stat(full);
          if (stat.mtimeMs > newest) newest = stat.mtimeMs;
        } catch (error) {
          // A file that vanished mid-walk is not a reason to fail the whole stamp.
        }
      })
    );
  };

  await walk(dir);
  return `${count}:${Math.round(newest)}`;
}

// ---------- PROJECT HEALTH ----------
// One score from five inputs, every one derived from files that already exist. The
// breakdown travels with the number on purpose: a score nobody can audit is
// decoration, and a dashboard has to earn the trust it asks for.
//
// Nothing is stored, and no component can be moved by editing a file - only by
// approving, testing, verifying, or archiving something real. A health number that
// can be gamed is worse than no number at all.
const HEALTH_GATES = ["specification", "planning", "implementation", "review"];
const HEALTH_STALE_DAYS = 14;
const HEALTH_MOMENTUM_DAYS = 7;
const HEALTH_MEMO_MS = 3000;
let healthMemo = { at: 0, value: null };

// Live = a version that has not been archived. Finished work is not a health
// problem; scoring it would punish a project for having shipped.
function liveVersions(state) {
  const live = [];
  for (const feature of state.features || []) {
    const version = (feature.versions || []).find((v) => v.id === feature.currentVersion);
    if (!version || version.phase === "completed") continue;
    live.push({ feature, version });
  }
  return live;
}

// Resolve a rules module from the copy that ships alongside this dashboard.
//
// __dirname is either <project>/.sdrive/dashboard or <package>/dashboard, and in both
// cases its sibling commands/ is the version-matched rules layer - the one that was
// released together with this file. Reading from sdriveRoot alone could pick up a stale
// project copy that predates an accessor and quietly score every gate as unapproved, so
// the sibling copy wins and the project copy is the fallback for older layouts.
function requireRules(relative) {
  const candidates = [path.join(__dirname, "..", relative), path.join(sdriveRoot, relative)];

  for (const candidate of candidates) {
    try {
      return require(candidate);
    } catch (error) {
      // Try the next candidate; the caller decides what a missing module means.
    }
  }

  return null;
}

const PROJECTS_CACHE_MS = 2000;
let projectsMemo = { at: 0, roots: [] };

function isSdriveProject(root) {
  return fs.existsSync(path.join(root, ".sdrive", "config.json"));
}

// A dashboard can move between projects without restarting. Discovery is
// deliberately local and explicit: the current project, SDRIVE_PROJECTS, an
// optional ~/.sdrive/projects.json, and immediately sibling projects. The
// endpoint never accepts an arbitrary path that was not offered by discovery.
function discoverProjectRoots() {
  if (projectsMemo.roots.length && Date.now() - projectsMemo.at < PROJECTS_CACHE_MS) {
    return projectsMemo.roots;
  }

  const roots = new Set();

  const add = (candidate) => {
    if (typeof candidate !== "string" || !candidate.trim()) return;
    const resolved = path.resolve(candidate.trim());
    if (isSdriveProject(resolved)) roots.add(resolved);
  };

  add(projectRoot);

  const configured = process.env.SDRIVE_PROJECTS || "";
  for (const candidate of configured.split(path.delimiter)) add(candidate);

  const registryPath = path.join(os.homedir(), ".sdrive", "projects.json");
  if (fs.existsSync(registryPath)) {
    try {
      const registry = JSON.parse(fs.readFileSync(registryPath, "utf8"));
      const entries = Array.isArray(registry) ? registry : registry.projects || [];
      for (const entry of entries) add(typeof entry === "string" ? entry : entry.path);
    } catch (error) {
      // A malformed registry must not break the dashboard.
    }
  }

  const parent = path.dirname(projectRoot);
  try {
    const entries = fs.readdirSync(parent, { withFileTypes: true }).slice(0, 200);
    for (const entry of entries) {
      if (entry.isDirectory()) add(path.join(parent, entry.name));
    }
  } catch (error) {
    // The parent may not be readable; the current project still works.
  }

  const list = [...roots];
  projectsMemo = { at: Date.now(), roots: list };
  return list;
}

function apiProjects() {
  const roots = discoverProjectRoots();
  const nameCounts = new Map();

  for (const root of roots) {
    const name = path.basename(root);
    nameCounts.set(name, (nameCounts.get(name) || 0) + 1);
  }

  return {
    active: path.resolve(projectRoot),
    projects: roots
      .map((root) => {
        const name = path.basename(root);
        return {
          name,
          path: root,
          label: nameCounts.get(name) > 1 ? `${name} — ${path.dirname(root)}` : name,
          active: path.resolve(root) === path.resolve(projectRoot)
        };
      })
      .sort((a, b) => Number(b.active) - Number(a.active) || a.label.localeCompare(b.label))
  };
}

function switchProject(target) {
  const resolved = path.resolve(String(target || ""));

  if (!discoverProjectRoots().includes(resolved)) {
    return {
      ok: false,
      error: "That project is not in the dashboard's project list. Add it to SDRIVE_PROJECTS or ~/.sdrive/projects.json."
    };
  }

  projectRoot = resolved;
  sdriveRoot = path.join(projectRoot, ".sdrive");
  projectsMemo = { at: 0, roots: [] };

  // Every cached answer was built from the previous project's files.
  gitCache.clear();
  healthMemo = { at: 0, value: null };
  stampMemo = { at: 0, value: null };

  return { ok: true, project: path.basename(projectRoot) };
}

function readJsonBody(req) {
  return new Promise((resolve) => {
    let body = "";

    req.on("data", (chunk) => {
      body += chunk;
      if (body.length > 1024 * 1024) req.destroy();
    });

    req.on("end", () => {
      try {
        resolve(JSON.parse(body || "{}"));
      } catch (error) {
        resolve(null);
      }
    });

    req.on("error", () => resolve(null));
  });
}

// Reuses commands/archive.js instead of reimplementing the rules, so this score and
// `sdrive archive:check` can never disagree about whether something is stuck.
function archivableChecker() {
  const archive = requireRules(path.join("commands", "archive.js"));

  return archive && typeof archive.checkArchivable === "function" ? archive.checkArchivable : null;
}

function eventTime(event) {
  const raw = event.timestamp || event.at || event.date || "";
  const parsed = Date.parse(raw);
  return Number.isNaN(parsed) ? null : parsed;
}

// Same reasoning as archivableChecker: reuse the real checker so the page can never
// suggest an approval that `sdrive approve` would refuse.
function gateChecker() {
  const gates = requireRules(path.join("commands", "gates.js"));

  return gates && typeof gates.checkGateRequirements === "function" ? gates.checkGateRequirements : null;
}

// Gate status is READ through commands/gates.js. Comparing `gates[gate] === "approved"`
// here was a second implementation of the rule, and the one endpoint that used it would
// have kept reporting stale results the moment the gate record changed shape.
//
// The accessors must be present, not merely the file: an older copy that loads fine but
// lacks them would throw on every health request instead of degrading to "unreadable".
function gateAccessors() {
  const gates = requireRules(path.join("commands", "gates.js"));

  if (
    !gates ||
    typeof gates.isGateApprovedFor !== "function" ||
    typeof gates.gateStatusFor !== "function"
  ) {
    return null;
  }

  return gates;
}

function apiValidateProject() {
  try {
    const { validateArtifacts } = require(path.join(__dirname, "..", "scripts", "validate-governance"));
    const verdict = validateArtifacts(projectRoot, {});
    return { ok: verdict.ok, results: verdict.results, skipped: verdict.skipped };
  } catch (error) {
    return {
      ok: false,
      error: error.message,
      results: [],
      skipped: []
    };
  }
}

function apiTestEvidence(feature, version) {
  if (!feature || !version) {
    return { found: false, evidence: null, error: "feature and version are required" };
  }

  const file = governanceFile(feature, version, "test-evidence.json");
  const evidence = loadJson(file, null);

  return {
    found: Boolean(evidence),
    evidence,
    path: path.relative(projectRoot, file)
  };
}

function apiReviewRecord(feature, version) {
  if (!feature || !version) {
    return { found: false, review: null, reviewMd: null, error: "feature and version are required" };
  }

  const file = governanceFile(feature, version, "review.json");
  const review = loadJson(file, null);
  const { dir } = resolveSpecDir(feature, version);
  let reviewMd = null;

  if (dir) {
    try {
      reviewMd = fs.readFileSync(path.join(dir, "review.md"), "utf8");
    } catch (error) {
      reviewMd = null;
    }
  }

  return {
    found: Boolean(review),
    review,
    reviewMd,
    path: path.relative(projectRoot, file)
  };
}

function apiArchiveCheck(feature) {
  if (!feature) {
    return { ok: false, blockers: ["feature is required"] };
  }

  const check = archivableChecker();

  if (!check) {
    return { ok: false, blockers: ["Archive rules are not available in this installation."] };
  }

  try {
    return check(projectRoot, feature);
  } catch (error) {
    return { ok: false, blockers: [error.message] };
  }
}

function apiMigrations() {
  let detections = [];

  try {
    const migrations = require(path.join(__dirname, "..", "migrations"));
    detections = migrations.detectAll(projectRoot).filter((entry) => entry.detected);
  } catch (error) {
    detections = [];
  }

  const runsDir = path.join(sdriveRoot, "migration-runs");
  const runs = [];

  if (fs.existsSync(runsDir)) {
    for (const name of fs.readdirSync(runsDir).slice(0, 20)) {
      const report = loadJson(path.join(runsDir, name, "report.json"), null);
      if (!report) continue;
      runs.push({
        id: report.migrationId || name,
        framework: report.framework,
        appliedAt: report.appliedAt,
        counts: report.counts,
        features: (report.features || []).map((feature) => ({
          feature: feature.feature,
          status: feature.status
        }))
      });
    }
  }

  return { detections, runs };
}

function apiHealth() {
  // Every open tab polls on a timer, so several requests can land within a second of
  // each other and each one would rebuild the whole breakdown from disk. A short memo
  // collapses those into one read without making the panel feel stale.
  if (healthMemo.value && Date.now() - healthMemo.at < HEALTH_MEMO_MS) {
    return healthMemo.value;
  }

  const value = computeHealth();
  healthMemo = { at: Date.now(), value };
  return value;
}

function computeHealth() {
  const state = loadJson(path.join(sdriveRoot, "workflow-state.json"), { features: [] });
  const live = liveVersions(state);
  const events = loadJson(path.join(sdriveRoot, "audit-log.json"), { events: [] }).events || [];
  const checkArchivable = archivableChecker();
  const checkGateRequirements = gateChecker();
  const gatesApi = gateAccessors();

  if (live.length === 0) {
    return {
      available: false,
      reason: "No live features. Every version is either not started or already archived, so there is nothing to score yet.",
      components: [],
      nextAction: null,
      blockers: []
    };
  }

  // ---- 1. Gates ----
  let gatesApproved = 0;
  const gatesMissing = [];
  for (const { feature, version } of live) {
    for (const gate of HEALTH_GATES) {
      // Unreadable gate state counts as unapproved: never credit work that cannot be
      // verified, and never claim a gate the CLI would not report as approved.
      if (gatesApi && gatesApi.isGateApprovedFor(version, gate)) {
        gatesApproved += 1;
      } else {
        gatesMissing.push(`${feature.name} · ${gate}`);
      }
    }
  }

  // ---- 2. Coverage (the same source as the TESTS chip, so they always agree) ----
  let acsTotal = 0;
  let acsTested = 0;
  const coverageGaps = [];
  for (const { feature, version } of live) {
    const summary = apiTestReport(feature.name, version.id).summary || {};
    const total = summary.totalACs || 0;
    const tested = summary.testedACs || 0;
    acsTotal += total;
    acsTested += tested;
    if (total - tested > 0) {
      coverageGaps.push(`${feature.name}: ${total - tested} of ${total} ACs untested`);
    }
  }

  // ---- 3. Verification ----
  let verifiedCount = 0;
  const unverified = [];
  for (const { feature, version } of live) {
    const verification = apiVerification(feature.name, version.id);
    if (verification.status === "pass") {
      verifiedCount += 1;
      continue;
    }

    // Name the drift, not just the count. "WARN - 9 extra" with no list is a dead
    // end, and the items are already sitting in verification.json.
    const summary = verification.summary || {};
    const counts = ["missing", "renamed", "extra"]
      .filter((kind) => (summary[kind] || 0) > 0)
      .map((kind) => `${summary[kind]} ${kind}`);
    const names = ["missing", "renamed", "extra"]
      .flatMap((kind) => (verification.items && verification.items[kind]) || [])
      .map((item) => item.name || item.file || "")
      .filter(Boolean)
      .slice(0, 4);

    unverified.push(
      `${feature.name}: ${verification.status}${
        counts.length ? ` (${counts.join(", ")})` : ""
      }${names.length ? ` - ${names.join(", ")}` : ""}`
    );
  }

  // ---- 4. Blockers, straight from the archive checker ----
  const blockers = [];
  let archivableCount = 0;
  if (checkArchivable) {
    for (const { feature } of live) {
      let verdict = null;
      try {
        verdict = checkArchivable(projectRoot, feature.name);
      } catch (error) {
        verdict = null;
      }
      if (verdict && verdict.ok) {
        archivableCount += 1;
      } else if (verdict && Array.isArray(verdict.blockers) && verdict.blockers.length > 0) {
        blockers.push({ feature: feature.name, blockers: verdict.blockers });
      }
    }
  }

  // ---- 5. Momentum ----
  const now = Date.now();
  const recent = events.filter((event) => {
    const at = eventTime(event);
    return at !== null && now - at <= HEALTH_MOMENTUM_DAYS * 86400000;
  });
  const lastAt = events.reduce((newest, event) => {
    const at = eventTime(event);
    return at === null ? newest : Math.max(newest, at);
  }, 0);
  const daysSinceLast = lastAt === 0 ? null : Math.floor((now - lastAt) / 86400000);

  const components = [
    {
      id: "gates",
      label: "Gates approved",
      weight: 30,
      earned: gatesApproved,
      max: live.length * HEALTH_GATES.length,
      detail: `${gatesApproved} of ${live.length * HEALTH_GATES.length} gate approvals are signed on live versions`,
      evidence: gatesMissing.slice(0, 8),
      evidenceMore: Math.max(0, gatesMissing.length - 8)
    },
    {
      id: "coverage",
      label: "Acceptance criteria tested",
      weight: 25,
      earned: acsTested,
      max: acsTotal,
      detail: `${acsTested} of ${acsTotal} acceptance criteria have a passing test`,
      evidence: coverageGaps,
      evidenceMore: 0
    },
    {
      id: "verification",
      label: "Spec-to-code verification",
      weight: 15,
      earned: verifiedCount,
      max: live.length,
      detail: `${verifiedCount} of ${live.length} live versions pass verification`,
      evidence: unverified,
      evidenceMore: 0
    },
    {
      id: "archivable",
      label: "Ready to archive",
      weight: 15,
      earned: archivableCount,
      max: checkArchivable ? live.length : 0,
      detail: checkArchivable
        ? `${archivableCount} of ${live.length} live versions have no blocker`
        : "the archive checker is not in this project yet - run `sdrive update`",
      evidence: blockers.map((b) => `${b.feature}: ${b.blockers[0]}`),
      evidenceMore: 0
    },
    {
      id: "freshness",
      label: "Freshness of activity",
      weight: 15,
      // Measures STALENESS, not velocity. An absolute "events per week" target is
      // unachievable for a small or early project and would pin its health at zero
      // through no fault of its own.
      earned: daysSinceLast === null ? 0 : Math.max(0, HEALTH_STALE_DAYS - daysSinceLast),
      max: HEALTH_STALE_DAYS,
      detail:
        daysSinceLast === null
          ? "no activity has ever been recorded in audit-log.json"
          : `last recorded activity was ${daysSinceLast} day(s) ago (full marks within a day, zero at ${HEALTH_STALE_DAYS})`,
      evidence:
        recent.length > 0
          ? [`${recent.length} event(s) in the last ${HEALTH_MOMENTUM_DAYS} days`]
          : [`nothing recorded in the last ${HEALTH_MOMENTUM_DAYS} days`],
      evidenceMore: 0
    }
  ];

  for (const component of components) {
    component.percent = component.max === 0 ? null : Math.round((component.earned / component.max) * 100);
  }

  // Rescaled over only the components that could actually be scored. An unavailable
  // checker should lower confidence in the score, never silently lower the score.
  let scoredWeight = 0;
  let earnedWeight = 0;
  for (const component of components) {
    if (component.percent === null) continue;
    scoredWeight += component.weight;
    earnedWeight += (component.percent / 100) * component.weight;
  }
  const score = scoredWeight === 0 ? 0 : Math.round((earnedWeight / scoredWeight) * 100);

  const band =
    score >= 80 ? "Healthy" : score >= 55 ? "Needs attention" : "At risk";

  // The part that earns a return visit. A snapshot says where you are; this says
  // what to do, with the real command and an honest estimate of what it is worth.
  const opportunities = [];
  const coverageWeightPerAc = acsTotal > 0 ? 25 / acsTotal : 0;
  const gatesMax = live.length * HEALTH_GATES.length;
  const gateWeight = gatesMax > 0 ? 30 / gatesMax : 0;
  const verificationWeight = live.length > 0 ? 15 / live.length : 0;

  for (const { feature, version } of live) {
    const summary = apiTestReport(feature.name, version.id).summary || {};
    const untested = (summary.totalACs || 0) - (summary.testedACs || 0);
    if (untested > 0) {
      opportunities.push({
        points: untested * coverageWeightPerAc,
        title: `${feature.name}: ${untested} acceptance criteria have no passing test`,
        command: `/sdrive:test ${feature.name}`,
        reason: "Testing is the largest single lever on this project's health."
      });
    }

    // Verification was the biggest recoverable block and had no suggested action
    // until this existed - the score dropped without saying how to raise it.
    const verification = apiVerification(feature.name, version.id);
    if (verification.status !== "pass") {
      opportunities.push({
        points: verificationWeight,
        title: `${feature.name}: spec-to-code verification is ${
          verification.status === "warn" ? "reporting drift" : "not run"
        }`,
        command: `sdrive verify ${feature.name}`,
        reason: "Verification counts only when it passes; drift here is real work outstanding."
      });
    }

    const pending = HEALTH_GATES.filter(
      (gate) => !(gatesApi && gatesApi.isGateApprovedFor(version, gate))
    );
    if (pending.length > 0 && untested === 0) {
      // Only suggest an approval the real checker would accept. Recommending a
      // command that then refuses is worse than recommending nothing.
      const approvable = checkGateRequirements
        ? pending.filter((gate) => {
            try {
              return checkGateRequirements(projectRoot, feature.name, version, gate).ok === true;
            } catch (error) {
              return false;
            }
          })
        : [];

      if (approvable.length > 0) {
        opportunities.push({
          points: approvable.length * gateWeight,
          title: `${feature.name}: ${approvable.length} gate(s) ready to approve (${approvable.join(", ")})`,
          command: `sdrive approve ${feature.name} ${approvable[0]}`,
          reason: "Its requirements are met, so the approval is the only thing outstanding."
        });
      } else if (pending.length > 0) {
        opportunities.push({
          points: 0,
          title: `${feature.name}: ${pending.length} gate(s) pending, none approvable yet (${pending.join(", ")})`,
          command: `sdrive status`,
          reason:
            "A gate is waiting, but its requirements are not met — approving now would be refused.",
        });
      }
    }
  }

  if (blockers.length > 0) {
    opportunities.push({
      points: 0,
      title: `${blockers.length} live version(s) cannot be archived`,
      command: `sdrive archive:check ${blockers[0].feature}`,
      reason: "A blocker is not worth points, it is worth clearing - see the list below."
    });
  }

  opportunities.sort((a, b) => b.points - a.points);
  const best = opportunities[0] || null;
  const nextAction = best
    ? {
        title: best.title,
        command: best.command,
        reason: best.reason,
        points: Math.round(best.points)
      }
    : null;

  return {
    available: true,
    score,
    band,
    liveCount: live.length,
    components,
    nextAction,
    blockers
  };
}

const STAMP_MEMO_MS = 2000;
let stampMemo = { at: 0, value: null };

async function apiStamp() {
  if (stampMemo.value && Date.now() - stampMemo.at < STAMP_MEMO_MS) {
    return stampMemo.value;
  }

  const files = ["workflow-state.json", "audit-log.json", "team.json", "config.json"];
  const dirs = ["specs", "governance", "product", "onboarding", "reports", "context", "migration-runs"];

  const parts = await Promise.all([
    ...files.map((name) => fileStamp(path.join(sdriveRoot, name))),
    ...dirs.map((dir) => treeStamp(path.join(sdriveRoot, dir)))
  ]);

  stampMemo = { at: Date.now(), value: { stamp: parts.join("|") } };
  return stampMemo.value;
}

function apiOnboarding() {
  const state = loadJson(path.join(sdriveRoot, "workflow-state.json"), { features: [] });
  return state.onboarding || null;
}

function readProductFile(fileName) {
  const filePath = path.join(sdriveRoot, "product", fileName);
  if (!fs.existsSync(filePath)) return null;
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch (error) {
    return null;
  }
}

function parseBacklog(markdown) {
  if (!markdown) return [];

  const rows = [];
  for (const line of markdown.split(/\r?\n/)) {
    const trimmed = line.trim();
    if (!trimmed.startsWith("|")) continue;
    if (/^\|[\s:|-]+\|$/.test(trimmed)) continue;

    const cells = trimmed
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim());

    if (cells.length < 3) continue;
    if (/^id$/i.test(cells[0])) continue;

    // Backlog table columns: ID | Feature | Milestone | Priority | Dependencies | Status
    // Older/simpler tables may only have: ID | Feature | Status
    const hasFullColumns = cells.length >= 6;
    rows.push({
      id: cells[0],
      feature: cells[1],
      milestone: hasFullColumns ? cells[2] : "",
      priority: hasFullColumns ? cells[3] : "",
      dependencies: hasFullColumns ? cells[4] : "",
      status: hasFullColumns ? cells[5] : cells[2]
    });
  }

  return rows;
}

function apiProduct() {
  const overview = readProductFile("overview.md");
  const mockup = readProductFile("mockup.md");
  const designSystem = readProductFile("design-system.md");
  const roadmap = readProductFile("roadmap.md");
  const backlogMarkdown = readProductFile("backlog.md");

  const exists = Boolean(overview || mockup || designSystem || roadmap || backlogMarkdown);

  return {
    exists,
    overview,
    mockup,
    designSystem,
    roadmap,
    backlog: parseBacklog(backlogMarkdown),
    summary: parseProductOverview(overview)
  };
}

// Pull the high-signal sections out of product/overview.md so the Overview page
// can show vision, goals, target users, tech stack, and languages without
// dumping the whole markdown file.
function parseProductOverview(markdown) {
  if (!markdown) return null;

  const lines = markdown.split(/\r?\n/);
  const sections = {};
  let current = null;

  for (const line of lines) {
    const heading = line.match(/^#{1,3}\s+(.*)$/);
    if (heading) {
      current = heading[1].trim().toLowerCase();
      sections[current] = [];
      continue;
    }
    if (current) sections[current].push(line);
  }

  const findSection = (needle) => {
    const key = Object.keys(sections).find((k) => k.includes(needle));
    return key ? sections[key].join("\n").trim() : "";
  };

  const bullets = (text) =>
    text
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter((l) => /^[-*]\s+/.test(l))
      .map((l) => l.replace(/^[-*]\s+/, "").replace(/\*\*/g, "").trim());

  const tableRows = (text) =>
    text
      .split(/\r?\n/)
      .map((l) => l.trim())
      .filter((l) => l.startsWith("|") && !/^\|[\s:|-]+\|$/.test(l))
      .map((l) => l.split("|").slice(1, -1).map((c) => c.trim()))
      .filter((cells) => cells.length && !/^segment$/i.test(cells[0]));

  const titleMatch = markdown.match(/^#\s+(.*)$/m);
  const statusMatch = markdown.match(/Status:\s*([^·\n]+)/i);

  return {
    title: titleMatch ? titleMatch[1].trim() : "",
    status: statusMatch ? statusMatch[1].trim() : "",
    vision: findSection("vision"),
    problem: findSection("problem"),
    goals: bullets(findSection("goals")),
    nonGoals: bullets(findSection("non-goals")),
    targetUsers: tableRows(findSection("target users")).map((cells) => ({
      segment: cells[0] || "",
      description: cells[1] || "",
      need: cells[2] || ""
    })),
    guidance: bullets(findSection("confirmed guidance")),
    architecture: bullets(findSection("existing architecture")),
    openQuestions: bullets(findSection("open questions"))
  };
}

// Static CLI command reference. Kept in sync with src/index.js.
// Each entry carries plain-language fields so a non-technical reader can
// understand what the command does, why it exists, and when to reach for it.
const CLI_COMMANDS = [
  { command: "sdrive init", description: "Initialize SpecDrive structure, tool adapters, and MCP wiring", group: "Setup",
    what: "Sets up SpecDrive inside your project for the first time.",
    why: "Creates the folders, config, and AI-tool connections SpecDrive needs to work.",
    when: "Once, at the very start of a new project.",
    example: "sdrive init" },
  { command: "sdrive update", description: "Refresh framework files (dashboard, mcp, scripts, schemas) without touching specs or governance", group: "Setup",
    what: "Updates SpecDrive's own files to the latest version.",
    why: "You get new dashboard and tooling improvements without losing your specs or approvals.",
    when: "After upgrading the SpecDrive package.",
    example: "sdrive update" },
  { command: "sdrive onboard", description: "Scan an existing project for onboarding (usually run by /sdrive:onboard)", group: "Setup",
    what: "Reads an existing codebase and builds an inventory of what is already there.",
    why: "Lets you adopt SpecDrive into a project that already has code, without starting over.",
    when: "When bringing an existing project under SpecDrive.",
    example: "sdrive onboard" },
  { command: "sdrive config:validate", description: "Validate .sdrive/config.json", group: "Setup",
    what: "Checks that your SpecDrive configuration file is correct.",
    why: "Catches typos or missing settings before they cause confusing failures.",
    when: "After editing .sdrive/config.json.",
    example: "sdrive config:validate" },
  { command: "sdrive template:list", description: "List available templates", group: "Setup",
    what: "Shows the document templates SpecDrive can use.",
    why: "Helps you pick the right starting point for your specs.",
    when: "When choosing how specs should be formatted.",
    example: "sdrive template:list" },
  { command: "sdrive template:set <name>", description: "Set the default template", group: "Setup",
    what: "Chooses which template SpecDrive uses by default.",
    why: "Keeps every new spec consistent with your team's preferred format.",
    when: "Once, when configuring your project.",
    example: "sdrive template:set standard" },
  { command: "sdrive hooks:install", description: "Install the SpecDrive pre-commit hook", group: "Setup",
    what: "Adds an automatic check that runs before each Git commit.",
    why: "Stops invalid or unapproved changes from being committed by accident.",
    when: "Once, after initializing SpecDrive.",
    example: "sdrive hooks:install" },

  { command: "sdrive create <feature>", description: "Create a new feature scaffold (internal)", group: "Features",
    what: "Creates the empty folder structure for a new feature.",
    why: "Gives every feature the same organised home for its spec, plan, and tasks.",
    when: "Usually run for you by /sdrive:propose.",
    example: "sdrive create user-login" },
  { command: "sdrive update-propose <feature>", description: "Validate an existing feature for extension (internal)", group: "Features",
    what: "Checks the feature exists, is not already completed, and has a spec folder to extend.",
    why: "Extension has preconditions — a shipped feature must be reopened before new requirements can be added.",
    when: "Usually run for you by /sdrive:update-propose.",
    example: "sdrive update-propose user-login" },
  { command: "sdrive assign <feature> <username>", description: "Assign a team member as the owner of a feature", group: "Features",
    what: "Records which team member owns a feature.",
    why: "Ownership makes it obvious who is accountable for a feature's specs and gates.",
    when: "When a feature changes hands, or when starting work in a team.",
    example: "sdrive assign user-login alice" },
  { command: "sdrive status", description: "Show feature lifecycle state", group: "Features",
    what: "Prints where each feature currently stands.",
    why: "A quick way to see what is in progress, blocked, or done.",
    when: "Any time you want a status check.",
    example: "sdrive status" },
  { command: "sdrive version:new <feature>", description: "Create a new version of a feature", group: "Features",
    what: "Starts a fresh version of an existing feature.",
    why: "Lets you evolve a shipped feature without erasing its history.",
    when: "When adding significant new scope to a completed feature.",
    example: "sdrive version:new user-login" },
  { command: "sdrive amend <feature> --reason \"...\"", description: "Reopen a wrongly-closed feature to correct its record (corrective action)", group: "Governance",
    what: "Moves a closed version back into active work so missing evidence can be gathered, then asks which branch the work should happen on and which branch to base it on. The reason is required and recorded.",
    why: "Some closures are simply wrong — a feature shipped without the tests its own acceptance criteria required. Amending that same version is what makes its record true; issuing a new version instead would attach v1's proofs to a version nobody shipped.",
    when: "Only when a lifecycle was closed incorrectly. To change or extend a released feature, use sdrive version:new instead.",
    example: "sdrive amend user-login --reason \"archived before tests were written\"" },
  { command: "sdrive diff:version <feature> <a> <b>", description: "Compare two versions of a feature", group: "Features",
    what: "Shows what changed between two versions of a feature.",
    why: "Makes it easy to review scope changes over time.",
    when: "Before approving a new version.",
    example: "sdrive diff:version user-login v1 v2" },

  { command: "sdrive validate", description: "Validate SpecDrive governance files", group: "Governance",
    what: "Checks that all your spec and governance files are well-formed.",
    why: "Catches broken or incomplete documents before they block the workflow.",
    when: "Before approving a gate or merging.",
    example: "sdrive validate" },
  { command: "sdrive verify <feature>", description: "Verify code matches the plan (spec-to-code diff)", group: "Governance",
    what: "Compares what was specified against what was actually built.",
    why: "Proves the code matches the approved plan — no missing or surprise work.",
    when: "Before the review gate.",
    example: "sdrive verify user-login" },
  { command: "sdrive approve <feature> <gate>", description: "Approve a workflow gate (specification, planning, implementation, review)", group: "Governance",
    what: "Signs off on one of the four approval checkpoints — but only after verifying the stage's artifacts actually exist.",
    why: "Gates are how a human confirms each stage is genuinely ready to advance. Approving an unmet gate would falsify the governance record, so the check runs before anything is written.",
    when: "After reviewing a stage's output. The feature must be named explicitly.",
    example: "sdrive approve user-login specification" },
  { command: "sdrive archive:check <feature>", description: "Report whether a feature may be archived (changes nothing)", group: "Governance",
    what: "Re-verifies a feature's approvals and the evidence behind them, then reports whether it is safe to archive — without writing anything.",
    why: "Lets you see exactly what is still outstanding before touching state, and gives /sdrive:archive --all a read-only pass.",
    when: "Before archiving, or when a feature is refused.",
    example: "sdrive archive:check user-login" },
  { command: "sdrive archive:finalize <feature>", description: "Close a feature's lifecycle: move its spec to completed/ and mark the version completed", group: "Governance",
    what: "Moves the feature's spec folder to specs/completed/ and marks that version completed — but only once all four gates are already approved and the evidence behind them is still present.",
    why: "Archiving used to write the gates itself, so it could record an approval nobody granted. It now refuses instead of promoting, which means a feature that was never tested or reviewed cannot be recorded as archived.",
    when: "After /sdrive:review has passed and the merge is done.",
    example: "sdrive archive:finalize user-login" },
  { command: "sdrive diff <oldFile> <newFile>", description: "Show differences between two spec files", group: "Governance",
    what: "Highlights what changed between two documents.",
    why: "Speeds up review by focusing attention on the differences.",
    when: "When comparing spec revisions.",
    example: "sdrive diff old.json new.json" },
  { command: "sdrive openapi:generate <feature>", description: "Generate an OpenAPI spec from plan.json API contracts", group: "Governance",
    what: "Turns your planned API contracts into a standard OpenAPI document.",
    why: "Keeps API documentation in sync with the plan automatically.",
    when: "After the plan defines API endpoints.",
    example: "sdrive openapi:generate user-login" },

  { command: "sdrive team:add <username> <role>", description: "Add a new team member", group: "Team",
    what: "Registers a person and their role on the project.",
    why: "Tracks who is accountable for what.",
    when: "When someone joins the project.",
    example: "sdrive team:add jane developer" },
  { command: "sdrive team:remove <username>", description: "Remove a team member", group: "Team",
    what: "Removes a person from the project roster.",
    why: "Keeps the team list accurate as people move on.",
    when: "When someone leaves the project.",
    example: "sdrive team:remove jane" },
  { command: "sdrive team:list", description: "List all team members", group: "Team",
    what: "Shows everyone on the project and their roles.",
    why: "A single source of truth for who is involved.",
    when: "Any time you need the roster.",
    example: "sdrive team:list" },
  { command: "sdrive team:update <username> <role>", description: "Update a team member's role", group: "Team",
    what: "Changes a person's role.",
    why: "Keeps responsibilities current as people grow.",
    when: "When someone's role changes.",
    example: "sdrive team:update jane lead" },
  { command: "sdrive team:sync", description: "Sync team members from GitHub collaborators", group: "Team",
    what: "Pulls the contributor list straight from GitHub.",
    why: "Avoids manually re-typing who is on the project.",
    when: "After adding collaborators on GitHub.",
    example: "sdrive team:sync" },

  { command: "sdrive dashboard", description: "Start the SpecDrive web dashboard", group: "Dashboard",
    what: "Opens the visual dashboard in your browser.",
    why: "Gives you a friendly view of features, gates, and progress.",
    when: "Whenever you want to see the big picture.",
    example: "sdrive dashboard" }
];

// Plain-language descriptions for the slash commands, keyed by command name.
// The manifest holds the terse machine description; these are the human ones.
const SLASH_DETAILS = {
  "/sdrive:constitution": {
    what: "Establishes the project's ground rules and context.",
    why: "Every later decision is checked against this shared foundation.",
    when: "Immediately after sdrive init, before any feature work.",
    does: [
      "Interviews you about the product, stack, and constraints",
      "Merges the answers into a single constitution document",
      "Assigns stable IDs to every principle so later work can cite them"
    ],
    produces: [
      ".sdrive/constitution.md",
      ".sdrive/context/ (product, stack, constraints)"
    ],
    reads: ["Your answers", "Existing repo files (brownfield)"],
    handsOff: "/sdrive:product-plan"
  },
  "/sdrive:product-plan": {
    what: "Creates the product overview, mockup, roadmap, and feature backlog.",
    why: "Gives the whole team a shared picture of what is being built and why.",
    when: "Early, once the constitution exists.",
    does: [
      "Drafts the product overview and positioning",
      "Produces a UI mockup and roadmap",
      "Breaks the roadmap into a prioritised feature backlog"
    ],
    produces: [
      ".sdrive/product/overview.md",
      ".sdrive/product/mockup.html",
      ".sdrive/product/roadmap.md",
      ".sdrive/product/backlog.md"
    ],
    reads: [".sdrive/constitution.md"],
    handsOff: "/sdrive:propose"
  },
  "/sdrive:propose": {
    what: "Turns an idea into a full specification, plan, and task list.",
    why: "Nothing gets built until it is clearly specified and approved.",
    when: "When starting a new feature.",
    does: [
      "Writes the feature specification with numbered requirements",
      "Derives acceptance criteria and a traceability matrix",
      "Produces a technical plan and an ordered task list",
      "Opens a feature branch for the work"
    ],
    produces: [
      ".sdrive/specs/ongoing/<feature>/<version>/spec.md",
      ".sdrive/specs/ongoing/<feature>/<version>/plan.md",
      ".sdrive/specs/ongoing/<feature>/<version>/tasks.md",
      ".sdrive/specs/ongoing/<feature>/<version>/traceability.md",
      ".sdrive/governance/<feature>/<version>/*.json"
    ],
    reads: [".sdrive/constitution.md", ".sdrive/product/backlog.md"],
    handsOff: "/sdrive:apply"
  },
  "/sdrive:update-propose": {
    what: "Extends an existing specification with new requirements.",
    why: "Lets features grow without rewriting their history.",
    when: "When a feature proposal needs changes before the feature is archived.",
    does: [
      "Adds new requirements without renumbering existing ones",
      "Updates the plan and task list to cover the additions",
      "Refreshes the traceability matrix"
    ],
    produces: [
      "Updated spec.md, plan.md, tasks.md, traceability.md",
      "Updated governance JSON for the version"
    ],
    reads: ["The existing feature spec and governance"],
    handsOff: "/sdrive:apply"
  },
  "/sdrive:design": {
    what: "Builds the UI/UX prototype and design system.",
    why: "Agrees on the look and feel before code is written.",
    when: "After the spec, before implementation.",
    does: [
      "Produces an interactive HTML/CSS/JS prototype",
      "Defines the design tokens and component inventory",
      "Maps every screen back to its requirements"
    ],
    produces: [
      ".sdrive/design/<feature>/prototype.html",
      ".sdrive/design/<feature>/design-system.md"
    ],
    reads: ["The feature spec and acceptance criteria"],
    handsOff: "/sdrive:apply"
  },
  "/sdrive:sync": {
    what: "Keeps the spec, plan, tasks, and traceability in agreement.",
    why: "Prevents documents from drifting out of sync as things change.",
    when: "After any change to the spec or plan.",
    does: [
      "Diffs the spec against the plan, tasks, and traceability",
      "Repairs mismatched IDs and orphaned references",
      "Reports anything it could not reconcile"
    ],
    produces: ["Updated spec, plan, tasks, and traceability files"],
    reads: ["Every document in the feature's version folder"],
    handsOff: "/sdrive:apply"
  },
  "/sdrive:skills": {
    what: "Audits and manages the AI skills available to the project.",
    why: "Keeps the AI's capabilities current and trustworthy.",
    when: "Periodically, or when adding new tooling.",
    does: [
      "Inventories the skills currently available",
      "Verifies each skill against its source",
      "Discovers and proposes new skills from GitHub"
    ],
    produces: [".sdrive/skills/ (skill workspace and audit notes)"],
    reads: ["The project's skill configuration"],
    handsOff: "Nothing — this is a maintenance command"
  },
  "/sdrive:docs": {
    what: "Generates documentation and flags drift from the code.",
    why: "Keeps docs honest and up to date automatically.",
    when: "After implementation, or on a schedule. Use --all to document every ongoing feature.",
    does: [
      "Writes inline comments and API documentation",
      "Generates or refreshes the project README",
      "Flags places where the docs no longer match the code"
    ],
    produces: [
      ".sdrive/reports/docs/coverage-report-<feature>-<version>-<date>.md",
      "Updated README and inline docs"
    ],
    reads: ["The implementation", "The feature spec"],
    handsOff: "/sdrive:review"
  },
  "/sdrive:apply": {
    what: "Writes the production code from the approved tasks.",
    why: "This is where the plan becomes real, working software.",
    when: "After the plan is approved. Use --all to implement every ongoing feature, one branch each.",
    does: [
      "Bases the feature branch on the branch it will merge into, and reports any base that carries another feature's commits (CON-612)",
      "Works through the task list in order, resuming from the first unchecked task",
      "Commits to the feature branch as tasks complete",
      "Defers test-writing tasks to /sdrive:test",
      "Updates the governance task state as it goes"
    ],
    produces: [
      "Production source code",
      "Updated .sdrive/governance/<feature>/<version>/tasks.json",
      "Git commits on the feature branch"
    ],
    reads: ["tasks.md", "plan.md", "spec.md"],
    handsOff: "/sdrive:test"
  },
  "/sdrive:performance": {
    what: "Benchmarks and optimises performance and search visibility (SEO).",
    why: "Ensures the feature is not just correct, but fast enough and discoverable.",
    when: "After implementation, before review. Use --all to optimise every ongoing feature.",
    does: [
      "Benchmarks against the feature's speed NFRs",
      "Audits metadata, semantic HTML, structured data, and crawlability",
      "Applies approved speed and SEO fixes",
      "Re-measures to prove the fix landed"
    ],
    produces: [
      ".sdrive/reports/performance/performance-report-<feature>-<version>-<date>.md"
    ],
    reads: ["The implementation", "The feature's NFRs"],
    handsOff: "/sdrive:review"
  },
  "/sdrive:debug": {
    what: "Reproduces, isolates, and fixes defects — layout, APIs, runtime, and glitches.",
    why: "Turns a vague \"something is broken\" into a reproduced defect, a root cause, and a verified fix.",
    when: "When something is visibly wrong. Use --all to sweep every ongoing feature for defects.",
    does: [
      "Reproduces the symptom and captures evidence",
      "Isolates the smallest failing case",
      "Identifies the root cause before touching code",
      "Applies a minimal fix and re-verifies"
    ],
    produces: [
      ".sdrive/reports/debug/debug-report-<feature>-<version>-<date>.md",
      ".sdrive/reports/debug/repro/ (reproduction steps)",
      ".sdrive/reports/debug/evidence/ (before/after captures)"
    ],
    reads: ["The implementation", "Runtime logs and console output"],
    handsOff: "/sdrive:review"
  },
  "/sdrive:review": {
    what: "Runs the final audit and produces a pass/fail verdict.",
    why: "The last quality gate before work is considered done. It does not merge or archive.",
    when: "When the feature is complete. Use --all to review every ongoing feature one after the other.",
    does: [
      "Verifies every acceptance criterion has a linked, passing test",
      "Audits the code against the spec and acceptance criteria",
      "Checks quality, security, and governance compliance",
      "Makes no Git or GitHub writes — no pull requests, no branch switching, no merging",
      "Issues a pass/fail verdict with findings"
    ],
    produces: [
      ".sdrive/reports/reviews/review-report-<feature>-<version>-<date>.md"
    ],
    reads: ["The implementation", "The spec", "Every prior report"],
    handsOff: "/sdrive:archive"
  },
  "/sdrive:archive": {
    what: "Merges an approved feature to the confirmed target branch and completes its lifecycle.",
    why: "Turns a passing review into shipped, archived work — specs move to completed/ and the branch is cleaned up.",
    when: "Only after /sdrive:review has passed. Use --all to archive every ongoing feature that passed review.",
    does: [
      "Asks whether to merge at all, then confirms the target branch — never assumed",
      "Refuses a branch that also carries another feature's commits, and prints the order to merge instead (CON-611)",
      "Merges the feature branch into the confirmed target branch",
      "Moves the spec from ongoing/ to completed/",
      "Deletes the merged feature branch, only after the merge is confirmed",
      "Marks the feature complete in the workflow state"
    ],
    produces: [
      ".sdrive/specs/completed/<feature>/<version>/",
      "A merge commit on main"
    ],
    reads: ["The passing review report"],
    handsOff: "Nothing — the feature lifecycle ends here"
  },
  "/sdrive:secure": {
    what: "Performs a security audit, threat modelling, and fixes.",
    why: "Catches vulnerabilities before they reach users.",
    when: "Before the review gate. Use --all to audit every ongoing feature.",
    does: [
      "Threat-models the feature's attack surface",
      "Scans for OWASP-class vulnerabilities",
      "Applies approved fixes and re-tests"
    ],
    produces: [
      ".sdrive/reports/security/security-report-<feature>-<version>-<date>.md"
    ],
    reads: ["The implementation", "The feature spec"],
    handsOff: "/sdrive:review"
  },
  "/sdrive:test": {
    what: "Writes tests for the acceptance criteria and reports coverage.",
    why: "Proves the feature actually does what was promised.",
    when: "After implementation. Use --all to test every ongoing feature.",
    does: [
      "Writes tests covering each acceptance criterion",
      "Runs the suite and records the results",
      "Reports coverage and any unmet criteria"
    ],
    produces: [
      ".sdrive/reports/tests/test-report-<feature>-<version>-<date>.md",
      "Test files in the project's test directory"
    ],
    reads: ["The acceptance criteria", "The implementation"],
    handsOff: "/sdrive:review",
    optional: true,
    optionalNote:
      "Optional — run when development is done (end of a sprint or once an MVP is complete), not on every change."
  },
  "/sdrive:onboard": {
    what: "Adopts an existing project into SpecDrive.",
    why: "Brings legacy code under governance without breaking it.",
    when: "Only for existing projects, at the very start.",
    does: [
      "Reads the existing repository structure and conventions",
      "Detects other spec frameworks (OpenSpec, Spec Kit, Software Factory)",
      "Brings existing specs across without touching the originals",
      "Writes the project context files"
    ],
    produces: [
      ".sdrive/context/ (product, stack, constraints)",
      ".sdrive/onboarding-report.md"
    ],
    reads: ["The existing repository", "Any existing spec documents"],
    handsOff: "/sdrive:constitution"
  }
};

// The two product lifecycles SpecDrive supports. Rendered as cards at the
// bottom of the commands page so users can see which path applies to them.
// Each card shows the setup steps for that path, followed by the shared
// iteration lifecycle (propose -> apply -> review -> archive).
//
// Migration is NOT a separate lifecycle. Arriving from another spec framework
// is just a Brownfield project with extra context, so it is surfaced as a
// sub-card inside the Brownfield card (see `migrationNote`) rather than as a
// peer card with its own command list.
const PRODUCT_WORKFLOWS = [
  {
    id: "greenfield",
    name: "Greenfield",
    tagline: "Brand-new project",
    icon: "✦",
    summary:
      "You are beginning with a clean slate — either an empty folder or a freshly scaffolded repo that has no plan in place yet. SpecDrive walks you from that starting point all the way to governed, specified, and tested code.",
    steps: ["/sdrive:constitution", "/sdrive:product-plan"],
    // product-plan is the only optional setup step. It is for a project that has an
    // idea but no written plan, or an empty repo. A project that already knows what
    // it is building can go straight from the constitution to /sdrive:propose.
    optional: ["/sdrive:product-plan"],
    optionalNote:
      "/sdrive:product-plan is optional: run it when you have an idea but no written plan, or an empty repo. You can go straight from the constitution to /sdrive:propose.",
    iteration: ["/sdrive:propose", "/sdrive:apply", "/sdrive:test", "/sdrive:review", "/sdrive:archive"]
  },
  {
    id: "brownfield",
    name: "Brownfield",
    tagline: "Existing codebase",
    icon: "◈",
    summary:
      "Your codebase is already well underway — a plan exists and development has moved far past the early stages. SpecDrive takes stock of what is there, writes specs after the fact, and flags the gaps as deferred so nothing breaks.",
    steps: ["/sdrive:onboard", "/sdrive:constitution"],
    optional: [],
    iteration: ["/sdrive:propose", "/sdrive:apply", "/sdrive:test", "/sdrive:review", "/sdrive:archive"],
    migrationNote: {
      title: "Coming from another spec framework?",
      body:
        "If SpecDrive detects OpenSpec, Software Factory, Spec Kit, or another spec-driven framework in your repo, onboarding brings your existing specs across and leaves the originals untouched. Nothing extra to run — the same Brownfield lifecycle applies."
    }
  }
];

// Optional commands are not part of the core workflow. They are run on demand —
// for example when a feature is totally done and ready to ship. They are shown
// in their own card on the Commands page, separate from the workflow cards.
const OPTIONAL_COMMANDS = {
  title: "Optional Commands",
  tagline: "Run on demand",
  icon: "✦",
  summary:
    "These commands are not part of the core workflow. Run them when you need them - for example when performance becomes a concern, when you need to fix a defect, or when you want docs.",
  commands: [
    {
      command: "/sdrive:update-propose",
      what: "Revises an existing proposal after feedback, without starting over.",
      when: "When a feature proposal needs changes before the feature is archived."
    },
    {
      command: "/sdrive:sync",
      what: "Re-aligns spec, plan, tasks, and traceability after a change.",
      when: "After a migration, or whenever the spec and the code drift apart."
    },
    {
      command: "/sdrive:performance",
      what: "Benchmarks and optimizes the code for speed, efficiency, and search visibility.",
      when: "When performance or SEO matters — before a release or after a slowdown."
    },
    {
      command: "/sdrive:debug",
      what: "Reproduces, isolates, and fixes defects — layout, APIs, runtime, and glitches.",
      when: "When something is visibly wrong — a broken layout, a failing endpoint, a console error."
    },
    {
      command: "/sdrive:docs",
      what: "Generates inline comments, API docs, and project READMEs.",
      when: "When you want documentation that matches the code."
    },
    {
      command: "/sdrive:secure",
      what: "Runs a deep security audit and fixes vulnerabilities.",
      when: "Before shipping anything that touches auth, data, or payments."
    }
  ]
};

function apiCommands() {
  const manifestPath = path.join(sdriveRoot, "commands", "manifest.json");
  const manifest = loadJson(manifestPath, { commands: [] });

  const slash = (manifest.commands || []).map((c) => {
    const detail = SLASH_DETAILS[c.command] || {};
    return {
      command: c.command,
      description: c.description || "",
      agent: c.agent || "",
      acceptsFeature: Boolean(c.acceptsFeature),
      acceptsAll: Boolean(c.acceptsAll),
      gates: c.gates || [],
      what: detail.what || "",
      why: detail.why || "",
      when: detail.when || "",
      does: detail.does || [],
      produces: detail.produces || [],
      reads: detail.reads || [],
      handsOff: detail.handsOff || "",
      optional: Boolean(detail.optional),
      optionalNote: detail.optionalNote || ""
    };
  });

  return { slash, cli: CLI_COMMANDS, workflows: PRODUCT_WORKFLOWS, optionalCommands: OPTIONAL_COMMANDS };
}

async function handleRequest(req, res) {
  try {
    return await routeRequest(req, res);
  } catch (error) {
    // One failing endpoint must not take the whole dashboard down. The page polls
    // several endpoints on a timer, so a single bad read would otherwise leave every
    // open tab connected to nothing.
    console.error(`❌ ${req.method} ${req.url} failed: ${error.message}`);

    if (res.headersSent) {
      return res.end();
    }

    res.writeHead(500, { "Content-Type": "application/json" });
    return res.end(JSON.stringify({ error: error.message }));
  }
}

function setSecurityHeaders(res) {
  // The dashboard is a local read-only view. These headers cost nothing and
  // close the obvious browser-side gaps if someone sets SDRIVE_HOST.
  res.setHeader("X-Content-Type-Options", "nosniff");
  res.setHeader("X-Frame-Options", "DENY");
  res.setHeader("Referrer-Policy", "no-referrer");
  res.setHeader(
    "Content-Security-Policy",
    [
      "default-src 'self'",
      "img-src 'self' data:",
      "style-src 'self' 'unsafe-inline'",
      "script-src 'self'",
      "connect-src 'self'",
      "font-src 'self' data:",
      "object-src 'none'",
      "base-uri 'none'",
      "frame-ancestors 'none'"
    ].join("; ")
  );
}

async function routeRequest(req, res) {
  const url = req.url;

  setSecurityHeaders(res);

  // Skill logos are binary assets, so they are served before the JSON API block
  // (which sets a JSON content-type header for everything under /api/).
  if (url.startsWith("/api/skill-logo/")) {
    const folder = decodeURIComponent(url.slice("/api/skill-logo/".length).split("?")[0]);
    return serveSkillLogo(res, folder);
  }

  if (url.startsWith("/api/")) {
    // Status and content-type are SET here, not written. writeHead() flushes the
    // headers immediately, so the 404 fallthrough below threw ERR_HTTP_HEADERS_SENT
    // and terminated the process on any unrecognised /api/ path.
    res.statusCode = 200;
    res.setHeader("Content-Type", "application/json");
    res.setHeader("Cache-Control", "no-store");

    if (url === "/api/workflow-state") {
      return res.end(JSON.stringify(apiWorkflowState()));
    }

    if (url === "/api/audit-log") {
      return res.end(JSON.stringify(apiAuditLog()));
    }

    if (url === "/api/activity") {
      return res.end(JSON.stringify(apiActivity()));
    }

    if (url === "/api/skills") {
      return res.end(JSON.stringify(apiSkills()));
    }
    if (url === "/api/team") {
      return res.end(JSON.stringify(apiTeam()));
    }

    if (url.startsWith("/api/traceability/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiTraceability(feature, version)));
    }

    if (url.startsWith("/api/tasks/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiTasks(feature, version)));
    }

    if (url.startsWith("/api/verification/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiVerification(feature, version)));
    }

    if (url.startsWith("/api/tests/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiTestReport(feature, version)));
    }

    if (url.startsWith("/api/reports/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiReports(feature, version)));
    }

    if (url.startsWith("/api/spec-files/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiSpecFiles(feature, version)));
    }

    if (url === "/api/onboarding") {
      return res.end(JSON.stringify(apiOnboarding()));
    }

    if (url === "/api/product") {
      return res.end(JSON.stringify(apiProduct()));
    }

    if (url === "/api/git-info") {
      return res.end(JSON.stringify(await apiGitInfo()));
    }

    if (url === "/api/branches") {
      return res.end(JSON.stringify(await apiBranches()));
    }

    if (url === "/api/project-info") {
      return res.end(JSON.stringify(apiProjectInfo()));
    }

    if (url === "/api/product-type") {
      return res.end(JSON.stringify(apiProductType()));
    }

    if (url === "/api/stamp") {
      return res.end(JSON.stringify(await apiStamp()));
    }

    if (url === "/api/health") {
      return res.end(JSON.stringify(apiHealth()));
    }

    if (url === "/api/projects") {
      return res.end(JSON.stringify(apiProjects()));
    }

    if (url === "/api/switch-project") {
      if (req.method !== "POST") {
        res.writeHead(405);
        return res.end(JSON.stringify({ error: "Use POST" }));
      }

      const body = await readJsonBody(req);
      if (!body || typeof body.path !== "string") {
        res.writeHead(400);
        return res.end(JSON.stringify({ error: "A project path is required" }));
      }

      const result = switchProject(body.path);
      res.writeHead(result.ok ? 200 : 400);
      return res.end(JSON.stringify(result));
    }

    if (url === "/api/validate") {
      return res.end(JSON.stringify(apiValidateProject()));
    }

    if (url === "/api/migrations") {
      return res.end(JSON.stringify(apiMigrations()));
    }

    if (url.startsWith("/api/test-evidence/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiTestEvidence(feature, version)));
    }

    if (url.startsWith("/api/review/")) {
      const { feature, version } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiReviewRecord(feature, version)));
    }

    if (url.startsWith("/api/archive-check/")) {
      const { feature } = parseFeatureVersion(url);
      return res.end(JSON.stringify(apiArchiveCheck(feature)));
    }

    if (url.startsWith("/api/file")) {
      const params = new URL(url, "http://localhost").searchParams;
      return res.end(
        JSON.stringify(apiFile(params.get("path"), params.get("check") === "1"))
      );
    }

    if (url === "/api/commands") {
      return res.end(JSON.stringify(apiCommands()));
    }

    res.writeHead(404);
    return res.end(JSON.stringify({ error: "Not found" }));
  }

  if (url === "/" || url === "/index.html") {
    return serveStatic(res, path.join(__dirname, "index.html"), "text/html");
  }

  // Path-based page routes. Each dashboard view is its own URL (e.g.
  // /product-plan, /features, /team). They all serve the same single-page shell;
  // the client router in app.js decides which section to display.
  const PAGE_ROUTES = ["/overview", "/product-plan", "/features", "/quality", "/team", "/activity", "/commands", "/skills"];
  if (PAGE_ROUTES.includes(url)) {
    return serveStatic(res, path.join(__dirname, "index.html"), "text/html");
  }

  // Dynamic feature detail route: /features/<name>. Serves the same shell;
  // the client router renders the detail page for the named feature.
  if (/^\/features\/[^/]+\/?$/.test(url)) {
    return serveStatic(res, path.join(__dirname, "index.html"), "text/html");
  }

  if (url === "/style.css") {
    return serveStatic(res, path.join(__dirname, "style.css"), "text/css");
  }

  if (url === "/app.js") {
    return serveStatic(res, path.join(__dirname, "app.js"), "application/javascript");
  }

  if (url === "/theme.js") {
    return serveStatic(res, path.join(__dirname, "theme.js"), "application/javascript");
  }

  if (url === "/logo.png") {
    const logoCandidates = [
      path.join(__dirname, "logo.png"),
      path.join(sdriveRoot, "logo.png"),
      path.join(projectRoot, "logo.png"),
      path.join(__dirname, "..", "logo.png"),
      path.join(__dirname, "..", "..", "logo.png")
    ];
    const logoPath = logoCandidates.find((candidate) => fs.existsSync(candidate));
    if (logoPath) {
      return serveStatic(res, logoPath, "image/png");
    }
  }

  res.writeHead(404);
  res.end("Not found");
}

// Preferred port, overridable via SDRIVE_PORT. If it is taken (for example
// another project's dashboard is already running), we walk forward to the next
// free port so multiple projects can each serve their own dashboard.
const PREFERRED_PORT = Number(process.env.SDRIVE_PORT) || 4747;
const MAX_PORT_ATTEMPTS = 20;

// Loopback by default. The dashboard has no authentication, so binding it to
// every interface (the old `listen(port)` behaviour) exposed project specs and
// source files to the local network.
const PREFERRED_HOST = process.env.SDRIVE_HOST || "127.0.0.1";
const LOOPBACK_HOSTS = new Set(["127.0.0.1", "::1", "localhost"]);

function isLoopback(host) {
  return LOOPBACK_HOSTS.has(host);
}

// A fresh server is created for every attempt. Reusing one server across
// listen() calls leaves stale "listening" callbacks queued, which makes the
// success message print once per failed attempt.
function listenOnFreePort(port, attemptsLeft, host = PREFERRED_HOST) {
  const server = http.createServer(handleRequest);

  server.once("error", (error) => {
    if (error.code === "EADDRINUSE" && attemptsLeft > 0) {
      console.log(`⚠️  Port ${port} is in use, trying ${port + 1}...`);
      listenOnFreePort(port + 1, attemptsLeft - 1, host);
      return;
    }

    console.error(`\n❌ Could not start the dashboard: ${error.message}\n`);
    process.exit(1);
  });

  server.listen(port, host, () => {
    const shown = isLoopback(host) ? "localhost" : host;
    console.log(`\n🚀 SpecDrive Dashboard running at http://${shown}:${port}\n`);

    if (!isLoopback(host)) {
      console.warn(
        `⚠️  Bound to ${host} with no authentication. Anyone who can reach this port can read project files.`
      );
      console.warn("   Use 127.0.0.1 (the default) unless you are on a trusted network.\n");
    }
  });
}

if (!isLoopback(PREFERRED_HOST)) {
  console.warn(`⚠️  SDRIVE_HOST=${PREFERRED_HOST}: the dashboard is reachable beyond this machine.`);
}

listenOnFreePort(PREFERRED_PORT, MAX_PORT_ATTEMPTS, PREFERRED_HOST);