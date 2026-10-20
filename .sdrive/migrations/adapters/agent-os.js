const path = require("path");
const {
  exists,
  listDirs,
  findFirst,
  slugify,
  relative
} = require("../lib/files");

const ID = "agent-os";
const NAME = "Agent OS";

function detect(root) {
  const base = path.join(root, ".agent-os");
  const detected = exists(base);

  return {
    id: ID,
    name: NAME,
    root: ".agent-os",
    detected,
    confidence: detected ? "high" : "none",
    counts: {
      specs: listDirs(path.join(base, "specs")).length,
      productDocs: listDirs(path.join(base, "product")).length
    }
  };
}

function inventory(root) {
  const specsDir = path.join(root, ".agent-os", "specs");
  if (!exists(specsDir)) return [];

  return listDirs(specsDir).map((name) => {
    const sourceDir = path.join(specsDir, name);

    return {
      feature: slugify(name.replace(/^\d{4}-\d{2}-\d{2}[-_]?/, "")),
      sourceDir: relative(root, sourceDir),
      lifecycle: "ongoing",
      kind: "spec",
      files: {
        spec: findFirst(sourceDir, ["spec.md", "specification.md"]),
        plan: findFirst(sourceDir, ["plan.md", "technical-plan.md"]),
        tasks: findFirst(sourceDir, ["tasks.md", "todo.md"])
      }
    };
  });
}

module.exports = {
  id: ID,
  name: NAME,
  defaultLifecycle: "ongoing",
  roots: [".agent-os"],
  detect,
  inventory
};
