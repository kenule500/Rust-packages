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
const group = require("./lib/group");

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

// Git prints worktree paths with forward slashes even on Windows, and the recorded path was
// built with `path.join`, so comparing them raw would report every existing worktree as
// missing. `realpath` matters just as much: Node's temp directory arrives as a Windows 8.3
// short path (`C:\Users\KENULE~1\...`) while git reports the long form, and those are the same
// directory. Resolving both sides is what makes them comparable; when a directory is gone the
// plain absolute path is all there is, which is exactly the case `present: false` describes.
function normalizePath(value) {
  const resolved = path.resolve(String(value || ""));

  let canonical = resolved;
  try {
    canonical = fs.realpathSync.native(resolved);
  } catch (error) {
    // Missing directory: compare the resolved path.
  }

  return canonical.replace(/\\/g, "/").toLowerCase();
}

function registeredWorktreePaths() {
  const listing = tryGit(["worktree", "list", "--porcelain"]);
  if (!listing) return new Set();

  return new Set(
    listing
      .split("\n")
      .filter((line) => line.startsWith("worktree "))
      .map((line) => normalizePath(line.slice("worktree ".length).trim()))
  );
}

// One record describes one BRANCH. `items` holds every Product Plan item that branch carries;
// `item` is the field written before shared branches existed, so it is still read - otherwise
// old records would stop matching their items.
function worktreeIds(record) {
  const listed =
    record && Array.isArray(record.items) && record.items.length ? record.items : null;

  return productPlan.normalizeIds(listed || (record && record.item) || []);
}

// The parsed backlog rows are the only place `kind`, `batch`, `gated`, and the
// dependency-derived wave carry the same values `/sdrive:propose` sees, so topology candidates
// are built from them rather than from the eligibility view, which exists to be shown.
//
// Annotated on purpose: `rowToCandidate` reads `row.wave`, and without the annotation that is
// empty, which made `waveValue` fall back to the milestone number - a different column, so a
// group whose members sit under two milestone headings looked like two waves.
function candidatesFor(ids) {
  const wanted = new Set((ids || []).map((id) => productPlan.normalizeId(id)).filter(Boolean));

  return productPlan
    .annotateBacklog(productPlan.parseBacklog(readBacklog()))
    .filter((row) => wanted.has(productPlan.normalizeId(row.id)))
    .map((row) => group.rowToCandidate(row));
}

