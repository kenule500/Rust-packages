const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();

function readJson(filePath) {
  try {
    if (fs.existsSync(filePath)) {
      return JSON.parse(fs.readFileSync(filePath, "utf8"));
    }
  } catch (error) {
    console.error(`⚠️  Could not parse ${filePath}: ${error.message}`);
  }
  return null;
}

function getInstalledDependencies() {
  const packageJson = readJson(path.join(projectRoot, "package.json"));
  if (!packageJson) return [];

  return [
    ...Object.keys(packageJson.dependencies || {}),
    ...Object.keys(packageJson.devDependencies || {})
  ];
}

function listFiles(dir, extensions = [".ts", ".tsx", ".js", ".jsx", ".py", ".go", ".rs"]) {
  if (!fs.existsSync(dir)) return [];

  let results = [];
  const entries = fs.readdirSync(dir, { withFileTypes: true });

  for (const entry of entries) {
    const fullPath = path.join(dir, entry.name);
    if (entry.isDirectory()) {
      results = results.concat(listFiles(fullPath, extensions));
    } else if (extensions.some((ext) => entry.name.endsWith(ext))) {
      results.push(fullPath);
    }
  }

  return results;
}

function detectExistingAssets() {
  const srcDir = path.join(projectRoot, "src");
  const files = listFiles(srcDir);

  const assets = {
    components: [],
    services: [],
    hooks: [],
    utils: [],
    api: [],
    models: [],
    constants: []
  };

  files.forEach((file) => {
    const relative = path.relative(projectRoot, file).replace(/\\/g, "/");

    if (relative.includes("/components/") || relative.includes("/ui/")) {
      assets.components.push(relative);
    }

    if (relative.includes("/services/") || relative.includes("/api/")) {
      assets.services.push(relative);
    }

    if (relative.includes("/hooks/") || relative.startsWith("use-")) {
      assets.hooks.push(relative);
    }

    if (relative.includes("/utils/") || relative.includes("/helpers/")) {
      assets.utils.push(relative);
    }

    if (relative.includes("/models/") || relative.includes("/entities/")) {
      assets.models.push(relative);
    }

    if (relative.includes("/constants/") || relative.includes("/config/")) {
      assets.constants.push(relative);
    }
  });

  return assets;
}

function checkRedundancy(plannedItems) {
  const existingAssets = detectExistingAssets();
  const installedDependencies = getInstalledDependencies();

  const report = plannedItems.map((item) => {
    let exists = false;
    let location = null;

    switch (item.type) {
      case "component":
        exists = existingAssets.components.some((p) => p.includes(item.name));
        if (exists) {
          location = existingAssets.components.find((p) => p.includes(item.name));
        }
        break;
      case "service":
      case "api":
        exists = existingAssets.services.some((p) => p.includes(item.name));
        if (exists) {
          location = existingAssets.services.find((p) => p.includes(item.name));
        }
        break;
      case "hook":
        exists = existingAssets.hooks.some((p) => p.includes(item.name));
        if (exists) {
          location = existingAssets.hooks.find((p) => p.includes(item.name));
        }
        break;
      case "utility":
        exists = existingAssets.utils.some((p) => p.includes(item.name));
        if (exists) {
          location = existingAssets.utils.find((p) => p.includes(item.name));
        }
        break;
      case "model":
      case "entity":
        exists = existingAssets.models.some((p) => p.includes(item.name));
        if (exists) {
          location = existingAssets.models.find((p) => p.includes(item.name));
        }
        break;
      case "package":
      case "dependency":
        exists = installedDependencies.includes(item.name);
        location = exists ? "package.json" : null;
        break;
      default:
        exists = false;
        location = null;
    }

    return {
      name: item.name,
      type: item.type,
      exists,
      action: exists ? "reuse" : "create",
      location
    };
  });

  return {
    feature: "current",
    items: report,
    existingAssets,
    installedDependencies
  };
}

function main() {
  const plannedItemsPath = process.argv[2];

  if (!plannedItemsPath) {
    console.error("❌ Usage: node anti-redundancy.js <planned-items.json>");
    process.exit(1);
  }

  const plannedItems = readJson(path.resolve(plannedItemsPath));

  if (!plannedItems || !Array.isArray(plannedItems)) {
    console.error("❌ planned-items.json must contain an array of items.");
    process.exit(1);
  }

  const report = checkRedundancy(plannedItems);
  console.log(JSON.stringify(report, null, 2));
}

main();