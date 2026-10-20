const path = require("path");
const {
  exists,
  listDirs,
  findFirst,
  slugify,
  relative
} = require("../lib/files");

const ID = "openspec";
const NAME = "OpenSpec";

function detect(root) {
  const base = path.join(root, "openspec");
  const detected = exists(base);

  return {
    id: ID,
    name: NAME,
    root: "openspec",
    detected,
    confidence: detected ? "high" : "none",
    counts: {
      specs: listDirs(path.join(base, "specs")).length,
      changes: listDirs(path.join(base, "changes")).length,
      archived: listDirs(path.join(base, "archive")).length
    }
  };
}

function entry(root, sourceDir, lifecycle, kind) {
  return {
    feature: slugify(path.basename(sourceDir)),
    sourceDir: relative(root, sourceDir),
    lifecycle,
    kind,
    files: {
      spec: findFirst(sourceDir, ["spec.md", "proposal.md", "README.md"]),
      plan: findFirst(sourceDir, ["plan.md", "design.md", "technical-plan.md"]),
      tasks: findFirst(sourceDir, ["tasks.md", "todo.md"])
    }
  };
}

function inventory(root) {
  const base = path.join(root, "openspec");
  const entries = [];

  for (const name of listDirs(path.join(base, "specs"))) {
    entries.push(entry(root, path.join(base, "specs", name), "ongoing", "spec"));
  }

  for (const name of listDirs(path.join(base, "changes"))) {
    entries.push(entry(root, path.join(base, "changes", name), "ongoing", "change"));
  }

  for (const name of listDirs(path.join(base, "archive"))) {
    entries.push(entry(root, path.join(base, "archive", name), "completed", "archive"));
  }

  return entries;
}

module.exports = {
  id: ID,
  name: NAME,
  defaultLifecycle: "ongoing",
  roots: ["openspec"],
  detect,
  inventory
};
