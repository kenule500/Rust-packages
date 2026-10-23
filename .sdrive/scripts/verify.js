#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { resolveGovernanceFile, resolveGovernanceDir } = require("./lib/governance-paths");

const projectRoot = process.cwd();

// ---------- Phase 1: Plan Reader ----------

function loadPlan(featureName, version) {
  const planPath = resolveGovernanceFile(projectRoot, featureName, version, "plan.json");

  if (!planPath) {
    console.error(`❌ plan.json not found for feature "${featureName}".`);
    process.exit(1);
  }

  return JSON.parse(fs.readFileSync(planPath, "utf8"));
}

function extractPlanItems(plan) {
  const items = [];

  (plan.architecture?.components || []).forEach((c) => {
    items.push({ type: "component", name: c.name, action: c.action });
  });

  (plan.apiContracts || []).forEach((api) => {
    items.push({ type: "endpoint", method: api.method, path: api.path, action: api.action });
  });

  (plan.dataModel?.entities || []).forEach((entity) => {
    items.push({ type: "entity", name: entity.name, action: entity.action });
    (entity.fields || []).forEach((field) => {
      items.push({
        type: "field",
        entity: entity.name,
        name: field.name,
        fieldType: field.type,
        action: field.action
      });
    });
  });

  (plan.dependencies || []).forEach((dep) => {
    items.push({ type: "dependency", name: dep.name, action: dep.action });
  });

  return items;
}

// ---------- Phase 2: Code Scanner ----------

const CANDIDATE_ROOTS = [
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
  "packages",
  "components",
  "messages",
  "locales",
  "i18n",
  "content"
];

const IGNORED_DIRS = new Set([
  "node_modules",
  ".git",
  "dist",
  "build",
  ".next",
  ".expo",
  ".turbo",
  "coverage",
  "out",
  ".sdrive",
  ".cache",
  "vendor"
]);

const CODE_EXTENSIONS = [
  ".ts", ".tsx", ".js", ".jsx", ".mjs", ".cjs",
  ".py", ".go", ".rs", ".dart", ".java", ".kt",
  ".rb", ".php", ".cs", ".swift",
  ".css", ".scss", ".json"
];

// Config files that are never feature components, even though they are JSON.
const IGNORED_FILES = new Set([
  "package.json",
  "package-lock.json",
  "tsconfig.json",
  "jsconfig.json",
  "components.json",
  "vercel.json",
  "next.config.json",
  "eslint.json",
  ".eslintrc.json",
  ".prettierrc.json"
]);

function readConstitutionSources() {
  const constitutionPath = path.join(projectRoot, ".sdrive", "constitution.md");

  if (!fs.existsSync(constitutionPath)) return [];

  const content = fs.readFileSync(constitutionPath, "utf8");
  const matches = content.match(/`([^`]+)\/`/g) || [];

  return matches
    .map((m) => m.replace(/`/g, "").replace(/\/$/, ""))
    .filter((p) => {
      // Keep the top-level segment of a declared path (e.g. "components/ui"
      // becomes "components") and only accept known source roots.
      const top = p.split("/")[0];
      return CANDIDATE_ROOTS.includes(top);
    })
    .map((p) => p.split("/")[0]);
}

function detectSourceRoots() {
  const fromConstitution = readConstitutionSources();
  const detected = CANDIDATE_ROOTS.filter((r) =>
    fs.existsSync(path.join(projectRoot, r))
  );

  // Union the constitution's declared roots with the conventional roots that
  // actually exist. Relying on the constitution alone misses real source
  // folders (e.g. lib/, messages/) that the document simply did not list.
  const roots = [...new Set([...fromConstitution, ...detected])];

  return roots.filter((r) => fs.existsSync(path.join(projectRoot, r)));
}

function walkDirectory(dir, files = []) {
  const entries = fs.readdirSync(dir, { withFileTypes: true });

  for (const entry of entries) {
    if (IGNORED_DIRS.has(entry.name)) continue;
    if (IGNORED_FILES.has(entry.name)) continue;

    const fullPath = path.join(dir, entry.name);

    if (entry.isDirectory()) {
      walkDirectory(fullPath, files);
    } else if (CODE_EXTENSIONS.some((ext) => entry.name.endsWith(ext))) {
      files.push(fullPath);
    }
  }

  return files;
}