function branchOptionsFor(ids) {
  return group.branchOptions(candidatesFor(ids));
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

  // `git worktree list` is the only authority on what is actually checked out; the records in
  // workflow-state.json are a history of what this command prepared.
  const registered = registeredWorktreePaths();
  const readyNow =
    currentWave == null ? [] : eligible.filter((item) => item.wave === currentWave);

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
    // The branch-topology question, answered for the ready wave: the only set the user can act
    // on now. `options` are the groupings that can reach archive; `rejected` says why an
    // obvious one is missing, so the agent never has to invent or withhold a reason.
    topologies:
      currentWave == null ? null : branchOptionsFor(readyNow.map((item) => item.id)),
    worktrees: (state.worktrees || []).map((record) => ({
      ...record,
      // A record outlives the directory it describes: the folder was deleted by hand, or
      // `git worktree prune` ran. Saying so beats reporting a worktree that is not there.
      present: Boolean(
        record &&
          record.path &&
          registered.has(normalizePath(record.path)) &&
          fs.existsSync(record.path)
      )
    }))
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

// Written through a temporary file: a reader that catches a half-written state file would
// treat it as invalid JSON and fall back to "no features", which looks like data loss.
function writeState(state) {
  const file = path.join(sdriveRoot, "workflow-state.json");
  const temporary = `${file}.tmp`;
  fs.mkdirSync(sdriveRoot, { recursive: true });
  fs.writeFileSync(temporary, `${JSON.stringify(state, null, 2)}\n`, "utf8");
  fs.renameSync(temporary, file);
}

// `.sdrive/.state.lock` is the convention `src/lib/state.js` already uses, and the file is
// gitignored, so a project-local helper can share it with the CLI's own writes instead of
// inventing a second lock. It is advisory: it stops two SpecDrive processes from racing on
// workflow-state.json, not another tool.
function withStateLock(fn, options = {}) {
  const { timeoutMs = 15000, staleMs = 30000 } = options;
  const lockPath = path.join(sdriveRoot, ".state.lock");
  const startedAt = Date.now();
  fs.mkdirSync(sdriveRoot, { recursive: true });

  for (;;) {
    try {
      // Created exclusively, written through the descriptor, then closed: `writeFileSync` given
      // an fd returns nothing, so the descriptor has to be closed explicitly.
      const fd = fs.openSync(lockPath, "wx");
      try {
        fs.writeFileSync(fd, String(process.pid));
      } finally {
        fs.closeSync(fd);
      }
      break;
    } catch (error) {
      if (error.code !== "EEXIST") throw error;

      let stale = false;
      try {
        stale = Date.now() - fs.statSync(lockPath).mtimeMs > staleMs;
      } catch (statError) {
        stale = false;
      }

      // A process that died holding the lock must not block this command forever.
      if (stale) {
        try {
          fs.unlinkSync(lockPath);
        } catch (unlinkError) {
          // Another process released or took it between the stat and the unlink.
        }
        continue;
      }

      if (Date.now() - startedAt > timeoutMs) {
        fail(
          "Another SpecDrive process is holding .sdrive/.state.lock. Wait for it to finish, or delete the lock file if nothing is running."
        );
      }

      // The lock is held for milliseconds, so a short sleep is cheaper than a dependency.
      Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 100);
    }
  }

  try {
    return fn();
  } finally {
    try {
      fs.unlinkSync(lockPath);
    } catch (error) {
      // Already released; the state write is what mattered.
    }
  }
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

  // Surfaced before creation, not after: this is the question the agent must ask. Options are
  // grouped by what can finish; `rejected` is shown too, because "why not one branch for all"
  // deserves an answer rather than a shrug.
  const topologies = plan.topologies;
  if (topologies && topologies.options.length) {
    console.log("");
    console.log(`Branch topologies for ${wave}:`);
    for (const candidate of topologies.options) {
      console.log(`  ${candidate.label}`);
      if (candidate.note) console.log(`        ${candidate.note}`);
    }
    for (const blocked of topologies.rejected) {
      console.log(`  not offered: ${blocked.label}`);
      for (const reason of blocked.reasons) console.log(`        ${reason}`);
    }
    for (const skip of topologies.excluded) {
      console.log(`  not a branch option: ${skip.item} (${skip.kind})`);
      console.log(`        ${skip.reason}`);
    }
  }

  console.log("");
  console.log("Prepared worktrees:");
  if (!plan.worktrees.length) console.log("  none");
  else {
    for (const worktree of plan.worktrees) {
      const carried = worktreeIds(worktree).join(", ") || worktree.item || "?";
      const missing = worktree.present ? "" : "  [missing: the directory is gone]";
      console.log(`  ${carried}  ${worktree.branch}  ${worktree.path}${missing}`);
    }
  }
}

// `--items BP-003,BP-004` (or repeated) states a branch's membership explicitly. It exists
// because every viable topology has to be expressible: a grouping that is neither a declared
// batch nor the whole wave still needs to be creatable, otherwise the agent would cut branches
// one at a time and pretend they are one.
function itemIdsFromArgs() {
  const values = [];

  for (let index = 0; index < process.argv.length; index += 1) {
    if (process.argv[index] !== "--items") continue;
    const value = process.argv[index + 1];
    if (!value || value.startsWith("--")) fail("--items needs at least one BP-### id.");
    values.push(...String(value).split(/[\s,]+/).filter(Boolean));
  }

  const single = option("--item", "");
  if (single) values.push(...String(single).split(/[\s,]+/).filter(Boolean));

  const ids = [];
  for (const value of values) {
    const id = productPlan.normalizeId(value);
    if (!id) fail(`"${value}" is not a product item id. Use BP-###.`);
    if (!ids.includes(id)) ids.push(id);
  }

  return ids;
}

