// Product-plan parsing and lifecycle mapping.
//
// The backlog is hand-written Markdown. This module keeps one implementation of
// the table parser, dependency/wave annotation, and the mapping from a backlog
// item (`BP-###`) to the feature that is actually working on it. Both the
// dashboard and `scripts/worktree.js` use it, so the page, the command, and the
// tests cannot disagree about what is ready.
const fs = require("fs");
const path = require("path");

const BP_TOKEN = /\bBP-(\d{3})\b/gi;
const LIFECYCLE_RANK = { backlog: 0, completed: 1, ongoing: 2 };

function normalizeId(value) {
  const match = String(value == null ? "" : value).match(/\bBP-(\d{3})\b/i);
  return match ? `BP-${match[1]}` : null;
}

function normalizeIds(value) {
  if (Array.isArray(value)) {
    return [...new Set(value.map(normalizeId).filter(Boolean))];
  }
  return [...new Set(String(value == null ? "" : value).match(BP_TOKEN) || [])].map((id) =>
    id.toUpperCase()
  );
}

// Expand "BP-001 to BP-004" as well as a comma-separated list. This is used for
// dependency cells and for feature scope lines, where a milestone is often
// written as a range.
function expandIds(text) {
  const source = String(text == null ? "" : text).replace(/[\u2013\u2014]/g, "-");
  const ids = [];
  const add = (id) => {
    if (id && !ids.includes(id)) ids.push(id);
  };

  let rest = source.replace(
    /\bBP-(\d{3})\s*(?:to|through)\s*BP-(\d{3})\b/gi,
    (_match, from, to) => {
      const start = Number(from);
      const end = Number(to);
      const step = start <= end ? 1 : -1;
      for (let n = start; step > 0 ? n <= end : n >= end; n += step) {
        add(`BP-${String(n).padStart(3, "0")}`);
      }
      return " ";
    }
  );

  rest = rest.replace(BP_TOKEN, (match) => {
    add(match.toUpperCase());
    return " ";
  });

  return ids;
}

function parseBacklog(markdown) {
  if (!markdown) return [];

  const lines = markdown.split(/\r?\n/);
  const items = [];
  let section = "";
  let columns = null;

  for (const line of lines) {
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    if (heading) {
      section = heading[1].trim();
      columns = null;
      continue;
    }

    const trimmed = line.trim();
    if (!trimmed.startsWith("|")) {
      if (!trimmed) columns = null;
      continue;
    }

    if (/^\|[\s:|-]+\|$/.test(trimmed)) continue;

    const cells = trimmed
      .split("|")
      .slice(1, -1)
      .map((cell) => cell.trim());

    if (!cells.length) continue;

    const header = cells.map((cell) => cell.toLowerCase());
    const idIndex = header.findIndex((cell) => /^id$|feature id|backlog id/.test(cell));
    const featureIndex = header.findIndex((cell) => /^feature$|^candidate$|^title$/.test(cell));

    // A table is a backlog table only if it has both an ID and a Feature
    // column. Summary and legend tables are documentation, not candidates.
    if (idIndex !== -1 && featureIndex !== -1) {
      columns = {
        id: idIndex,
        feature: featureIndex,
        details: header.findIndex((cell) => /^details?$|^description$|what it does|^summary$/.test(cell)),
        milestone: header.findIndex((cell) => /milestone|phase/.test(cell)),
        userValue: header.findIndex((cell) => /user value|value|benefit/.test(cell)),
        priority: header.findIndex((cell) => /priority/.test(cell)),
        dependsOn: header.findIndex((cell) => /depends|dependency|dependencies|prerequisite/.test(cell)),
        kind: header.findIndex((cell) => /^kind$|item kind|work kind/.test(cell)),
        batch: header.findIndex((cell) => /^batch$|branch batch|bundle/.test(cell)),
        status: header.findIndex((cell) => /^status$|state/.test(cell)),
        confidence: header.findIndex((cell) => /confidence|source/.test(cell))
      };
      continue;
    }

    if (!columns) continue;

    const id = normalizeId(cells[columns.id]);
    if (!id) continue;

    const valueAt = (index) => (index >= 0 && index < cells.length ? cells[index] : "");
    const depends = parseDependencyCell(valueAt(columns.dependsOn));

    items.push({
      id,
      feature: valueAt(columns.feature) || "",
      details: valueAt(columns.details),
      milestone: valueAt(columns.milestone) || section || "",
      userValue: valueAt(columns.userValue),
      priority: valueAt(columns.priority),
      dependsOn: depends.ids,
      externalDependencies: depends.external,
      // Optional `Kind` column: build | decision | release. A decision has no test to pass
      // and a release cannot run before the build lands, so `[w2 p1 build]` can exclude both
      // before a spec exists.
      kind: (valueAt(columns.kind) || "").toLowerCase(),
      // Optional `Batch` column (`b1`, `b2`, ...): the items that land together on one
      // branch. W (wave) is derived from dependencies; B is a promise a human makes, so it
      // is declared here rather than computed.
      batch: (valueAt(columns.batch) || "").toLowerCase(),
      status: (valueAt(columns.status) || "backlog").toLowerCase(),
      confidence: valueAt(columns.confidence),
      section
    });
  }

  const seen = new Set();
  return items.filter((item) => {
    if (seen.has(item.id)) return false;
    seen.add(item.id);
    return true;
  });
}

