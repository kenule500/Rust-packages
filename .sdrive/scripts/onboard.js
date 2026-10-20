#!/usr/bin/env node

const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();
const sdriveRoot = path.join(projectRoot, ".sdrive");
const onboardingDir = path.join(sdriveRoot, "onboarding");

const inventoryPath = path.join(onboardingDir, "inventory.json");
const reportPath = path.join(onboardingDir, "report.md");

const DETECT_FILES = {
  manifests: [
    "package.json",
    "requirements.txt",
    "pyproject.toml",
    "go.mod",
    "pubspec.yaml",
    "Cargo.toml",
    "pom.xml",
    "build.gradle"
  ],
  docs: [
    "README.md",
    "CONTRIBUTING.md"
  ],
  codeRoots: [
    "src",
    "app",
    "lib",
    "pages",
    "server",
    "internal",
    "cmd",
    "pkg",
    "api",
    "apps",
    "packages"
  ],
  ci: [
    ".github/workflows",
    ".gitlab-ci.yml"
  ],
  tests: [
    "tests",
    "__tests__",
    "spec",
    "test"
  ],
  configs: [
    "tsconfig.json",
    "jsconfig.json",
    ".eslintrc",
    ".eslintrc.js",
    ".eslintrc.json",
    ".prettierrc",
    ".prettierrc.js",
    ".prettierrc.json"
  ],
  specdrive: [
    ".sdrive/constitution.md",
    ".sdrive/skills",
    ".sdrive/specs",
    ".sdrive/governance",
    ".sdrive/workflow-state.json",
    ".sdrive/schemas",
    ".sdrive/scripts",
    ".sdrive/commands",
    ".sdrive/agents",
    ".sdrive/dashboard",
    ".sdrive/mcp"
  ],
  externalSpecSystems: [
    "openspec",
    "openspec/specs",
    "openspec/changes",
    "openspec/archive",
    ".specify",
    ".specify/specs",
    ".specify/memory"
  ]
};

function exists(relativePath) {
  return fs.existsSync(path.join(projectRoot, relativePath));
}

function countFiles(relativePath) {
  const full = path.join(projectRoot, relativePath);

  if (!fs.existsSync(full)) return 0;

  const stat = fs.statSync(full);
  if (!stat.isDirectory()) return 1;

  let count = 0;
  const walk = (dir) => {
    for (const entry of fs.readdirSync(dir, { withFileTypes: true })) {
      if (entry.name === "node_modules" || entry.name === ".git") continue;
      const fullEntry = path.join(dir, entry.name);
      if (entry.isDirectory()) {
        walk(fullEntry);
      } else {
        count++;
      }
    }
  };
  walk(full);
  return count;
}

function scanSection(list) {
  const result = [];
  for (const relativePath of list) {
    const present = exists(relativePath);
    result.push({
      path: relativePath,
      present,
      fileCount: present ? countFiles(relativePath) : 0
    });
  }
  return result;
}

function detectFrameworks() {
  try {
    const migrations = require(path.join(__dirname, "..", "migrations"));
    return migrations
      .detectAll(projectRoot)
      .filter((entry) => entry.detected)
      .map((entry) => ({
        id: entry.id,
        name: entry.name,
        root: entry.root,
        confidence: entry.confidence,
        detected: true,
        counts: entry.counts || {}
      }));
  } catch (error) {
    return [];
  }
}

function detectExternalSpecSystems() {
  const detection = {
    openspec: {
      detected: false,
      specs: 0,
      changes: 0,
      archived: 0
    },
    speckit: {
      detected: false,
      specs: 0,
      hasConstitution: false
    },
    frameworks: []
  };

  // OpenSpec detection
  const openspecDir = path.join(projectRoot, "openspec");
  if (fs.existsSync(openspecDir)) {
    detection.openspec.detected = true;

    const specsDir = path.join(openspecDir, "specs");
    const changesDir = path.join(openspecDir, "changes");
    const archiveDir = path.join(openspecDir, "archive");

    if (fs.existsSync(specsDir)) {
      detection.openspec.specs = fs
        .readdirSync(specsDir, { withFileTypes: true })
        .filter((d) => d.isDirectory()).length;
    }

    if (fs.existsSync(changesDir)) {
      detection.openspec.changes = fs
        .readdirSync(changesDir, { withFileTypes: true })
        .filter((d) => d.isDirectory()).length;
    }

    if (fs.existsSync(archiveDir)) {
      detection.openspec.archived = fs
        .readdirSync(archiveDir, { withFileTypes: true })
        .filter((d) => d.isDirectory()).length;
    }
  }

  // Spec-Kit detection
  const speckitDir = path.join(projectRoot, ".specify");
  if (fs.existsSync(speckitDir)) {
    detection.speckit.detected = true;

    const specsDir = path.join(speckitDir, "specs");
    const constitutionPath = path.join(speckitDir, "memory", "constitution.md");

    if (fs.existsSync(specsDir)) {
      detection.speckit.specs = fs
        .readdirSync(specsDir, { withFileTypes: true })
        .filter((d) => d.isDirectory()).length;
    }

    if (fs.existsSync(constitutionPath)) {
      detection.speckit.hasConstitution = true;
    }
  }

  detection.frameworks = detectFrameworks();

  return detection;
}

