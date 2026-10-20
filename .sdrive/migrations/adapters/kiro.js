const path = require("path");
const {
  exists,
  listDirs,
  findFirst,
  slugify,
  relative
} = require("../lib/files");

const ID = "kiro";
const NAME = "AWS Kiro";

function detect(root) {
  const base = path.join(root, ".kiro");
  const detected = exists(base);

  return {
    id: ID,
    name: NAME,
    root: ".kiro",
    detected,
    confidence: detected ? "high" : "none",
    counts: {
      specs: listDirs(path.join(base, "specs")).length,
      steering: listDirs(path.join(base, "steering")).length
    }
  };
}

function inventory(root) {
  const specsDir = path.join(root, ".kiro", "specs");
  if (!exists(specsDir)) return [];

  return listDirs(specsDir).map((name) => {
    const sourceDir = path.join(specsDir, name);

    return {
      feature: slugify(name),
      sourceDir: relative(root, sourceDir),
      lifecycle: "ongoing",
      kind: "spec",
      files: {
        // Kiro's requirements.md is the closest analogue to a SpecDrive spec;
        // design.md is the plan, and tasks.md is already task-shaped.
        spec: findFirst(sourceDir, ["requirements.md", "spec.md"]),
        plan: findFirst(sourceDir, ["design.md", "plan.md"]),
        tasks: findFirst(sourceDir, ["tasks.md", "todo.md"])
      }
    };
  });
}

module.exports = {
  id: ID,
  name: NAME,
  defaultLifecycle: "ongoing",
  roots: [".kiro"],
  detect,
  inventory
};