// A dependency cell may mix feature IDs with outside blockers:
//   "BP-001; partner access" -> ids ["BP-001"], external ["partner access"]
//   "BP-002 to BP-003"       -> ids ["BP-002", "BP-003"]
function parseDependencyCell(raw) {
  const text = String(raw == null ? "" : raw).trim();
  if (!text || /^[-—–]+$/.test(text)) return { ids: [], external: [] };

  const ids = [];
  const external = [];
  const addId = (id) => {
    if (!ids.includes(id)) ids.push(id);
  };

  // Protect feature ranges before splitting on separators.
  const withPlaceholders = text.replace(
    /\bBP-(\d{3})\s*(?:to|through)\s*BP-(\d{3})\b/gi,
    (_match, from, to) => {
      const start = Number(from);
      const end = Number(to);
      const step = start <= end ? 1 : -1;
      for (let n = start; step > 0 ? n <= end : n >= end; n += step) {
        addId(`BP-${String(n).padStart(3, "0")}`);
      }
      return " ";
    }
  );

  for (const part of withPlaceholders.split(/[;,]/)) {
    const value = part.trim();
    if (!value) continue;

    const found = value.match(/\bBP-\d{3}\b/gi);
    if (found) {
      found.forEach((id) => addId(id.toUpperCase()));
      const remainder = value.replace(/\bBP-\d{3}\b/gi, "").replace(/[()]/g, "").trim();
      if (remainder && !/^[-—–]*$/.test(remainder)) external.push(remainder);
    } else {
      external.push(value);
    }
  }

  const cleanedExternal = external
    .map((value) => value.replace(/^[-–—\s]+|[-–—\s]+$/g, "").trim())
    .filter(Boolean);

  return { ids, external: [...new Set(cleanedExternal)] };
}

function annotateBacklog(items) {
  const byId = new Map(items.map((item) => [item.id, item]));
  const waveMemo = new Map();

  const waveOf = (id, visiting = new Set()) => {
    if (waveMemo.has(id)) return waveMemo.get(id);
    const item = byId.get(id);
    if (!item || visiting.has(id)) return 0;
    visiting.add(id);

    const dependencies = item.dependsOn || [];
    const wave = dependencies.length
      ? Math.max(...dependencies.map((dependency) => waveOf(dependency, visiting))) + 1
      : 0;

    visiting.delete(id);
    waveMemo.set(id, wave);
    return wave;
  };

  const annotated = items.map((item) => {
    const baseFor = items
      .filter((other) => (other.dependsOn || []).includes(item.id))
      .map((other) => other.id);
    const wave = waveOf(item.id);

    return {
      ...item,
      wave,
      parallelWith: items
        .filter(
          (other) =>
            other.id !== item.id &&
            waveOf(other.id) === wave &&
            !(other.dependsOn || []).includes(item.id) &&
            !(item.dependsOn || []).includes(other.id)
        )
        .map((other) => other.id),
      baseFor
    };
  });

  return annotated;
}

function readWorkflowState(sdriveRoot) {
  const file = path.join(sdriveRoot, "workflow-state.json");
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    return { features: [] };
  }
}

function featureStageVersion(feature, sdriveRoot) {
  const stages = ["ongoing", "completed", "backlog"];
  const versions = Array.isArray(feature.versions) ? feature.versions : [];
  const ids = versions.map((version) => version.id).filter(Boolean);
  if (feature.currentVersion && !ids.includes(feature.currentVersion)) {
    ids.unshift(feature.currentVersion);
  }
  if (!ids.length) ids.push("v1");

  for (const stage of stages) {
    for (const version of ids) {
      if (fs.existsSync(path.join(sdriveRoot, "specs", stage, feature.name, version))) {
        return stage;
      }
    }
  }
  return null;
}

function featureLifecycle(feature, sdriveRoot) {
  const versions = Array.isArray(feature.versions) ? feature.versions : [];
  const phases = versions.map((version) => String(version.phase || "").toLowerCase());
  const stage = featureStageVersion(feature, sdriveRoot);

  if (stage === "ongoing" || phases.some((phase) => /ongoing|in_progress|blocked|active/.test(phase))) {
    return "ongoing";
  }
  if (stage === "completed" || phases.some((phase) => /completed|archived|released/.test(phase))) {
    return "completed";
  }
  return "backlog";
}

