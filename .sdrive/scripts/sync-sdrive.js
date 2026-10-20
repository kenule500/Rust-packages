#!/usr/bin/env node

// Refreshes the reference installation under .sdrive/ from the source
// directories. The root directories are the source of truth; `.sdrive/` is what
// a consumer project would receive.
const fs = require("fs");
const path = require("path");
const { writeManifest, FRAMEWORK_DIRS } = require("../src/lib/manifest");

const projectRoot = path.resolve(__dirname, "..");
const sdriveRoot = path.join(projectRoot, ".sdrive");

function copyForce(src, dest) {
  fs.mkdirSync(dest, { recursive: true });

  for (const entry of fs.readdirSync(src, { withFileTypes: true })) {
    const from = path.join(src, entry.name);
    const to = path.join(dest, entry.name);

    if (entry.isDirectory()) copyForce(from, to);
    else fs.copyFileSync(from, to);
  }
}

// Remove files under a framework directory that no longer exist in the source.
function prune(src, dest, removed = [], rel = "") {
  if (!fs.existsSync(dest)) return removed;

  for (const entry of fs.readdirSync(dest, { withFileTypes: true })) {
    const from = path.join(src, entry.name);
    const to = path.join(dest, entry.name);
    const relative = rel ? path.join(rel, entry.name) : entry.name;

    if (!fs.existsSync(from)) {
      fs.rmSync(to, { recursive: true, force: true });
      removed.push(relative);
    } else if (entry.isDirectory()) {
      prune(from, to, removed, relative);
    }
  }

  return removed;
}

fs.mkdirSync(sdriveRoot, { recursive: true });

for (const dir of FRAMEWORK_DIRS) {
  const src = path.join(projectRoot, dir);
  const dest = path.join(sdriveRoot, dir);

  if (!fs.existsSync(src)) continue;

  copyForce(src, dest);
  const removed = prune(src, dest);

  console.log(
    `✅ .sdrive/${dir}/ (${removed.length ? `${removed.length} stale removed` : "in sync"})`
  );
}

const logo = path.join(projectRoot, "logo.png");
if (fs.existsSync(logo)) fs.copyFileSync(logo, path.join(sdriveRoot, "logo.png"));

const scope = path.join(sdriveRoot, "package.json");
if (!fs.existsSync(scope)) {
  fs.writeFileSync(scope, `${JSON.stringify({ type: "commonjs" }, null, 2)}\n`);
  console.log("✅ .sdrive/package.json");
}

writeManifest(projectRoot, { cliVersion: require("../package.json").version });
console.log("✅ .sdrive/framework-manifest.json");