function scanCode() {
  const roots = detectSourceRoots();

  if (roots.length === 0) {
    console.warn("⚠️  No source folders detected.");
    return { files: [], roots: [] };
  }

  console.log(`🔍 Scanning roots: ${roots.join(", ")}\n`);

  const files = [];
  roots.forEach((root) => {
    walkDirectory(path.join(projectRoot, root), files);
  });

  console.log(`📄 Found ${files.length} source files.\n`);

  return { files, roots };
}


// ---------- Phase 3: Diff Engine ----------

function readFileContent(filePath) {
  try {
    return fs.readFileSync(filePath, "utf8");
  } catch (error) {
    return "";
  }
}

function detectComponents(files) {
  const components = [];

  files.forEach((file) => {
    const content = readFileContent(file);
    const relative = path.relative(projectRoot, file).replace(/\\/g, "/");
    const basename = path.basename(file, path.extname(file));

    // React / Vue / Svelte components
    const componentMatches = content.match(
      /(?:export\s+)?(?:function|const|class)\s+([A-Z][A-Za-z0-9_]+)/
    );

    if (componentMatches && /components?\//i.test(relative)) {
      components.push({
        name: componentMatches[1],
        file: relative,
        basename
      });
    }

    // Every source file is also a valid "component" target when the plan
    // references it by path (e.g. "components/home/hero.tsx"). Plans commonly
    // name files rather than exported symbols, so record the path too.
    components.push({
      name: relative,
      file: relative,
      basename,
      isPath: true
    });
  });

  return components;
}