function addIdsFromText(ids, text) {
  for (const id of expandIds(text)) ids.add(id);
}

function featureProductIds(feature, sdriveRoot) {
  const ids = new Set();

  // New /propose runs record this explicitly. Keep the accepted spellings
  // because older specs used different names.
  for (const key of [
    "productItems",
    "productItem",
    "backlogItems",
    "backlogItem",
    "sourceItems",
    "sourceItem",
    "backlogId",
    "productId"
  ]) {
    const value = feature[key];
    if (Array.isArray(value)) value.forEach((entry) => addIdsFromText(ids, entry));
    else if (value) addIdsFromText(ids, value);
  }

  // Existing projects predate the explicit field. Their scope is still written
  // down in the feature history or in the spec's scope/confirmed-decision rows.
  const versions = Array.isArray(feature.versions) ? feature.versions : [];
  for (const version of versions) {
    for (const event of version.history || []) {
      const text = String(event.event || "");
      if (/\bBP-\d{3}\b/i.test(text) && /scope|promot|source|backlog/i.test(text)) {
        addIdsFromText(ids, text);
      }
    }
  }

  const versionIds = versions.map((version) => version.id).filter(Boolean);
  if (feature.currentVersion && !versionIds.includes(feature.currentVersion)) {
    versionIds.unshift(feature.currentVersion);
  }
  if (!versionIds.length) versionIds.push("v1");

  for (const version of versionIds) {
    for (const stage of ["backlog", "ongoing", "completed"]) {
      const file = path.join(sdriveRoot, "specs", stage, feature.name, version, "spec.md");
      if (!fs.existsSync(file)) continue;

      let markdown = "";
      try {
        markdown = fs.readFileSync(file, "utf8");
      } catch (error) {
        continue;
      }

      for (const line of markdown.split(/\r?\n/)) {
        const trimmed = line.trim();
        if (!trimmed) continue;

        // Explicit frontmatter or table rows are the strongest signal.
        if (
          /^(product-item|product-items|backlog-item|backlog-items|source-item|source-items|backlog-id|product-id)\s*:/i.test(
            trimmed
          )
        ) {
          addIdsFromText(ids, trimmed);
          continue;
        }

        if (/\|\s*(feature scope|scope)\s*\|/i.test(trimmed)) {
          addIdsFromText(ids, trimmed);
          continue;
        }

        if (/backlog records this/i.test(trimmed)) {
          addIdsFromText(ids, trimmed);
          continue;
        }

        if (/one feature and one branch for/i.test(trimmed)) {
          addIdsFromText(ids, trimmed);
          continue;
        }

        if (/^#\s+.*\bBP-\d{3}\b/i.test(trimmed)) {
          addIdsFromText(ids, trimmed);
        }
      }
    }
  }

  return [...ids];
}

function collectFeatureLinks(state, sdriveRoot) {
  const links = new Map();

  const put = (id, link) => {
    const previous = links.get(id);
    if (!previous) {
      links.set(id, link);
      return;
    }

    // A prepared worktree is evidence that work has started, but it must never
    // downgrade a completed feature or replace the feature's recorded branch.
    if (link.prepared) {
      if (previous.status === "backlog") {
        links.set(id, {
          ...previous,
          status: "ongoing",
          featureName: previous.featureName || link.featureName,
          branch: previous.branch || link.branch,
          base: previous.base || link.base,
          worktreePath: link.worktreePath,
          prepared: true
        });
      } else if (previous.status === "ongoing" && link.worktreePath && !previous.worktreePath) {
        links.set(id, { ...previous, worktreePath: link.worktreePath });
      }
      return;
    }

    const previousRank = LIFECYCLE_RANK[previous.status] || 0;
    const nextRank = LIFECYCLE_RANK[link.status] || 0;
    if (nextRank > previousRank) {
      links.set(id, { ...previous, ...link });
    } else if (nextRank === previousRank && link.worktreePath && !previous.worktreePath) {
      links.set(id, { ...previous, worktreePath: link.worktreePath });
    }
  };

  for (const feature of state.features || []) {
    if (!feature || !feature.name) continue;
    const status = featureLifecycle(feature, sdriveRoot);
    const productIds = featureProductIds(feature, sdriveRoot);
    for (const id of productIds) {
      put(id, {
        featureName: feature.name,
        status,
        branch: feature.branch || null,
        base: feature.base || null,
        worktreePath: null,
        prepared: false
      });
    }
  }

  for (const worktree of state.worktrees || []) {
    if (!worktree) continue;
    const ids = normalizeIds(
      worktree.item || worktree.items || worktree.productItem || worktree.backlogId || []
    );
    for (const id of ids) {
      put(id, {
        featureName: worktree.featureName || worktree.feature || null,
        status: "ongoing",
        branch: worktree.branch || null,
        base: worktree.base || null,
        worktreePath: worktree.path || null,
        prepared: true
      });
    }
  }

  return links;
}

