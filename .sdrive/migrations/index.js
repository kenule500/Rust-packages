const path = require("path");
const adapters = require("./adapters");
const { exists, relative, slugify } = require("./lib/files");

const VERSION = "v1";

function listAdapters() {
  return adapters.map((adapter) => ({
    id: adapter.id,
    name: adapter.name,
    roots: adapter.roots,
    defaultLifecycle: adapter.defaultLifecycle || "backlog"
  }));
}

function getAdapter(id) {
  return adapters.find((adapter) => adapter.id === id) || null;
}

function detectAll(root) {
  return adapters.map((adapter) => {
    try {
      return adapter.detect(root);
    } catch (error) {
      return {
        id: adapter.id,
        name: adapter.name,
        root: adapter.roots.join(", "),
        detected: false,
        confidence: "none",
        counts: {},
        error: error.message
      };
    }
  });
}

function uniqueFeature(base, seen, kind) {
  const clean = slugify(base) || "untitled";

  if (!seen.has(clean)) {
    seen.add(clean);
    return clean;
  }

  const suffixed = `${clean}-${slugify(kind || "item") || "item"}`;
  if (!seen.has(suffixed)) {
    seen.add(suffixed);
    return suffixed;
  }

  for (let index = 2; index < 100; index += 1) {
    const candidate = `${suffixed}-${index}`;
    if (!seen.has(candidate)) {
      seen.add(candidate);
      return candidate;
    }
  }

  return `${clean}-${Date.now()}`;
}

function fileMapping(root, source, target) {
  if (!source) {
    return { kind: path.basename(target).replace(/\.md$/, ""), source: null, target, status: "placeholder" };
  }

  return {
    kind: path.basename(target).replace(/\.md$/, ""),
    source: relative(root, source),
    target,
    status: "mapped"
  };
}

function buildPlan(root, adapterId) {
  const adapter = getAdapter(adapterId);

  if (!adapter) {
    throw new Error(`Unknown migration framework "${adapterId}". Available: ${listAdapters().map((entry) => entry.id).join(", ")}`);
  }

  const detection = adapter.detect(root);
  const entries = adapter.inventory(root);
  const seen = new Set();
  const features = [];

  for (const entry of entries) {
    const feature = uniqueFeature(entry.feature, seen, entry.kind);
    const lifecycle = entry.lifecycle || adapter.defaultLifecycle || "backlog";
    const targetDir = `.sdrive/specs/${lifecycle}/${feature}/${VERSION}`;
    const governanceDir = `.sdrive/governance/${feature}/${VERSION}`;
    const targetRoot = path.join(root, targetDir);
    const conflict = exists(targetRoot);
    const files = [
      fileMapping(root, entry.files.spec, `${targetDir}/spec.md`),
      fileMapping(root, entry.files.plan, `${targetDir}/plan.md`),
      fileMapping(root, entry.files.tasks, `${targetDir}/tasks.md`)
    ];
    const missing = files.filter((file) => file.status === "placeholder").map((file) => file.kind);

    features.push({
      feature,
      source: entry.sourceDir,
      sourceLifecycle: entry.lifecycle || null,
      targetLifecycle: lifecycle,
      kind: entry.kind || "spec",
      status: conflict ? "conflict" : missing.includes("spec") ? "partial" : "ready",
      conflict,
      missing,
      targetDir,
      governanceDir,
      files,
      notes: entry.notes || []
    });
  }

  return {
    framework: adapter.id,
    name: adapter.name,
    root: adapter.roots.join(", "),
    detected: detection.detected,
    confidence: detection.confidence,
    generatedAt: new Date().toISOString(),
    features,
    summary: {
      features: features.length,
      ready: features.filter((feature) => feature.status === "ready").length,
      partial: features.filter((feature) => feature.status === "partial").length,
      conflicts: features.filter((feature) => feature.status === "conflict").length
    }
  };
}

module.exports = {
  VERSION,
  listAdapters,
  getAdapter,
  detectAll,
  buildPlan
};
