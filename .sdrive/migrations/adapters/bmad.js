const path = require("path");
const {
  exists,
  listFiles,
  findFirst,
  slugify,
  relative
} = require("../lib/files");

const ID = "bmad";
const NAME = "BMAD Method";

function detect(root) {
  const base = path.join(root, ".bmad-core");
  const prd = path.join(root, "docs", "prd.md");
  const storiesDir = path.join(root, "docs", "stories");
  const detected = exists(base) || exists(prd) || exists(storiesDir);

  return {
    id: ID,
    name: NAME,
    root: ".bmad-core / docs",
    detected,
    confidence: detected ? (exists(base) ? "medium" : "low") : "none",
    counts: {
      stories: listFiles(storiesDir).filter((file) => file.endsWith(".md")).length,
      hasPrd: exists(prd),
      hasArchitecture: exists(path.join(root, "docs", "architecture.md"))
    }
  };
}

function inventory(root) {
  const storiesDir = path.join(root, "docs", "stories");
  const prd = path.join(root, "docs", "prd.md");
  const architecture = path.join(root, "docs", "architecture.md");
  const entries = [];

  // BMAD stories are the closest thing to a feature spec. They are imported as
  // backlog drafts because BMAD's prose does not carry SpecDrive AC IDs.
  for (const file of listFiles(storiesDir)) {
    if (!file.endsWith(".md")) continue;

    const source = path.join(storiesDir, file);
    entries.push({
      feature: slugify(file.replace(/\.md$/, "")),
      sourceDir: relative(root, path.dirname(source)),
      lifecycle: "backlog",
      kind: "story",
      files: {
        spec: source,
        plan: exists(architecture) ? architecture : null,
        tasks: null
      },
      notes: ["BMAD story imported as a backlog draft; acceptance criteria must be confirmed."]
    });
  }

  if (entries.length === 0 && exists(prd)) {
    entries.push({
      feature: "bmad-product",
      sourceDir: "docs",
      lifecycle: "backlog",
      kind: "prd",
      files: {
        spec: prd,
        plan: findFirst(path.join(root, "docs"), ["architecture.md", "tech-stack.md"]),
        tasks: null
      },
      notes: ["BMAD PRD imported as one backlog draft; split it into features with /sdrive:propose."]
    });
  }

  return entries;
}

module.exports = {
  id: ID,
  name: NAME,
  defaultLifecycle: "backlog",
  roots: [".bmad-core", "docs/prd.md", "docs/stories"],
  detect,
  inventory
};
