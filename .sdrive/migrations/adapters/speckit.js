const path = require("path");
const {
  exists,
  listDirs,
  findFirst,
  slugify,
  relative
} = require("../lib/files");

const ID = "speckit";
const NAME = "GitHub Spec Kit";

function detect(root) {
  const base = path.join(root, ".specify");
  const detected = exists(base);
  const specsDir = path.join(base, "specs");

  return {
    id: ID,
    name: NAME,
    root: ".specify",
    detected,
    confidence: detected ? "high" : "none",
    counts: {
      specs: listDirs(specsDir).length,
      hasConstitution: exists(path.join(base, "memory", "constitution.md"))
    }
  };
}

function inventory(root) {
  const specsDir = path.join(root, ".specify", "specs");
  if (!exists(specsDir)) return [];

  return listDirs(specsDir).map((name) => {
    const sourceDir = path.join(specsDir, name);
    const feature = slugify(name.replace(/^[0-9]+[-_]/, ""));

    return {
      feature,
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
  roots: [".specify"],
  detect,
  inventory
};
