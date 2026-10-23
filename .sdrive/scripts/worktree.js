#!/usr/bin/env node

// Deterministic helper for /sdrive:worktree.
//
// The agent asks the questions (which items, which naming style) and this script
// does the part that must be exact: decide what is ready, create the branch and
// worktree, and record the preparation in workflow-state.json. Keeping the git
// and eligibility rules here means a model cannot accidentally open a W2 branch
// while its W1 dependency is still ongoing.
const fs = require("fs");
const path = require("path");
const { execFileSync } = require("child_process");
const productPlan = require("./lib/product-plan");

const projectRoot = process.cwd();
const sdriveRoot = path.join(projectRoot, ".sdrive");

function fail(message, code = 1) {
  console.error(`❌ ${message}`);
  process.exit(code);
}

function git(args, cwd = projectRoot) {
  return execFileSync("git", args, {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"]
  }).trim();
}

function tryGit(args, cwd = projectRoot) {
  try {
    return git(args, cwd);
  } catch (error) {
    return null;
  }
}

function readBacklog() {
  const file = path.join(sdriveRoot, "product", "backlog.md");
  if (!fs.existsSync(file)) return "";
  return fs.readFileSync(file, "utf8");
}

function resolveIntegrationBranch() {
  const originHead = tryGit(["symbolic-ref", "refs/remotes/origin/HEAD", "--short"]);
  if (originHead && originHead.startsWith("origin/")) {
    return originHead.slice("origin/".length);
  }

  for (const candidate of ["main", "master", "trunk", "develop"]) {
    if (tryGit(["rev-parse", "--verify", `${candidate}^{commit}`])) return candidate;
  }

  return tryGit(["branch", "--show-current"]) || "main";
}

function buildPlan() {
  const state = productPlan.readWorkflowState(sdriveRoot);
  const enriched = productPlan.enrichBacklog(
    productPlan.annotateBacklog(productPlan.parseBacklog(readBacklog())),
    { sdriveRoot, state }
  );
  const items = productPlan.sortBacklog(enriched.items);

  const explain = (item) => {
    if (item.featureName) return `feature already exists (${item.featureName})`;
    if (item.worktreePath) return `worktree already prepared (${item.worktreePath})`;
    if (/deferred|needs discovery/.test(item.planStatus)) {
      return `plan status is ${item.planStatus}`;
    }
    if (item.blockedBy.length) return `depends on ${item.blockedBy.join(", ")}`;
    if ((item.externalDependencies || []).length) {
      return `outside blocker: ${item.externalDependencies.join(", ")}`;
    }
    if (item.lifecycleStatus === "ongoing") return "already ongoing";
    if (item.lifecycleStatus === "completed") return "already completed";
    return "not eligible";
  };

  const publicItem = (item) => ({
    id: item.id,
    feature: item.feature,
    milestone: item.milestone,
    priority: item.priority,
    wave: item.wave,
    waveLabel: productPlan.waveLabel(item.wave),
    lifecycleStatus: item.lifecycleStatus,
    planStatus: item.planStatus,
    dependsOn: item.dependsOn,
    externalDependencies: item.externalDependencies,
    blockedBy: item.blockedBy,
    eligible: item.eligible,
    reason: item.eligible ? "" : explain(item),
    suggestions: productPlan.suggestBranchNames(item)
  });

  const eligible = items.filter((item) => item.eligible).map(publicItem);
  const waiting = items
    .filter((item) => item.lifecycleStatus === "backlog" && !item.eligible)
    .map(publicItem);
  const unavailable = items.filter((item) => !item.eligible).map(publicItem);
  const currentWave = eligible.length
    ? Math.min(...eligible.map((item) => item.wave))
    : null;

  return {
    projectRoot,
    integrationBranch: resolveIntegrationBranch(),
    currentWave,
    currentWaveLabel: currentWave == null ? null : productPlan.waveLabel(currentWave),
    counts: enriched.counts,
    waves: enriched.waves,
    eligible,
    waiting,
    unavailable,
    worktrees: (state.worktrees || []).slice()
  };
}

function option(name, fallback) {
  const index = process.argv.indexOf(name);
  if (index === -1 || index + 1 >= process.argv.length) return fallback;
  return process.argv[index + 1];
}

function normalizeBranch(value) {
  const raw = String(value || "").trim();
  const branch = raw.startsWith("feature/") ? raw : `feature/${raw.replace(/^\/+/, "")}`;
  if (!/^feature\/[A-Za-z0-9][A-Za-z0-9._-]*$/.test(branch)) {
    fail(
      `Invalid branch "${raw}". Use the form feature/<name>, for example feature/BP-003 or feature/chat.`
    );
  }
  return branch;
}

function branchExists(branch) {
  return Boolean(tryGit(["rev-parse", "--verify", `refs/heads/${branch}`]));
}

function writeState(state) {
  const file = path.join(sdriveRoot, "workflow-state.json");
  fs.mkdirSync(sdriveRoot, { recursive: true });
  fs.writeFileSync(file, `${JSON.stringify(state, null, 2)}\n`, "utf8");
}

function copyStateToWorktree(worktreePath) {
  const source = path.join(sdriveRoot, "workflow-state.json");
  const destination = path.join(worktreePath, ".sdrive", "workflow-state.json");
  fs.mkdirSync(path.dirname(destination), { recursive: true });
  fs.copyFileSync(source, destination);
}