function countStatuses(items) {
  const counts = { backlog: 0, ongoing: 0, completed: 0 };
  for (const item of items) counts[item.lifecycleStatus] += 1;
  return counts;
}

function enrichBacklog(items, { sdriveRoot, state } = {}) {
  const workflow = state || readWorkflowState(sdriveRoot);
  const links = collectFeatureLinks(workflow, sdriveRoot);
  const statusById = new Map();

  for (const item of items) {
    const link = links.get(item.id);
    statusById.set(item.id, link ? link.status : "backlog");
  }

  const enriched = items.map((item) => {
    const link = links.get(item.id) || null;
    const lifecycleStatus = link ? link.status : "backlog";
    const blockedBy = (item.dependsOn || []).filter(
      (dependency) => statusById.get(dependency) !== "completed"
    );
    const external = item.externalDependencies || [];
    const planStatus = String(item.status || "backlog").toLowerCase();
    const deliberatelyParked = /deferred|needs discovery/.test(planStatus);
    const hasFeature = Boolean(link && link.featureName);
    const hasWorktree = Boolean(link && link.worktreePath);

    return {
      ...item,
      planStatus,
      lifecycleStatus,
      featureName: link ? link.featureName : null,
      branch: link ? link.branch : null,
      base: link ? link.base : null,
      worktreePath: link ? link.worktreePath : null,
      prepared: Boolean(link && link.prepared),
      blockedBy,
      eligible:
        lifecycleStatus === "backlog" &&
        !hasFeature &&
        !hasWorktree &&
        !deliberatelyParked &&
        blockedBy.length === 0 &&
        external.length === 0
    };
  });

  const counts = countStatuses(enriched);
  const waveMap = new Map();
  for (const item of enriched) {
    if (!waveMap.has(item.wave)) {
      waveMap.set(item.wave, {
        wave: item.wave,
        label: waveLabel(item.wave),
        total: 0,
        backlog: 0,
        ongoing: 0,
        completed: 0,
        eligible: 0
      });
    }
    const entry = waveMap.get(item.wave);
    entry.total += 1;
    entry[item.lifecycleStatus] += 1;
    if (item.eligible) entry.eligible += 1;
  }

  return {
    items: enriched,
    counts,
    waves: [...waveMap.values()].sort((a, b) => a.wave - b.wave)
  };
}

function waveLabel(wave) {
  return Number(wave) === 0 ? "start" : `W${Number(wave) || 0}`;
}

function milestoneRank(milestone) {
  const match = String(milestone || "").match(/\bM(\d+)\b/i);
  return match ? Number(match[1]) : 999;
}

function priorityRank(priority) {
  const match = String(priority || "").match(/\bP(\d+)\b/i);
  return match ? Number(match[1]) : 99;
}

function sortBacklog(items) {
  return [...items].sort(
    (a, b) =>
      (a.wave || 0) - (b.wave || 0) ||
      milestoneRank(a.milestone) - milestoneRank(b.milestone) ||
      priorityRank(a.priority) - priorityRank(b.priority) ||
      String(a.id).localeCompare(String(b.id), undefined, { numeric: true })
  );
}

function slugify(value, maxLength = 40) {
  return String(value == null ? "" : value)
    .toLowerCase()
    .replace(/[`*_]/g, "")
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, maxLength)
    .replace(/-+$/g, "");
}

function suggestBranchNames(item) {
  const feature = String(item && item.feature ? item.feature : "").trim();
  if (!feature) return [];

  const firstClause = feature.split(/[:;—–]|\s+-\s+/)[0];
  const words = slugify(feature).split("-").filter(Boolean);
  const candidates = [
    slugify(feature, 42),
    slugify(firstClause, 34),
    words.slice(0, 4).join("-"),
    words.slice(0, 5).join("-")
  ];

  return [...new Set(candidates.filter(Boolean))].slice(0, 3).map((name) => `feature/${name}`);
}

function branchSlug(branch) {
  return String(branch || "")
    .replace(/^feature\//i, "")
    .replace(/[^A-Za-z0-9._-]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .toLowerCase();
}

module.exports = {
  annotateBacklog,
  branchSlug,
  collectFeatureLinks,
  countStatuses,
  enrichBacklog,
  expandIds,
  featureLifecycle,
  featureProductIds,
  milestoneRank,
  normalizeId,
  normalizeIds,
  parseBacklog,
  parseDependencyCell,
  priorityRank,
  readWorkflowState,
  slugify,
  sortBacklog,
  suggestBranchNames,
  waveLabel
};