// Reported, never run. Guessing a package manager is exactly the decision that belongs to the
// person who owns the repository, and a worktree is a fresh checkout with no `node_modules`.
function bootstrapHint(worktreePath) {
  const lockfiles = [
    ["pnpm-lock.yaml", "pnpm install --frozen-lockfile"],
    ["yarn.lock", "yarn install --frozen-lockfile"],
    ["package-lock.json", "npm ci"],
    ["bun.lockb", "bun install --frozen-lockfile"],
    ["Pipfile.lock", "pipenv sync --dev"],
    ["poetry.lock", "poetry install"],
    ["go.sum", "go mod download"],
    ["Cargo.lock", "cargo fetch"]
  ];
  const match = lockfiles.find(([name]) => fs.existsSync(path.join(worktreePath, name)));

  return match ? `install its dependencies first (fresh checkout): ${match[1]}` : null;
}

function createCommand() {
  const branchInput = option("--branch", "");
  const base = option("--base", "");
  const customPath = option("--path", "");
  const json = process.argv.includes("--json");
  const dryRun = process.argv.includes("--dry-run");

  const ids = itemIdsFromArgs();
  if (!ids.length) fail("Missing --item BP-### (or --items BP-003,BP-004).");
  if (!branchInput) fail("Missing --branch feature/<name>.");

  const plan = buildPlan();
  // Every id is resolved before git is touched: a bad second id must not leave half a branch
  // behind, so either the whole selection is ready or nothing is created.
  const selected = ids.map((id) => {
    const item = plan.eligible.find((entry) => entry.id === id);
    if (item) return item;

    const known = [...plan.eligible, ...plan.unavailable].find((entry) => entry.id === id);
    const reason = known ? known.reason : "item not found in the product backlog";
    fail(`${id} is not ready: ${reason}.`);
  });

  // Wave is build order: start, W1, W2, ... An item in a later wave is only
  // eligible once every item in the current wave has been started. Without this
  // guard a dependency-complete W2 item could be cut while start-wave work is
  // still untouched, which is exactly the ordering the plan encodes.
  for (const item of selected) {
    if (plan.currentWave != null && item.wave > plan.currentWave) {
      fail(
        `${item.id} is in ${productPlan.waveLabel(item.wave)} but the current wave is ${plan.currentWaveLabel}. ` +
          `Finish ${plan.currentWaveLabel} first.`
      );
    }
  }

  // One branch carries items that become ready together, so its members must share a wave:
  // a branch spanning two waves cannot merge until the later one starts, and the earlier
  // members would sit unmerged meanwhile.
  const waves = [...new Set(selected.map((item) => item.wave))];
  if (waves.length > 1) {
    fail(
      `${ids.join(", ")} span ${waves.map((wave) => productPlan.waveLabel(wave)).join(", ")}. ` +
        "One branch carries items that become ready together, so its members must share a wave."
    );
  }

  // The verdict the option list is built from, applied to an explicit selection: `--items`
  // cannot smuggle in a group the agent was told not to offer.
  if (selected.length > 1) {
    const verdict = group.checkBatch({ matched: candidatesFor(ids) });
    if (!verdict.ok) {
      fail(`One branch for ${ids.join(", ")} cannot finish: ${verdict.blockers.join(" ")}`);
    }
  }

  const branch = normalizeBranch(branchInput);
  if (branchExists(branch)) {
    fail(`Branch "${branch}" already exists. Choose another name, or use a -v2 suffix.`);
  }

  const baseBranch = base || plan.integrationBranch;
  if (!tryGit(["rev-parse", "--verify", `${baseBranch}^{commit}`])) {
    fail(`Base branch "${baseBranch}" does not exist.`);
  }

  const slug = productPlan.branchSlug(branch) || ids[0].toLowerCase();
  const worktreePath = customPath
    ? path.resolve(projectRoot, customPath)
    : path.join(path.dirname(projectRoot), `${path.basename(projectRoot)}-worktrees`, slug);

  if (fs.existsSync(worktreePath)) {
    fail(`Worktree path already exists: ${worktreePath}`);
  }

  const state = productPlan.readWorkflowState(sdriveRoot);
  // One record per BRANCH, so any item it already carries makes the whole selection a
  // duplicate - checking only the first id would let `--items BP-003,BP-004` pick up an item
  // whose branch is already prepared under a different name.
  const wanted = new Set(ids);
  const duplicate = (state.worktrees || []).find(
    (worktree) =>
      worktree.branch === branch || worktreeIds(worktree).some((id) => wanted.has(id))
  );
  if (duplicate) {
    const carried = worktreeIds(duplicate).filter((id) => wanted.has(id));
    fail(
      carried.length
        ? `${carried.join(", ")} already has a prepared worktree at ${duplicate.path} (${duplicate.branch}).`
        : `Branch ${branch} is already recorded at ${duplicate.path}.`
    );
  }

  const now = new Date().toISOString();
  const record = {
    // `item` stays the first id so a reader written before shared branches existed still sees
    // something it understands; `items` is what every reader now expands.
    item: ids[0],
    items: ids,
    branch,
    base: baseBranch,
    path: worktreePath,
    title: selected.map((entry) => entry.feature).join(" + "),
    createdAt: now,
    updatedAt: now,
    status: "prepared"
  };

  // One branch, one feature, one spec: the proposal covers every id the branch carries.
  const proposal = ids.length > 1 ? `[${ids.join(", ")}]` : `${ids[0]} ${selected[0].feature}`;

  if (dryRun) {
    // Nothing is created and nothing is written. The agent shows this, asks, and only then runs
    // the real command - so the guard verdict is visible before any repository state changes.
    if (json) {
      console.log(
        JSON.stringify(
          { ok: true, dryRun: true, ...record, next: `/sdrive:propose "${proposal}"` },
          null,
          2
        )
      );
      return;
    }

    console.log(`Would prepare ${ids.join(", ")}: ${record.title}`);
    console.log(`   Worktree: ${worktreePath}`);
    console.log(`   Branch:   ${branch}`);
    console.log(`   Base:     ${baseBranch}`);
    return;
  }

  fs.mkdirSync(path.dirname(worktreePath), { recursive: true });

  let created = false;
  try {
    // Held across the git command and the state write: two runs at once would otherwise both
    // pass the duplicate guard and record the same branch twice.
    withStateLock(() => {
      git(["worktree", "add", "-b", branch, worktreePath, baseBranch]);
      created = true;

      const latest = productPlan.readWorkflowState(sdriveRoot);
      latest.worktrees = Array.isArray(latest.worktrees) ? latest.worktrees : [];
      // A run that finished between the guard and this line must not leave two records for one
      // branch: the record is the branch, so a later write replaces the earlier one.
      latest.worktrees = latest.worktrees.filter((entry) => entry.branch !== branch);
      latest.worktrees.push(record);
      writeState(latest);
      copyStateToWorktree(worktreePath);
    });

    if (json) {
      console.log(
        JSON.stringify(
          { ok: true, item: ids[0], items: ids, ...record, next: `/sdrive:propose "${proposal}"` },
          null,
          2
        )
      );
    } else {
      console.log(`✅ Prepared ${ids.join(", ")}: ${record.title}`);
      console.log(`   Worktree: ${worktreePath}`);
      console.log(`   Branch:   ${branch}`);
      console.log(`   Base:     ${baseBranch}`);
      console.log("");
      console.log("Note:");
      console.log("   .sdrive/workflow-state.json is tracked, and each worktree keeps its own copy.");
      console.log("   Commit it on one branch at a time. If two branches change it, the merge keeps");
      console.log("   every feature, worktree, and version entry; the newer updatedAt wins a clash.");
      console.log("");
      console.log("Next:");
      console.log(`   cd "${worktreePath}"`);
      const setup = bootstrapHint(worktreePath);
      if (setup) console.log(`   ${setup}`);
      console.log(`   /sdrive:propose "${proposal}"`);
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

// The missing half of `create`. Without it a directory deleted by hand leaves a record stuck at
// `prepared`, which makes its item permanently ineligible - the plan would say "worktree already
// prepared" about a worktree that does not exist, with no command to correct it.
function removeCommand() {
  const json = process.argv.includes("--json");
  const keepBranch = process.argv.includes("--keep-branch");
  const force = process.argv.includes("--force");
  const branchInput = option("--branch", "");
  const customPath = option("--path", "");
  const ids = itemIdsFromArgs();

  if (!ids.length && !branchInput && !customPath) {
    fail(
      "Name the worktree to remove: --item BP-003, --items BP-003,BP-004, --branch feature/x, or --path <dir>."
    );
  }

  const state = productPlan.readWorkflowState(sdriveRoot);
  const records = Array.isArray(state.worktrees) ? state.worktrees : [];
  const branch = branchInput ? normalizeBranch(branchInput) : null;
  const wanted = new Set(ids);
  const targetPath = customPath ? normalizePath(path.resolve(projectRoot, customPath)) : null;

  const record = records.find((entry) => {
    if (branch && entry.branch === branch) return true;
    if (targetPath && normalizePath(entry.path) === targetPath) return true;
    return worktreeIds(entry).some((id) => wanted.has(id));
  });

  if (!record) {
    fail("No prepared worktree matches that. Run `worktree.js list` to see the records.");
  }

  let removedPath = null;

  if (record.path && fs.existsSync(record.path)) {
    try {
      git(
        force
          ? ["worktree", "remove", "--force", record.path]
          : ["worktree", "remove", record.path]
      );
      removedPath = record.path;
    } catch (error) {
      // `git worktree remove` refuses a dirty tree on purpose: there may be uncommitted work in
      // there. Only --force overrides that, and only the user can decide it is disposable.
      const detail =
        error && error.stderr ? String(error.stderr).trim() : String(error.message || error);
      fail(
        `Could not remove ${record.path}: ${detail} Use --force only if its uncommitted work is disposable.`
      );
    }
  } else if (record.path) {
    // Already gone from disk. Clearing git's registration is what lets the branch be reused.
    tryGit(["worktree", "prune"]);
  }

  // The record is keyed by branch, so removing it cannot touch a different worktree's entry.
  withStateLock(() => {
    const latest = productPlan.readWorkflowState(sdriveRoot);
    latest.worktrees = (Array.isArray(latest.worktrees) ? latest.worktrees : []).filter(
      (entry) => entry.branch !== record.branch
    );
    writeState(latest);
  });

  let branchDeleted = false;
  let note = "";

  if (!keepBranch && record.branch && branchExists(record.branch)) {
    try {
      // `-d`, never `-D`: if the branch holds commits that are not in its base, deleting it
      // would lose them, and that refusal is information rather than an error to force past.
      git(["branch", "-d", record.branch]);
      branchDeleted = true;
    } catch (error) {
      note = `${record.branch} was kept: it holds commits that are not merged into ${record.base || "its base"}. Delete it once they land.`;
    }
  }

  const carried = worktreeIds(record);

  if (json) {
    console.log(
      JSON.stringify(
        {
          ok: true,
          removed: { items: carried, branch: record.branch, path: record.path },
          removedPath,
          branchDeleted,
          note
        },
        null,
        2
      )
    );
    return;
  }

  console.log(`✅ Removed the worktree for ${carried.join(", ") || record.branch}`);
  console.log(`   Path:   ${record.path}`);
  console.log(`   Branch: ${record.branch}${branchDeleted ? " (deleted)" : " (kept)"}`);
  if (note) console.log(`   ${note}`);
  console.log("");
  console.log(`${carried.join(", ")} can be prepared again with a fresh branch.`);
}

const command = process.argv[2];
if (command === "list") listCommand();
else if (command === "create") createCommand();
else if (command === "remove") removeCommand();
else {
  console.log("Usage:");
  console.log("  node .sdrive/scripts/worktree.js list [--json]");
  console.log(
    "  node .sdrive/scripts/worktree.js create --item BP-003 --branch feature/BP-003 [--base main] [--path <dir>] [--dry-run] [--json]"
  );
  console.log(
    "  node .sdrive/scripts/worktree.js create --items BP-003,BP-004 --branch feature/start [--base main] [--dry-run] [--json]"
  );
  console.log(
    "  node .sdrive/scripts/worktree.js remove (--item BP-003 | --items BP-003,BP-004 | --branch feature/x | --path <dir>) [--keep-branch] [--force] [--json]"
  );
  process.exit(command ? 1 : 0);
}