function detectEndpoints(files) {
  const endpoints = [];

  files.forEach((file) => {
    const content = readFileContent(file);
    const relative = path.relative(projectRoot, file).replace(/\\/g, "/");

    // Express / Fastify / Koa
    const expressMatches = content.matchAll(
      /(?:app|router)\.(get|post|put|delete|patch)\(\s*['"`]([^'"`]+)['"`]/gi
    );

    for (const match of expressMatches) {
      endpoints.push({
        method: match[1].toUpperCase(),
        path: match[2],
        file: relative
      });
    }

    // FastAPI / Flask
    const fastapiMatches = content.matchAll(
      /@(?:app|router)\.(get|post|put|delete|patch)\(\s*['"`]([^'"`]+)['"`]/gi
    );

    for (const match of fastapiMatches) {
      endpoints.push({
        method: match[1].toUpperCase(),
        path: match[2],
        file: relative
      });
    }

    // Next.js API routes
    if (/pages\/api\//.test(relative)) {
      const route = relative.replace("pages/api", "").replace(/\.[jt]sx?$/, "");
      endpoints.push({
        method: "GET",
        path: `/api${route}`,
        file: relative
      });
    }
  });

  return endpoints;
}

function detectEntities(files) {
  const entities = [];

  files.forEach((file) => {
    const content = readFileContent(file);
    const relative = path.relative(projectRoot, file).replace(/\\/g, "/");

    // SQL CREATE TABLE
    const sqlMatches = content.matchAll(/CREATE TABLE\s+(\w+)/gi);
    for (const match of sqlMatches) {
      entities.push({
        name: match[1],
        file: relative
      });
    }

    // Prisma models
    const prismaMatches = content.matchAll(/model\s+(\w+)/g);
    for (const match of prismaMatches) {
      entities.push({
        name: match[1],
        file: relative
      });
    }

    // TypeORM entities
    const typeormMatches = content.matchAll(/@Entity\([^)]*\)\s*class\s+(\w+)/g);
    for (const match of typeormMatches) {
      entities.push({
        name: match[1],
        file: relative
      });
    }

    // TypeScript type aliases and interfaces (e.g. `export type Sector = {`
    // or `export interface Sector {`). These model the plan's data entities
    // in front-end projects that have no database layer.
    const tsTypeMatches = content.matchAll(
      /(?:export\s+)?(?:type|interface)\s+([A-Z][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*=?\s*\{/g
    );
    for (const match of tsTypeMatches) {
      entities.push({
        name: match[1],
        file: relative
      });
    }
  });

  return entities;
}

function detectFields(files) {
  const fields = [];

  files.forEach((file) => {
    const content = readFileContent(file);
    const relative = path.relative(projectRoot, file).replace(/\\/g, "/");

    // SQL fields inside CREATE TABLE
    const tableBlocks = content.matchAll(
      /CREATE TABLE\s+(\w+)\s*\(([\s\S]*?)\)/gi
    );

    for (const block of tableBlocks) {
      const entity = block[1];
      const body = block[2];

      const fieldMatches = body.matchAll(/^\s*(\w+)\s+\w+/gm);

      for (const field of fieldMatches) {
        fields.push({
          entity,
          name: field[1],
          file: relative
        });
      }
    }

    // Prisma fields
    const prismaBlocks = content.matchAll(/model\s+(\w+)\s*\{([\s\S]*?)\}/g);

    for (const block of prismaBlocks) {
      const entity = block[1];
      const body = block[2];

      const fieldMatches = body.matchAll(/^\s*(\w+)\s+\w+/gm);

      for (const field of fieldMatches) {
        fields.push({
          entity,
          name: field[1],
          file: relative
        });
      }
    }

    // TypeScript type/interface fields (e.g. `id: string;` inside a type).
    // Matches `name: Type` and optional `name?: Type` property declarations.
    const tsBlocks = content.matchAll(
      /(?:export\s+)?(?:type|interface)\s+([A-Z][A-Za-z0-9_]*)\s*(?:<[^>]*>)?\s*=?\s*\{([\s\S]*?)\}/g
    );

    for (const block of tsBlocks) {
      const entity = block[1];
      const body = block[2];

      const fieldMatches = body.matchAll(/^\s*(\w+)\??\s*:\s*([^;\n]+)/gm);

      for (const field of fieldMatches) {
        fields.push({
          entity,
          name: field[1],
          file: relative
        });
      }
    }
  });

  return fields;
}

// Dependencies that are satisfied without appearing in package.json. These are
// virtual/registry-based or bundled toolchains (e.g. shadcn/ui is a CLI that
// copies components into the repo rather than a runtime dependency).
const VIRTUAL_DEPENDENCIES = new Set([
  "shadcn/ui",
  "shadcn",
  "tailwind",
  "tailwindcss",
  "radix-ui",
  "radix",
  "lucide",
  "lucide-react"
]);

function detectDependencies() {
  const pkgPath = path.join(projectRoot, "package.json");

  if (!fs.existsSync(pkgPath)) return [];

  try {
    const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
    return [
      ...Object.keys(pkg.dependencies || {}),
      ...Object.keys(pkg.devDependencies || {})
    ];
  } catch (error) {
    return [];
  }
}

function diffPlanAgainstCode(planItems, code) {
  const matched = [];
  const missing = [];
  const renamed = [];
  const extra = [];

  const codeComponents = detectComponents(code.files);
  const codeEndpoints = detectEndpoints(code.files);
  const codeEntities = detectEntities(code.files);
  const codeFields = detectFields(code.files);
  const codeDependencies = detectDependencies();

  // Expand brace globs such as "messages/{fr,pt,es}.json" into concrete paths
  // so a plan that groups sibling files still matches the real files on disk.
  const expandBraces = (pattern) => {
    const match = pattern.match(/^(.*)\{([^}]+)\}(.*)$/);
    if (!match) return [pattern];
    return match[2]
      .split(",")
      .map((part) => `${match[1]}${part.trim()}${match[3]}`);
  };

  // A plan item matches a code component when the names are equal, when the
  // plan names the file path directly, or when the plan uses a brace glob that
  // expands to the file path.
  const componentMatches = (item, c) => {
    if (c.name === item.name || c.file === item.name) return true;
    if (item.name.includes("{")) {
      return expandBraces(item.name).some((p) => p === c.file || p === c.name);
    }
    return false;
  };

  planItems.forEach((item) => {
    if (item.type === "component") {
      // Plans may reference a component either by exported symbol name
      // ("Hero") or by file path ("components/home/hero.tsx"). Match both.
      const found = codeComponents.find((c) => componentMatches(item, c));

      if (found) {
        matched.push({ ...item, file: found.file });
      } else {
        // possible rename: same folder or similar name
        const similar = codeComponents.find((c) =>
          c.name.toLowerCase().includes(item.name.toLowerCase().slice(0, 5))
        );

        if (similar) {
          renamed.push({ ...item, to: similar.name, file: similar.file });
        } else {
          missing.push(item);
        }
      }
    }

    if (item.type === "endpoint") {
      const found = codeEndpoints.find(
        (e) => e.method === item.method && e.path === item.path
      );

      if (found) {
        matched.push({ ...item, file: found.file });
      } else {
        missing.push(item);
      }
    }

    if (item.type === "entity") {
      const found = codeEntities.find((e) => e.name === item.name);

      if (found) {
        matched.push({ ...item, file: found.file });
      } else {
        missing.push(item);
      }
    }

    if (item.type === "field") {
      const found = codeFields.find(
        (f) => f.entity === item.entity && f.name === item.name
      );

      if (found) {
        matched.push({ ...item, file: found.file });
      } else {
        missing.push(item);
      }
    }

    if (item.type === "dependency") {
      if (
        codeDependencies.includes(item.name) ||
        VIRTUAL_DEPENDENCIES.has(item.name.toLowerCase())
      ) {
        matched.push(item);
      } else {
        missing.push(item);
      }
    }
  });

  // Extra code detection. Only files that live in a folder the plan itself
  // declares are candidates for "unplanned work". A plan for one feature does
  // not own every file in the repository, so pre-existing files from other
  // features (and vendored primitives such as components/ui/*) must not be
  // reported as extra. This keeps the signal focused on genuinely surprising
  // additions inside the feature's own area.
  //
  // A folder counts as feature-owned only when the plan creates a new file in
  // it. Folders the plan merely modifies (e.g. app/[locale]/, messages/) are
  // shared infrastructure that legitimately contains sibling files the plan
  // never mentions.
  const plannedFolders = new Set(
    planItems
      .filter(
        (p) =>
          p.type === "component" &&
          p.name.includes("/") &&
          (p.action || "").toLowerCase() === "create"
      )
      .map((p) => p.name.split("/").slice(0, -1).join("/"))
      .filter(Boolean)
  );

  codeComponents.forEach((c) => {
    if (!c.isPath) return;
    if (!plannedFolders.has(c.file.split("/").slice(0, -1).join("/"))) return;

    const inPlan = planItems.some(
      (p) => p.type === "component" && componentMatches(p, c)
    );

    if (!inPlan) {
      extra.push({ type: "component", name: c.name, file: c.file });
    }
  });

  codeEndpoints.forEach((e) => {
    if (!planItems.some((p) => p.type === "endpoint" && p.path === e.path)) {
      extra.push({ type: "endpoint", method: e.method, path: e.path, file: e.file });
    }
  });

  return { matched, missing, renamed, extra };
}


// ---------- Phase 4: Report Writer ----------

function determineStatus(result) {
  if (result.missing.length > 0) return "fail";
  if (result.renamed.length > 0) return "fail";
  if (result.extra.length > 0) return "warn";
  return "pass";
}

function writeReport(featureName, result, version) {
  const status = determineStatus(result);

  const report = {
    feature: featureName,
    status,
    checkedAt: new Date().toISOString(),
    summary: {
      matched: result.matched.length,
      missing: result.missing.length,
      renamed: result.renamed.length,
      extra: result.extra.length
    },
    items: {
      matched: result.matched,
      missing: result.missing,
      renamed: result.renamed,
      extra: result.extra
    }
  };

  // Write alongside the plan we verified, so the report lands in the same
  // version folder (falling back to the flat layout for older projects).
  const reportDir = resolveGovernanceDir(projectRoot, featureName, version);
  const reportPath = path.join(reportDir, "verification.json");

  fs.writeFileSync(reportPath, JSON.stringify(report, null, 2));

  return { status, reportPath };
}


// ---------- Entry ----------

const featureName = process.argv[2];
const version = process.argv[3];

if (!featureName) {
  console.error("❌ Usage: node verify.js <feature-name> [version]");
  process.exit(1);
}

const plan = loadPlan(featureName, version);
const planItems = extractPlanItems(plan);
const code = scanCode();

const result = diffPlanAgainstCode(planItems, code);
const { status, reportPath } = writeReport(featureName, result, version);

console.log("\n📊 Verification Result\n");
console.log(`✅ Matched:  ${result.matched.length}`);
console.log(`❌ Missing:  ${result.missing.length}`);
console.log(`⚠️  Renamed: ${result.renamed.length}`);
console.log(`➕ Extra:    ${result.extra.length}`);
console.log(`\n🧾 Status:   ${status.toUpperCase()}`);
console.log(`📁 Report:   ${path.relative(projectRoot, reportPath)}\n`);

if (result.missing.length > 0) {
  console.log("❌ Missing:");
  result.missing.forEach((i) => {
    console.log(`  [${i.type}] ${i.name || i.path}`);
  });
}

if (result.renamed.length > 0) {
  console.log("\n⚠️  Renamed:");
  result.renamed.forEach((i) => {
    console.log(`  [${i.type}] ${i.name} → ${i.to}`);
  });
}

if (result.extra.length > 0) {
  console.log("\n➕ Extra (not in plan):");
  result.extra.forEach((i) => {
    console.log(`  [${i.type}] ${i.name || i.path}`);
  });
}

process.exit(status === "fail" ? 1 : 0);

module.exports = {
  loadPlan,
  extractPlanItems,
  scanCode,
  diffPlanAgainstCode,
  writeReport
};