function listCommand() {
  const plan = buildPlan();
  if (process.argv.includes("--json")) {
    console.log(JSON.stringify(plan, null, 2));
    return;
  }

  console.log(`Project: ${plan.projectRoot}`);
  console.log(`Integration branch: ${plan.integrationBranch}`);
  console.log("");
  if (!plan.eligible.length) {
    console.log("No item is ready for a worktree right now.");
    const next = plan.waves.find((wave) => wave.eligible > 0);
    if (next) console.log(`Next ready wave: ${next.label}`);
    return;
  }

  const wave = plan.currentWaveLabel;
  console.log(`Ready now (${wave}):`);
  for (const item of plan.eligible.filter((entry) => entry.wave === plan.currentWave)) {
    console.log(`  ${item.id}  ${item.feature}`);
    console.log(`        branch choices: feature/${item.id} or ${item.suggestions.join(", ") || "a custom name"}`);
  }

  const later = plan.eligible.filter((entry) => entry.wave > plan.currentWave);
  if (later.length) {
    console.log("");
    console.log("Ready in a later wave (do not create these yet):");
    for (const item of later) {
      console.log(`  ${item.id}  ${item.waveLabel}  ${item.feature}`);
    }
  }

  console.log("");
  console.log("Prepared worktrees:");
  if (!plan.worktrees.length) console.log("  none");
  else {
    for (const worktree of plan.worktrees) {
      console.log(`  ${worktree.item}  ${worktree.branch}  ${worktree.path}`);
    }
  }
}

function createCommand() {
  const itemId = option("--item", "");
  const branchInput = option("--branch", "");
  const base = option("--base", "");
  const customPath = option("--path", "");
  const json = process.argv.includes("--json");

  if (!itemId) fail("Missing --item BP-###.");
  if (!branchInput) fail("Missing --branch feature/<name>.");

  const plan = buildPlan();
  const item = plan.eligible.find((entry) => entry.id === productPlan.normalizeId(itemId));
  if (!item) {
    const known = [...plan.eligible, ...plan.unavailable].find(
      (entry) => entry.id === productPlan.normalizeId(itemId)
    );
    const reason = known ? known.reason : "item not found in the product backlog";
    fail(`${productPlan.normalizeId(itemId) || itemId} is not ready: ${reason}.`);
  }

  // Wave is build order: start, W1, W2, ... An item in a later wave is only
  // eligible once every item in the current wave has been started. Without this
  // guard a dependency-complete W2 item could be cut while start-wave work is
  // still untouched, which is exactly the ordering the plan encodes.
  if (plan.currentWave != null && item.wave > plan.currentWave) {
    fail(
      `${item.id} is in ${productPlan.waveLabel(item.wave)} but the current wave is ${plan.currentWaveLabel}. ` +
        `Finish ${plan.currentWaveLabel} first.`
    );
  }

  const branch = normalizeBranch(branchInput);
  if (branchExists(branch)) {
    fail(`Branch "${branch}" already exists. Choose another name, or use a -v2 suffix.`);
  }

  const baseBranch = base || plan.integrationBranch;
  if (!tryGit(["rev-parse", "--verify", `${baseBranch}^{commit}`])) {
    fail(`Base branch "${baseBranch}" does not exist.`);
  }

  const slug = productPlan.branchSlug(branch) || item.id.toLowerCase();
  const worktreePath = customPath
    ? path.resolve(projectRoot, customPath)
    : path.join(path.dirname(projectRoot), `${path.basename(projectRoot)}-worktrees`, slug);

  if (fs.existsSync(worktreePath)) {
    fail(`Worktree path already exists: ${worktreePath}`);
  }

  const statePath = path.join(sdriveRoot, "workflow-state.json");
  const state = productPlan.readWorkflowState(sdriveRoot);
  const duplicate = (state.worktrees || []).find(
    (worktree) => worktree.item === item.id || worktree.branch === branch
  );
  if (duplicate) {
    fail(
      `${item.id} already has a prepared worktree at ${duplicate.path} (${duplicate.branch}).`
    );
  }

  fs.mkdirSync(path.dirname(worktreePath), { recursive: true });

  let created = false;
  try {
    git(["worktree", "add", "-b", branch, worktreePath, baseBranch]);
    created = true;

    const record = {
      item: item.id,
      branch,
      base: baseBranch,
      path: worktreePath,
      title: item.feature,
      createdAt: new Date().toISOString(),
      status: "prepared"
    };

    const latest = productPlan.readWorkflowState(sdriveRoot);
    latest.worktrees = Array.isArray(latest.worktrees) ? latest.worktrees : [];
    latest.worktrees.push(record);
    writeState(latest);
    copyStateToWorktree(worktreePath);

    if (json) {
      console.log(JSON.stringify({ ok: true, item: item.id, ...record, next: `/sdrive:propose "${item.id} ${item.feature}"` }, null, 2));
    } else {
      console.log(`✅ Prepared ${item.id}: ${item.feature}`);
      console.log(`   Worktree: ${worktreePath}`);
      console.log(`   Branch:   ${branch}`);
      console.log(`   Base:     ${baseBranch}`);
      console.log("");
      console.log("Next:");
      console.log(`   cd "${worktreePath}"`);
      console.log(`   /sdrive:propose "${item.id} ${item.feature}"`);
    }
  } catch (error) {
    if (created) {
      try {
        git(["worktree", "remove", "--force", worktreePath]);
      } catch (cleanupError) {
        // The worktree was created but recording state failed. Leave the path in
        // place and report both facts rather than deleting something the user
        // may already be looking at.
      }
    }
    const detail = error && error.stderr ? String(error.stderr).trim() : String(error.message || error);
    fail(`Could not prepare the worktree: ${detail}`);
  }
}

const command = process.argv[2];
if (command === "list") listCommand();
else if (command === "create") createCommand();
else {
  console.log("Usage:");
  console.log("  node .sdrive/scripts/worktree.js list [--json]");
  console.log("  node .sdrive/scripts/worktree.js create --item BP-003 --branch feature/BP-003 [--base main] [--path <dir>] [--json]");
  process.exit(command ? 1 : 0);
}