// A monorepo keeps its real code roots in a workspace manifest, not in the fixed
// codeRoots list above. Reading that manifest is the difference between finding
// `server/` and missing `client/` entirely - and a partial inventory silently
// poisons everything built on it later (constitution, retroactive specs).
function readWorkspacePatterns() {
  const patterns = [];
  const sources = [];

  const add = (list, source) => {
    const usable = list.filter((p) => typeof p === "string" && p.trim() !== "");
    if (usable.length > 0) sources.push(source);
    for (const pattern of usable) {
      if (!patterns.includes(pattern)) patterns.push(pattern);
    }
  };

  // pnpm-workspace.yaml. Only the `packages:` list is parsed, which keeps a YAML
  // dependency out of a script that needs exactly one field from the file.
  const pnpmPath = path.join(projectRoot, "pnpm-workspace.yaml");
  if (fs.existsSync(pnpmPath)) {
    const found = [];
    let inPackages = false;
    for (const raw of fs.readFileSync(pnpmPath, "utf8").split(/\r?\n/)) {
      const line = raw.replace(/#.*$/, "");
      if (/^\s*packages\s*:/.test(line)) {
        inPackages = true;
        continue;
      }
      if (!inPackages) continue;

      const item = line.match(/^\s*-\s*(.+?)\s*$/);
      if (item) {
        found.push(item[1].replace(/^['"]|['"]$/g, ""));
        continue;
      }

      // A non-indented, non-blank line means the packages block has ended.
      if (line.trim() !== "" && !/^\s/.test(line)) inPackages = false;
    }
    add(found, "pnpm-workspace.yaml");
  }

  // package.json#workspaces, which is either an array or { packages: [...] }
  const manifestPath = path.join(projectRoot, "package.json");
  if (fs.existsSync(manifestPath)) {
    try {
      const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
      const workspaces = manifest.workspaces;
      const list = Array.isArray(workspaces)
        ? workspaces
        : workspaces && Array.isArray(workspaces.packages)
          ? workspaces.packages
          : [];
      add(list, "package.json#workspaces");
    } catch {
      // A malformed package.json is the project's problem, not the scan's. The
      // rest of the inventory is still worth producing.
    }
  }

  // lerna.json#packages
  const lernaPath = path.join(projectRoot, "lerna.json");
  if (fs.existsSync(lernaPath)) {
    try {
      const lerna = JSON.parse(fs.readFileSync(lernaPath, "utf8"));
      add(Array.isArray(lerna.packages) ? lerna.packages : [], "lerna.json");
    } catch {
      // Same reasoning as above.
    }
  }

  // turbo.json is a SIGNAL only: Turborepo delegates package discovery to the
  // package manager and keeps no package list of its own to read.
  const signals = [];
  for (const signal of ["turbo.json", "nx.json", "rush.json"]) {
    if (fs.existsSync(path.join(projectRoot, signal))) signals.push(signal);
  }

  return { patterns, sources, signals };
}

function expandWorkspacePattern(pattern) {
  const normalized = pattern
    .trim()
    .replace(/\\/g, "/")
    .replace(/^\.\//, "")
    .replace(/\/+$/, "");
  if (normalized === "") return [];

  // A literal path names a package directly - no globbing needed.
  if (!normalized.includes("*")) {
    const full = path.join(projectRoot, normalized);
    return fs.existsSync(full) && fs.statSync(full).isDirectory() ? [normalized] : [];
  }

  const base = normalized.slice(0, normalized.indexOf("*")).replace(/\/+$/, "");
  const baseDir = path.join(projectRoot, base === "" ? "." : base);
  if (!fs.existsSync(baseDir) || !fs.statSync(baseDir).isDirectory()) return [];

  const deep = normalized.includes("**");
  const results = [];

  const walk = (dir, relDir) => {
    let entries;
    try {
      entries = fs.readdirSync(dir, { withFileTypes: true });
    } catch {
      return;
    }

    for (const entry of entries) {
      if (!entry.isDirectory()) continue;
      if (entry.name === "node_modules" || entry.name === ".git") continue;

      const rel = relDir === "" ? entry.name : `${relDir}/${entry.name}`;
      const full = path.join(dir, entry.name);

      // Inside a workspace, a package is a directory with its own manifest.
      if (fs.existsSync(path.join(full, "package.json")) && !results.includes(rel)) {
        results.push(rel);
      }

      if (deep) walk(full, rel);
    }
  };

  walk(baseDir, base);
  return results;
}

function detectWorkspaces() {
  const { patterns, sources, signals } = readWorkspacePatterns();

  const packages = [];
  for (const pattern of patterns) {
    for (const found of expandWorkspacePattern(pattern)) {
      if (!packages.includes(found)) packages.push(found);
    }
  }
  packages.sort();

  return {
    detected: patterns.length > 0,
    source: sources.length > 0 ? sources.join(", ") : null,
    patterns,
    packages,
    signals
  };
}

// ---------- PROJECT BRIEF ----------
// A new SpecDrive project shows empty dashboard panels until a product plan
// exists, which reads as though SpecDrive knows nothing about the project. But
// something IS known: the README says what the app is for, and when it says
// nothing useful the manifests still say what the app is built with.
//
// The brief is DERIVED, and it says so. It is written to product/overview.md in
// the exact shape the dashboard parses, marked as a draft, and only when that file
// does not already exist. It is deliberately NOT a product plan: only
// /sdrive:product-plan writes one, and that flow reads this file, preserves it, and
// proposes a merge rather than treating it as finished.
const README_CANDIDATES = [
  "README.md",
  "readme.md",
  "Readme.md",
  "README.MD",
  "README.markdown",
  "README.rst"
];

// Recognised dependencies, mapped to the name a person would use. Anything not on
// this list is left out rather than guessed at.
const TECH_LABELS = {
  next: "Next.js",
  react: "React",
  vue: "Vue",
  nuxt: "Nuxt",
  svelte: "Svelte",
  express: "Express",
  fastify: "Fastify",
  "@nestjs/core": "NestJS",
  hono: "Hono",
  koa: "Koa",
  typescript: "TypeScript",
  vite: "Vite",
  webpack: "Webpack",
  esbuild: "esbuild",
  turbo: "Turborepo",
  tailwindcss: "Tailwind CSS",
  prisma: "Prisma",
  mongoose: "Mongoose",
  "drizzle-orm": "Drizzle",
  sequelize: "Sequelize",
  mongodb: "MongoDB",
  pg: "PostgreSQL",
  mysql2: "MySQL",
  ioredis: "Redis",
  redis: "Redis",
  vitest: "Vitest",
  jest: "Jest",
  mocha: "Mocha",
  playwright: "Playwright",
  cypress: "Cypress",
  zod: "Zod",
  graphql: "GraphQL",
  "socket.io": "Socket.IO",
  stripe: "Stripe",
  convex: "Convex",
  "@clerk/nextjs": "Clerk",
  "@supabase/supabase-js": "Supabase",
  firebase: "Firebase",
  "next-auth": "NextAuth",
  "@trpc/server": "tRPC",
  resend: "Resend",
  zustand: "Zustand"
};

// A manifest sitting in the repo is the surest statement of language it can make.
const LANGUAGE_MARKERS = [
  ["tsconfig.json", "TypeScript"],
  ["go.mod", "Go"],
  ["Cargo.toml", "Rust"],
  ["pyproject.toml", "Python"],
  ["requirements.txt", "Python"],
  ["Gemfile", "Ruby"],
  ["pom.xml", "Java"],
  ["build.gradle", "Java"],
  ["composer.json", "PHP"]
];

function readReadme() {
  for (const name of README_CANDIDATES) {
    const full = path.join(projectRoot, name);
    if (!fs.existsSync(full)) continue;
    try {
      return { name, text: fs.readFileSync(full, "utf8") };
    } catch (error) {
      return null;
    }
  }
  return null;
}

// A scaffolded project ships with the scaffolder's README, and that README is
// confidently wrong about the product: "This is a Next.js project bootstrapped
// with create-next-app" is true of thousands of unrelated repositories. Taking
// its first paragraph as the description produced a brief that described the
// framework, not the app.
//
// These are the exact sentences the generators emit. Each one is conclusive on
// its own, because no project writes "bootstrapped with create-next-app" about
// itself after it becomes an actual product.
const TEMPLATE_README_SIGNATURES = [
  { pattern: /bootstrapped with \[?`?create-next-app/i, label: "create-next-app" },
  { pattern: /this is a \[?next\.?js\]?\s+project/i, label: "next.js starter readme" },
  { pattern: /create-vite|create-remix|create-t3-app|create-svelte|create-nuxt|create-expo-app/i, label: "framework scaffold" },
  { pattern: /deploy on vercel/i, label: "vercel deploy section" }
];

// The last two signatures are weak on their own - plenty of real READMEs link
// Next.js docs or mention Vercel - so they only count when the README also looks
// like a generator's output: a Getting Started section whose only step is
// `npm run dev`, and no product language anywhere.
const TEMPLATE_README_SUPPORTING = [
  { pattern: /##\s+getting started/i, label: "getting-started section" },
  { pattern: /npm run dev|yarn dev|pnpm dev|bun dev/i, label: "run-the-dev-server step" },
  { pattern: /##\s+learn more/i, label: "learn-more section" },
  { pattern: /you can start editing the page/i, label: "edit-this-page instruction" }
];

/**
 * Decides whether the README can be trusted to describe the product.
 *
 * @returns {{quality: "usable"|"template"|"missing", signals: string[]}}
 */
function assessReadme(readme) {
  if (!readme) return { quality: "missing", signals: [] };

  const text = readme.text;
  const matched = TEMPLATE_README_SIGNATURES.filter((entry) => entry.pattern.test(text));

  // The first two signatures are conclusive. "Deploy on Vercel" is not, so it
  // only counts alongside generator-shaped scaffolding.
  const conclusive = matched.filter((entry) => entry.label !== "vercel deploy section");

  if (conclusive.length > 0) {
    const supporting = TEMPLATE_README_SUPPORTING.filter((entry) => entry.pattern.test(text));
    return {
      quality: "template",
      signals: conclusive.concat(supporting).map((entry) => entry.label)
    };
  }

  const supporting = TEMPLATE_README_SUPPORTING.filter((entry) => entry.pattern.test(text));

  if (matched.length > 0 && supporting.length >= 2) {
    return { quality: "template", signals: matched.concat(supporting).map((entry) => entry.label) };
  }

  return { quality: "usable", signals: [] };
}

// A README line worth quoting as a description. Headings, badges, images, HTML,
// list items and rules say nothing about what the app is, and a very short line is
// usually a label rather than a sentence.
function isDescriptiveProse(line) {
  const value = line.trim();
  if (value.length < 30) return false;
  if (/^#{1,6}\s/.test(value)) return false;
  if (/^([-*+]|\d+\.)\s/.test(value)) return false;
  if (/^[>|]/.test(value)) return false;
  if (/^</.test(value)) return false;
  if (/!\[/.test(value)) return false;
  if (/^=+$/.test(value) || /^-+$/.test(value)) return false;
  return true;
}

function bulletValue(line) {
  const match = line.match(/^\s*[-*+]\s+(.*)$/);
  if (!match) return "";
  return match[1].replace(/\*\*/g, "").replace(/`/g, "").trim();
}

// Bullets under the first heading whose text contains one of `needles`.
function sectionBullets(text, needles) {
  const found = [];
  let capturing = false;

  for (const line of text.split(/\r?\n/)) {
    const heading = line.match(/^#{1,6}\s+(.*)$/);
    if (heading) {
      const key = heading[1].toLowerCase();
      capturing = needles.some((needle) => key.includes(needle));
      continue;
    }
    if (!capturing) continue;
    const value = bulletValue(line);
    if (value && value.length <= 200) found.push(value);
  }

  return found.slice(0, 6);
}

// The opening paragraph, reassembled from its wrapped lines. Taking only the first
// matching line would cut a sentence in half at whatever column the README happens
// to wrap at.
function firstParagraph(lines) {
  const start = lines.findIndex(isDescriptiveProse);
  if (start < 0) return "";

  const paragraph = [];
  for (const line of lines.slice(start)) {
    if (!line.trim()) break;
    if (/^#{1,6}\s/.test(line)) break;
    if (/^([-*+]|\d+\.)\s/.test(line)) break;
    paragraph.push(line.trim());
  }
  return paragraph.join(" ");
}

function extractReadmeBrief(readme) {
  const lines = readme.text.split(/\r?\n/);
  const titleIndex = lines.findIndex((line) => /^#\s+\S/.test(line));
  const title = titleIndex >= 0 ? lines[titleIndex].replace(/^#\s+/, "").trim() : "";

  const description = firstParagraph(lines.slice(titleIndex + 1));

  const goals = sectionBullets(readme.text, [
    "what it does",
    "features",
    "goals",
    "overview",
    "capabilities"
  ]);
  const targetUsers = sectionBullets(readme.text, [
    "users",
    "audience",
    "personas",
    "customers"
  ]);

  // A README of just a title and badges says nothing; let the code scan answer.
  if (!description && goals.length === 0 && targetUsers.length === 0) return null;

  return {
    derivedFrom: "readme",
    readmePath: readme.name,
    title,
    description: description.trim(),
    goals,
    targetUsers,
    architecture: []
  };
}

function detectTechStack(inventory) {
  const found = [];

  for (const [file, language] of LANGUAGE_MARKERS) {
    if (fs.existsSync(path.join(projectRoot, file)) && !found.includes(language)) {
      found.push(language);
    }
  }

  const manifestPath = path.join(projectRoot, "package.json");
  if (fs.existsSync(manifestPath)) {
    try {
      const manifest = JSON.parse(fs.readFileSync(manifestPath, "utf8"));
      const deps = { ...(manifest.dependencies || {}), ...(manifest.devDependencies || {}) };
      for (const name of Object.keys(deps)) {
        const label = TECH_LABELS[name];
        if (label && !found.includes(label)) found.push(label);
      }
      if (manifest.packageManager) {
        found.push(manifest.packageManager.split("@")[0]);
      }
    } catch (error) {
      // A malformed manifest is reported by the scan elsewhere; the brief carries
      // on with whatever the other markers told it.
    }
  }

  if (inventory.workspaces?.detected) {
    found.push(`monorepo (${inventory.workspaces.packages.length} packages)`);
  }

  return found.slice(0, 10);
}

// README first, because it is the only file that was written to explain the
// product - but only when it actually describes the product. A scaffolded
// README is treated as absent: it is confidently about the framework, and
// quoting it would fill the overview with something true of every repo the
// generator ever produced.
function buildProjectBrief(inventory) {
  const readme = readReadme();
  const assessment = assessReadme(readme);
  const fromReadme = readme && assessment.quality === "usable" ? extractReadmeBrief(readme) : null;

  if (fromReadme) {
    fromReadme.architecture = detectTechStack(inventory);
    fromReadme.readmeQuality = assessment.quality;
    fromReadme.templateSignals = [];
    fromReadme.needsProductInput = false;
    return fromReadme;
  }

  const architecture = detectTechStack(inventory);
  let title = "";
  const manifestPath = path.join(projectRoot, "package.json");
  if (fs.existsSync(manifestPath)) {
    try {
      title = JSON.parse(fs.readFileSync(manifestPath, "utf8")).name || "";
    } catch (error) {
      title = "";
    }
  }

  // Nothing to say at all: report that honestly rather than writing an empty brief.
  if (!title && architecture.length === 0) {
    return {
      derivedFrom: "none",
      readmePath: readme ? readme.name : null,
      readmeQuality: assessment.quality,
      templateSignals: assessment.signals,
      needsProductInput: true,
      title: "",
      description: "",
      goals: [],
      targetUsers: [],
      architecture: []
    };
  }

  return {
    derivedFrom: "code-scan",
    readmePath: readme ? readme.name : null,
    readmeQuality: assessment.quality,
    templateSignals: assessment.signals,
    // A code scan can name the stack; it cannot say what the product is for.
    // Whatever it produces, the overview needs the user's own words.
    needsProductInput: !title || assessment.quality === "template",
    title,
    description: "",
    goals: [],
    targetUsers: [],
    architecture
  };
}

function renderBriefMarkdown(brief) {
  const origin = brief.derivedFrom === "readme" ? brief.readmePath : "the code scan";
  const isTemplate = brief.readmeQuality === "template";
  const lines = [];

  lines.push(`# ${brief.title || path.basename(projectRoot)}`);
  lines.push("");
  lines.push(
    isTemplate
      ? `Status: Draft (derived from ${origin}; README is template boilerplate)`
      : `Status: Draft (derived from ${origin})`
  );
  lines.push("");

  if (isTemplate) {
    lines.push("> **The README is scaffold boilerplate, not a description of this product.**");
    lines.push(
      `> ${brief.readmePath} looks like the file ${
        (brief.templateSignals || []).includes("create-next-app") ? "create-next-app" : "a project generator"
      } writes (${(brief.templateSignals || []).slice(0, 3).join(", ") || "template signature"}).`
    );
    lines.push("> Taking its opening paragraph as the vision would describe the framework, not the app -");
    lines.push('> "This is a Next.js project bootstrapped with create-next-app" is true of thousands of');
    lines.push("> unrelated repositories. So this brief deliberately has no Vision section.");
    lines.push(">");
    lines.push("> Run `/sdrive:onboard` again, or `/sdrive:product-plan`, and answer the questions");
    lines.push("> below to replace this file with a real product overview.");
    lines.push("");
  }

  lines.push("> **This is a draft, not a product plan.**");
  lines.push("> The onboarding scan wrote it from what was already in the repository. Nobody");
  lines.push("> reviewed it and nothing here was confirmed. Run `/sdrive:product-plan` to");
  lines.push("> replace it with a plan you have actually approved - that flow reads this file");
  lines.push("> and proposes a merge rather than discarding it.");
  lines.push("");

  if (brief.description) {
    lines.push("## Vision");
    lines.push("");
    lines.push(brief.description);
    lines.push("");
  }

  if (brief.goals.length > 0) {
    lines.push("## Goals");
    lines.push("");
    brief.goals.forEach((goal) => lines.push(`- ${goal}`));
    lines.push("");
  }

  if (brief.targetUsers.length > 0) {
    lines.push("## Target Users");
    lines.push("");
    lines.push("| Segment | Description | Need |");
    lines.push("| --- | --- | --- |");
    brief.targetUsers.forEach((user) => lines.push(`| ${user} | from ${brief.readmePath} |  |`));
    lines.push("");
  }

  if (brief.architecture.length > 0) {
    lines.push("## Existing Architecture");
    lines.push("");
    brief.architecture.forEach((item) => lines.push(`- ${item}`));
    lines.push("");
  }

  lines.push("## Open Questions");
  lines.push("");
  lines.push(
    isTemplate
      ? "- What is this product, in one sentence? The scaffolded README never says."
      : "- Was the vision above accurate, or is it only the opening paragraph of the README?"
  );
  lines.push(
    brief.targetUsers.length > 0
      ? "- Are those the right segments, and what does each one actually need?"
      : "- Who are the target users? Nothing in the repository names them."
  );
  lines.push(
    brief.goals.length > 0
      ? "- Which of the goals above matter most, and what is deliberately out of scope?"
      : "- What is this product for, and what is deliberately out of scope?"
  );
  lines.push("");

  return lines.join("\n");
}

// Writes product/overview.md ONLY when nothing is there. An existing product plan
// is the user's own work and is never touched.
function writeProductBrief(brief) {
  if (!brief || brief.derivedFrom === "none") {
    return { written: false, reason: "nothing to report" };
  }

  const dir = path.join(sdriveRoot, "product");
  const target = path.join(dir, "overview.md");

  if (fs.existsSync(target)) {
    return { written: false, reason: "product/overview.md already exists" };
  }

  fs.mkdirSync(dir, { recursive: true });
  fs.writeFileSync(target, renderBriefMarkdown(brief));
  return { written: true, path: "product/overview.md" };
}

function buildInventory() {
  // Detected first, because the fixed codeRoots list cannot know what a monorepo
  // calls its packages.
  const workspaces = detectWorkspaces();

  const codeRootPaths = [...DETECT_FILES.codeRoots];
  for (const pkg of workspaces.packages) {
    if (!codeRootPaths.includes(pkg)) codeRootPaths.push(pkg);
  }

  const inventory = {
    projectRoot,
    scannedAt: new Date().toISOString(),
    mode: "brownfield",
    workspaces,
    sections: {
      manifests: scanSection(DETECT_FILES.manifests),
      docs: scanSection(DETECT_FILES.docs),
      codeRoots: scanSection(codeRootPaths),
      ci: scanSection(DETECT_FILES.ci),
      tests: scanSection(DETECT_FILES.tests),
      configs: scanSection(DETECT_FILES.configs),
      specdrive: scanSection(DETECT_FILES.specdrive),
      externalSpecSystems: scanSection(DETECT_FILES.externalSpecSystems)
    },
    detectedSystems: detectExternalSpecSystems(),
    missing: [],
    present: [],
    deferred: []
  };

  const flatten = (section) => section.map((item) => item);

  const all = [
    ...flatten(inventory.sections.manifests),
    ...flatten(inventory.sections.docs),
    ...flatten(inventory.sections.codeRoots),
    ...flatten(inventory.sections.ci),
    ...flatten(inventory.sections.tests),
    ...flatten(inventory.sections.configs),
    ...flatten(inventory.sections.specdrive)
  ];

  for (const item of all) {
    if (item.present) {
      inventory.present.push(item.path);
    } else {
      inventory.missing.push(item.path);
    }
  }

  // Derived last, because it reports on what the sections above actually found.
  inventory.projectBrief = buildProjectBrief(inventory);

  return inventory;
}

function buildReport(inventory) {
  const lines = [];

  lines.push("# Onboarding Report");
  lines.push("");
  lines.push(`Scanned at: ${inventory.scannedAt}`);
  lines.push(`Project root: ${inventory.projectRoot}`);
  lines.push("");

  // External Spec Systems
  const ext = inventory.detectedSystems || {};
  const extraFrameworks = (ext.frameworks || []).filter(
    (framework) => !["openspec", "speckit"].includes(framework.id)
  );

  if (ext.openspec?.detected || ext.speckit?.detected || extraFrameworks.length > 0) {
    lines.push("## Detected External Spec Systems");
    if (ext.openspec?.detected) {
      lines.push(`- ✅ OpenSpec`);
      lines.push(`  - Specs: ${ext.openspec.specs}`);
      lines.push(`  - Active changes: ${ext.openspec.changes}`);
      lines.push(`  - Archived: ${ext.openspec.archived}`);
    }
    if (ext.speckit?.detected) {
      lines.push(`- ✅ Spec-Kit`);
      lines.push(`  - Specs: ${ext.speckit.specs}`);
      lines.push(`  - Constitution: ${ext.speckit.hasConstitution ? "found" : "missing"}`);
    }
    for (const framework of extraFrameworks) {
      lines.push(`- ✅ ${framework.name} (${framework.id})`);
      lines.push(`  - Root: ${framework.root}`);
      lines.push(`  - Confidence: ${framework.confidence}`);
      const counts = Object.entries(framework.counts || {})
        .map(([key, value]) => `${key}=${value}`)
        .join(", ");
      if (counts) lines.push(`  - Counts: ${counts}`);
    }
    lines.push("");
    lines.push("> These can be migrated into SpecDrive during onboarding via the Onboarding Agent.");
    lines.push("> Plan first: `sdrive migrate:plan <framework>`; apply with `sdrive migrate:apply <framework> --yes`.");
    lines.push("");
  }

  // Monorepo workspaces. In a monorepo these ARE the code roots; the fixed list
  // in DETECT_FILES can only ever guess at them.
  const ws = inventory.workspaces || {};
  if (ws.detected || (ws.signals && ws.signals.length > 0)) {
    lines.push("## Monorepo Workspaces");

    if (ws.detected) {
      lines.push(`- Source: ${ws.source}`);
      lines.push(`- Patterns: ${ws.patterns.join(", ")}`);
      lines.push(`- Packages (${ws.packages.length}):`);
      if (ws.packages.length === 0) {
        lines.push("  - (patterns matched no package manifest)");
      } else {
        ws.packages.forEach((pkg) => lines.push(`  - ${pkg}`));
      }
    } else {
      lines.push("- No workspace package list found.");
    }

    if (ws.signals && ws.signals.length > 0) {
      lines.push(`- Monorepo signal: ${ws.signals.join(", ")} (carries no package list of its own)`);
    }

    lines.push("");
  }

  lines.push("## Project Brief");

  const brief = inventory.projectBrief || {};
  if (brief.derivedFrom === "readme") {
    lines.push(`- Source: ${brief.readmePath}`);
  } else if (brief.derivedFrom === "code-scan") {
    lines.push(`- Source: code scan (${brief.readmePath || "no README"})`);
  } else {
    lines.push("- Nothing to report.");
  }
  if (brief.title) lines.push(`- Title: ${brief.title}`);
  if (brief.description) lines.push(`- Description: ${brief.description}`);
  if (brief.goals && brief.goals.length > 0) {
    lines.push(`- Goals (${brief.goals.length}):`);
    brief.goals.forEach((goal) => lines.push(`  - ${goal}`));
  }
  if (brief.targetUsers && brief.targetUsers.length > 0) {
    lines.push(`- Target users (${brief.targetUsers.length}):`);
    brief.targetUsers.forEach((user) => lines.push(`  - ${user}`));
  }
  if (brief.architecture && brief.architecture.length > 0) {
    lines.push(`- Tech stack: ${brief.architecture.join(", ")}`);
  }

  if (brief.readmeQuality === "template") {
    lines.push(
      `- ⚠️  README looks like generator boilerplate (${(brief.templateSignals || []).join(", ") || "template signature"})`
    );
    lines.push("- ⚠️  It does not describe this product, so the brief was built from the code scan");
    lines.push("- ➡️  Ask the user whether to write a real product overview (see the Onboarding Agent)");
  } else if (brief.readmeQuality === "missing") {
    lines.push("- ⚠️  No README, so the brief was built from the code scan");
    lines.push("- ➡️  Ask the user whether to write a real product overview");
  }

  lines.push("");
  lines.push("> This brief is DERIVED from the repository, not confirmed by anyone. It is");
  lines.push("> written to `.sdrive/product/overview.md` as a draft, and only when that file");
  lines.push("> does not already exist. Run `/sdrive:product-plan` to replace it with an");
  lines.push("> approved plan. If the README says nothing useful, the brief falls back to the");
  lines.push("> code scan and will be thinner - that gap is real, not an error.");
  lines.push("");

  lines.push("## Present");
  inventory.present.forEach((p) => lines.push(`- ✅ ${p}`));
  lines.push("");
  lines.push("## Missing");
  inventory.missing.forEach((p) => lines.push(`- ❌ ${p}`));
  lines.push("");
  lines.push("## Deferred");
  if (inventory.deferred.length === 0) {
    lines.push("(none yet)");
  } else {
    inventory.deferred.forEach((p) => lines.push(`- ⚠️  ${p}`));
  }
  lines.push("");

  return lines.join("\n");
}

function main() {
  fs.mkdirSync(onboardingDir, { recursive: true });

  const inventory = buildInventory();
  fs.writeFileSync(inventoryPath, JSON.stringify(inventory, null, 2));

  const report = buildReport(inventory);
  fs.writeFileSync(reportPath, report);

  // The dashboard reads product/overview.md, so without this its panels stay blank
  // on a freshly onboarded project. Never overwrites an existing product plan.
  const briefResult = writeProductBrief(inventory.projectBrief);

  const workflowStatePath = path.join(sdriveRoot, "workflow-state.json");
  const workflowState = fs.existsSync(workflowStatePath)
    ? JSON.parse(fs.readFileSync(workflowStatePath, "utf8"))
    : { features: [] };
  workflowState.onboarding = {
    mode: inventory.mode,
    date: inventory.scannedAt,
    detectedSystems: inventory.detectedSystems,
    missing: inventory.missing,
    deferred: inventory.deferred,
    // Surfaced so the Onboarding Agent can see, without re-reading the README,
    // that the product overview still needs the user's own words.
    projectBrief: {
      quality: inventory.projectBrief?.readmeQuality || "missing",
      derivedFrom: inventory.projectBrief?.derivedFrom || "none",
      needsProductInput: inventory.projectBrief?.needsProductInput === true,
      signals: inventory.projectBrief?.templateSignals || []
    }
  };
  fs.writeFileSync(workflowStatePath, JSON.stringify(workflowState, null, 2));

  console.log(`✅ Inventory written: ${path.relative(projectRoot, inventoryPath)}`);
  console.log(`✅ Report written:    ${path.relative(projectRoot, reportPath)}`);
  console.log("");
  console.log(`Present:  ${inventory.present.length}`);
  console.log(`Missing:  ${inventory.missing.length}`);
  console.log(`Deferred: ${inventory.deferred.length}`);

  const brief = inventory.projectBrief || {};

  if (brief.readmeQuality === "template") {
    console.log("");
    console.log(`⚠️  ${brief.readmePath} looks like scaffold boilerplate (${(brief.templateSignals || []).slice(0, 3).join(", ")}).`);
    console.log("   It does not describe this product, so the brief uses the code scan instead.");
    console.log("   The Onboarding Agent will ask whether to write a real product overview.");
  }

  if (briefResult.written) {
    console.log("");
    console.log(`📝 Draft product brief written: ${briefResult.path}`);
    console.log(`   Derived from ${brief.derivedFrom === "readme" ? brief.readmePath : "the code scan"}.`);
    console.log("   It is a draft, not an approved product plan. Run /sdrive:product-plan to replace it.");
  } else if (brief.derivedFrom && brief.derivedFrom !== "none") {
    console.log("");
    console.log(`📝 Draft product brief NOT written: ${briefResult.reason}`);
  }

  if (inventory.workspaces?.detected) {
    console.log("");
    console.log(
      `📦 Monorepo: ${inventory.workspaces.packages.length} workspace package(s) from ${inventory.workspaces.source}`
    );
    inventory.workspaces.packages.forEach((pkg) => console.log(`   - ${pkg}`));
  }

  const ext = inventory.detectedSystems || {};
  const extraFrameworks = (ext.frameworks || []).filter(
    (framework) => !["openspec", "speckit"].includes(framework.id)
  );

  if (ext.openspec?.detected || ext.speckit?.detected || extraFrameworks.length > 0) {
    console.log("");
    console.log("🔍 External Spec Systems Detected:");
    if (ext.openspec?.detected) {
      console.log(`   - OpenSpec (${ext.openspec.specs} specs, ${ext.openspec.changes} changes)`);
    }
    if (ext.speckit?.detected) {
      console.log(`   - Spec-Kit (${ext.speckit.specs} specs)`);
    }
    for (const framework of extraFrameworks) {
      console.log(`   - ${framework.name} (${framework.id})`);
    }
    console.log("");
    console.log("   Run /sdrive:onboard to migrate them, or plan first:");
    console.log("   sdrive migrate:plan <framework>");
  }
}

main();