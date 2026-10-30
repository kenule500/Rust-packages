#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { resolveGovernanceFile, resolveGovernanceDir } = require("./lib/governance-paths");
const { isResolved, resolutionOf } = require("./lib/resolution");

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
    items.push({
      type: "component",
      name: c.name,
      action: c.action,
      resolution: isResolved(c) ? resolutionOf(c) : null
    });
  });

  (plan.apiContracts || []).forEach((api) => {
    items.push({
      type: "endpoint",
      // Endpoints have no name of their own; a readable one keeps the withdrawn list
      // legible instead of printing "(unnamed)".
      name: `${api.method} ${api.path}`,
      method: api.method,
      path: api.path,
      action: api.action,
      resolution: isResolved(api) ? resolutionOf(api) : null
    });
  });

  (plan.dataModel?.entities || []).forEach((entity) => {
    const entityResolution = isResolved(entity) ? resolutionOf(entity) : null;

    items.push({
      type: "entity",
      name: entity.name,
      action: entity.action,
      resolution: entityResolution
    });

    (entity.fields || []).forEach((field) => {
      items.push({
        type: "field",
        entity: entity.name,
        name: field.name,
        fieldType: field.type,
        action: field.action,
        // A field of a descoped entity is descoped with it. Without this the report
        // lists every column of a table that will never exist as "missing".
        resolution: entityResolution
      });
    });
  });

  (plan.dependencies || []).forEach((dep) => {
    items.push({
      type: "dependency",
      name: dep.name,
      action: dep.action,
      // `library` is the only dependency kind that is an installed artifact. `internal`,
      // `service`, and `external` describe relationships and access gates, so they can
      // never appear in a lockfile and must not be reported as missing.
      depType: dep.type || "library",
      resolution: isResolved(dep) ? resolutionOf(dep) : null
    });
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
  "crates",
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
  ".css", ".scss", ".json",
  // Manifests and docs that a plan legitimately declares as `modify` components. They
  // are not source, but "this feature changed Cargo.toml / README.md" is a real,
  // checkable claim, and reporting it as missing because of its extension is wrong.
  ".toml", ".md", ".yml", ".yaml"
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
  const matches = content.match(/`([^`]+)`/g) || [];

  // A constitution names a source root in whichever shape the project's own
  // convention uses: `crates/*`, `crates/`, `src/components/`, `crates/<crate>/tests/`.
  // All of them mean the top-level directory to the scanner.
  //
  // The previous pattern required a literal slash immediately before the closing
  // backtick, so `crates/*` - the Cargo workspace convention, and the exact shape a
  // Rust project's constitution uses - never matched. Combined with `crates` being
  // absent from CANDIDATE_ROOTS, that made `sdrive verify` scan zero files in a
  // Rust workspace and report every planned component, entity, and field as missing.
  return matches
    .map((match) => match.replace(/`/g, "").trim())
    .map((declared) => declared.split(/[/\\]/)[0].replace(/\*+$/, ""))
    .filter((top) => top && CANDIDATE_ROOTS.includes(top));
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

    // Rust structs and enums, with or without `pub` and with any visibility
    // qualifier. A plan entity in a Rust crate is a type, so a declared entity was
    // invisible to this scanner until these patterns existed - every Rust model and
    // every one of its fields was reported as "missing" in a file that matched.
    const rustTypeMatches = content.matchAll(
      /\b(?:pub(?:\([^)]*\))?\s+)?(?:struct|enum)\s+([A-Z]\w*)/g
    );
    for (const match of rustTypeMatches) {
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

    // Rust struct fields: `pub name: Type,` inside a `struct Name { ... }` body, which
    // is how a plan's entity fields are written in Rust. Generics such as `Option<T>`
    // and `Vec<T>` are captured as-is; only the field name is used for matching, so the
    // type spelling (`usize`, `String`, `Option<String>`) never affects the result.
    //
    // This is parsed line by line rather than with one regular expression because a
    // field is not required to end in a comma - the last field in a struct often does
    // not - and a pattern that insists on one silently drops that field while its
    // siblings match.
    const rustStructs = content.matchAll(
      /\b(?:pub(?:\([^)]*\))?\s+)?struct\s+([A-Z]\w*)[^;{]*\{([\s\S]*?)\n\s*\}/g
    );

    for (const block of rustStructs) {
      const rustEntity = block[1];

      for (const rawLine of block[2].split(/\r?\n/)) {
        // Drop a trailing inline comment, then attributes and blank lines.
        const line = rawLine.replace(/\/\/.*$/, "").trim();

        if (!line || line.startsWith("#")) continue;

        const field = line.match(
          /^(?:pub(?:\([^)]*\))?\s+)?([a-z_]\w*)\s*:\s*(.+?),?\s*$/
        );

        if (!field) continue;

        fields.push({
          entity: rustEntity,
          name: field[1],
          file: relative
        });
      }
    }

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

// Dependency names declared by a Cargo manifest: `[dependencies]`, `[dev-dependencies]`,
// `[build-dependencies]`, and their `workspace.` forms.
function cargoDependencyNames(file) {
  const content = readFileContent(file);
  const names = [];
  let collecting = false;

  for (const line of content.split(/\r?\n/)) {
    const trimmed = line.trim();

    if (trimmed.startsWith("[")) {
      collecting = /^\[(?:workspace\.)?(?:dev-|build-)?dependencies\]$/.test(trimmed);
      continue;
    }

    if (!collecting) continue;

    const match = trimmed.match(/^([A-Za-z0-9_-]+)\s*=/);

    if (match) names.push(match[1]);
  }

  return names;
}

// Dependencies come from whatever manifest the project actually has.
//
// This read `package.json` only, so in a Rust, Python, or Go project every declared
// library dependency was reported as missing - and a `missing` count keeps
// `verification.json` at "fail" no matter what the code does. The unconditional early
// return also skipped every check when there was no `package.json` at all.
function detectDependencies(sourceRoots = []) {
  const names = new Set();

  const pkgPath = path.join(projectRoot, "package.json");

  if (fs.existsSync(pkgPath)) {
    try {
      const pkg = JSON.parse(fs.readFileSync(pkgPath, "utf8"));
      Object.keys(pkg.dependencies || {}).forEach((name) => names.add(name));
      Object.keys(pkg.devDependencies || {}).forEach((name) => names.add(name));
    } catch (error) {
      // A broken package.json is not a dependency answer; keep looking elsewhere.
    }
  }

  // Rust: the workspace manifest plus each member manifest one level down.
  const cargoFiles = [path.join(projectRoot, "Cargo.toml")];

  for (const root of sourceRoots) {
    const rootDir = path.join(projectRoot, root);

    cargoFiles.push(path.join(rootDir, "Cargo.toml"));

    if (!fs.existsSync(rootDir)) continue;

    for (const entry of fs.readdirSync(rootDir, { withFileTypes: true })) {
      if (entry.isDirectory()) cargoFiles.push(path.join(rootDir, entry.name, "Cargo.toml"));
    }
  }

  cargoFiles
    .filter((file) => fs.existsSync(file))
    .forEach((file) => cargoDependencyNames(file).forEach((name) => names.add(name)));

  // Python
  const requirements = path.join(projectRoot, "requirements.txt");

  if (fs.existsSync(requirements)) {
    readFileContent(requirements)
      .split(/\r?\n/)
      .map((line) => line.trim())
      .filter((line) => line && !line.startsWith("#"))
      .forEach((line) => names.add(line.split(/[<>=!~[ ]/)[0]));
  }

  return [...names];
}

function diffPlanAgainstCode(planItems, code) {
  const matched = [];
  const missing = [];
  const renamed = [];
  const extra = [];
  const withdrawn = [];
  // Files already accounted for by a matched or renamed item. A rename must never
  // claim a file that another plan item already owns.
  const claimedFiles = new Set();

  const codeComponents = detectComponents(code.files);
  const codeEndpoints = detectEndpoints(code.files);
  const codeEntities = detectEntities(code.files);
  const codeFields = detectFields(code.files);
  const codeDependencies = detectDependencies(code.roots);

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
    // A descoped plan item is not missing work - it was decided not to exist. Reporting
    // it as "missing" is what makes a bundled feature that lost one leg unarchivable:
    // the count can never reach zero, so `verification.json` stays "fail" forever and
    // `approve implementation` refuses on evidence nobody can produce.
    if (item.resolution) {
      withdrawn.push({ ...item, reason: item.resolution.reason, kind: item.resolution.kind });
      return;
    }

    if (item.type === "component") {
      // Plans may reference a component either by exported symbol name
      // ("Hero") or by file path ("components/home/hero.tsx"). Match both.
      const found = codeComponents.find((c) => componentMatches(item, c));

      if (found) {
        matched.push({ ...item, file: found.file });
        claimedFiles.add(found.file);
      } else {
        // A rename claims that a similar file IS this plan item under another name.
        // The old test compared the first five characters of the whole path against
        // every code path, so `crates/wirekit/Cargo.toml` "matched"
        // `crates/dado/src/diag.rs` (both begin "crate"), and one file could be
        // claimed by several items. A rename now needs a similar *basename* and a file
        // no other item has claimed; otherwise the item is honestly missing.
        const stem = path.basename(item.name, path.extname(item.name)).toLowerCase();

        const similar = codeComponents.find((c) => {
          if (c.isPath !== true) return false;
          if (claimedFiles.has(c.file)) return false;

          const candidateStem = path.basename(c.name, path.extname(c.name)).toLowerCase();

          if (!candidateStem || !stem) return false;

          return (
            candidateStem === stem ||
            candidateStem.startsWith(stem) ||
            stem.startsWith(candidateStem)
          );
        });

        if (similar && similar.file !== item.name) {
          renamed.push({ ...item, to: similar.name, file: similar.file });
          claimedFiles.add(similar.file);
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
      // A non-library dependency cannot be verified against a manifest, and saying it is
      // "missing" is a false accusation that keeps verification at `fail` forever: an
      // internal reuse of another crate, a partner agreement, or a live-use access gate
      // has no package name to find. It is recorded as matched with a note instead.
      if (item.depType && item.depType !== "library") {
        matched.push({ ...item, note: `${item.depType} dependency; not manifest-verifiable` });
      } else if (
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

  return { matched, missing, renamed, extra, withdrawn };
}


// ---------- Phase 4: Report Writer ----------

function determineStatus(result) {
  if (result.missing.length > 0) return "fail";
  if (result.renamed.length > 0) return "fail";
  if (result.extra.length > 0) return "warn";
  // Nothing matched and nothing is missing means every declared item was descoped.
  // That is not a verified feature, so it cannot report "pass" on an empty diff.
  if (result.withdrawn.length > 0 && result.matched.length === 0) return "warn";
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
      extra: result.extra.length,
      withdrawn: result.withdrawn.length
    },
    items: {
      matched: result.matched,
      missing: result.missing,
      renamed: result.renamed,
      extra: result.extra,
      withdrawn: result.withdrawn
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
console.log(`✅ Matched:   ${result.matched.length}`);
console.log(`❌ Missing:   ${result.missing.length}`);
console.log(`⏸️  Withdrawn: ${result.withdrawn.length}`);
console.log(`⚠️  Renamed:  ${result.renamed.length}`);
console.log(`➕ Extra:     ${result.extra.length}`);
console.log(`\n🧾 Status:    ${status.toUpperCase()}`);
console.log(`📁 Report:    ${path.relative(projectRoot, reportPath)}\n`);

// A field is only meaningful with its entity: "row" says nothing on its own, while
// "AngiImportOutcome.row" says which model is missing it - and makes a rename obvious.
const labelItem = (item) =>
  item.type === "field" ? `${item.entity}.${item.name}` : item.name || item.path || "";

if (result.withdrawn.length > 0) {
  console.log("⏸️  Withdrawn (descoped, not missing):");
  result.withdrawn.forEach((i) => {
    console.log(`  [${i.type}] ${labelItem(i)} - ${i.reason}`);
  });
  console.log("");
}

if (result.missing.length > 0) {
  console.log("❌ Missing:");
  result.missing.forEach((i) => {
    console.log(`  [${i.type}] ${labelItem(i)}`);
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