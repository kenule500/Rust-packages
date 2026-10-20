const path = require("path");
const {
  exists,
  listDirs,
  findFirst,
  slugify,
  relative
} = require("../lib/files");

const ID = "generic";
const NAME = "Generic spec folders";

const ROOTS = ["specs", "docs/specs", "documentation/specs"];

function detect(root) {
  const roots = ROOTS.filter((dir) => exists(path.join(root, dir)));

  return {
    id: ID,
    name: NAME,
    root: roots.join(", "),
    detected: roots.length > 0,
    confidence: roots.length > 0 ? "low" : "none",
    counts: {
      roots: roots.length,
      features: roots.reduce((total, dir) => total + listDirs(path.join(root, dir)).length, 0)
    }
  };
}

function inventory(root) {
  const entries = [];

  for (const dir of ROOTS) {
    const base = path.join(root, dir);
    if (!exists(base)) continue;

    for (const name of listDirs(base)) {
      const sourceDir = path.join(base, name);
      const spec = findFirst(sourceDir, ["spec.md", "requirements.md", "README.md"]);
      const plan = findFirst(sourceDir, ["plan.md", "design.md", "technical-plan.md"]);
      const tasks = findFirst(sourceDir, ["tasks.md", "todo.md"]);

      if (!spec && !plan && !tasks) continue;

      entries.push({
        feature: slugify(name),
        sourceDir: relative(root, sourceDir),
        lifecycle: "backlog",
        kind: "spec-folder",
        files: { spec, plan, tasks },
        notes: ["Generic spec folder imported as a backlog draft; confirm the mapping."]
      });
    }
  }

  return entries;
}

module.exports = {
  id: ID,
  name: NAME,
  defaultLifecycle: "backlog",
  roots: ROOTS,
  detect,
  inventory
};
