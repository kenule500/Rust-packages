// Generates the per-tool adapter files described by `.sdrive/commands/tools.json`.
//
// The heavy lifting lives in `lib/tool-registry.js` so `sdrive init` and
// `sdrive adapters:*` share exactly the same planning, dedupe, and content
// generation rules. This file is only the command-line wrapper.

const path = require("path");

const { loadCommands, loadRegistry, writeAdapters } = require("./lib/tool-registry");

const projectRoot = process.cwd();
const sdriveRoot = path.join(projectRoot, ".sdrive");

function loadTools() {
  try {
    return loadRegistry(sdriveRoot).tools;
  } catch (error) {
    console.error(`❌ ${error.message}`);
    console.error("   Run `sdrive init` first.");
    process.exit(1);
  }
}

function generateForTools(tools, options = {}) {
  const result = writeAdapters(tools, projectRoot, {
    force: options.force === true,
    commands: loadCommands(sdriveRoot),
    log: (message) => console.log(message)
  });

  return result;
}

function generateAll(options = {}) {
  const tools = loadTools();
  const result = generateForTools(tools, options);
  console.log(
    `\n🎉 Adapters generated: ${result.written.length} written, ${result.skipped.length} left untouched.`
  );
  return result;
}

function generateSelected(indexes, options = {}) {
  const tools = loadTools();
  const selectedTools = indexes
    .map((index) => tools[index])
    .filter((tool) => Boolean(tool));

  if (selectedTools.length === 0) {
    console.error("❌ No valid tools selected.");
    process.exit(1);
  }

  const result = generateForTools(selectedTools, options);
  console.log(
    `\n🎉 Selected adapters generated: ${result.written.length} written, ${result.skipped.length} left untouched.`
  );
  return result;
}

function generateNamed(names, options = {}) {
  const tools = loadTools();
  const wanted = names.map((name) => name.trim().toLowerCase()).filter(Boolean);
  const selectedTools = tools.filter((tool) => wanted.includes(tool.name.toLowerCase()));
  const unknown = wanted.filter((name) => !selectedTools.some((tool) => tool.name.toLowerCase() === name));

  if (unknown.length > 0) {
    console.error(`❌ Unknown tool(s): ${unknown.join(", ")}`);
    console.error(`   Available: ${tools.map((tool) => tool.name).join(", ")}`);
    process.exit(1);
  }

  const result = generateForTools(selectedTools, options);
  console.log(`\n🎉 Adapters generated: ${result.written.length} written, ${result.skipped.length} left untouched.`);
  return result;
}

module.exports = {
  generateAll,
  generateNamed,
  generateSelected,
  loadTools
};

if (require.main === module) {
  const args = process.argv.slice(2);
  const force = args.includes("--force");
  const toolsFlag = args.find((arg) => arg.startsWith("--tools"));

  if (toolsFlag) {
    const inline = toolsFlag.includes("=") ? toolsFlag.split("=").slice(1).join("=") : "";
    const idx = args.indexOf(toolsFlag);
    const names = (inline || args[idx + 1] || "").split(",");
    generateNamed(names, { force });
  } else {
    generateAll({ force });
  }
}
