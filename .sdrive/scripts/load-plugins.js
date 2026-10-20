const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();
const pluginsDir = path.join(projectRoot, ".sdrive", "plugins");

function loadPlugins() {
  if (!fs.existsSync(pluginsDir)) {
    return [];
  }

  const pluginFolders = fs.readdirSync(pluginsDir, { withFileTypes: true })
    .filter((d) => d.isDirectory())
    .map((d) => d.name);

  const plugins = [];

  pluginFolders.forEach((folder) => {
    const pluginJsonPath = path.join(pluginsDir, folder, "plugin.json");

    if (!fs.existsSync(pluginJsonPath)) {
      return;
    }

    try {
      const pluginDef = JSON.parse(fs.readFileSync(pluginJsonPath, "utf8"));
      plugins.push({
        ...pluginDef,
        path: path.join(pluginsDir, folder)
      });
    } catch (error) {
      console.warn(`⚠️  Skipping invalid plugin: ${folder}`);
    }
  });

  return plugins;
}

function runPlugin(pluginName) {
  const plugins = loadPlugins();
  const plugin = plugins.find((p) => p.name === pluginName);

  if (!plugin) {
    console.error(`❌ Plugin "${pluginName}" not found.`);
    process.exit(1);
  }

  const entryPath = path.join(plugin.path, plugin.entry);

  if (!fs.existsSync(entryPath)) {
    console.error(`❌ Plugin entry not found: ${plugin.entry}`);
    process.exit(1);
  }

  require(entryPath);
}

function listPlugins() {
  return loadPlugins();
}

module.exports = {
  loadPlugins,
  runPlugin,
  listPlugins
};