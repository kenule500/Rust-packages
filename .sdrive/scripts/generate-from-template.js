const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();

function loadRegistry() {
  const registryPath = path.join(projectRoot, ".sdrive", "templates", "registry.json");

  if (!fs.existsSync(registryPath)) {
    console.error("❌ registry.json not found.");
    process.exit(1);
  }

  return JSON.parse(fs.readFileSync(registryPath, "utf8")).templates;
}

function templateExists(templateName) {
  const registry = loadRegistry();
  return registry.some((t) => t.name === templateName || t.path === templateName);
}

function listTemplates() {
  const registry = loadRegistry();
  console.log("\nAvailable SpecDrive Templates\n");
  registry.forEach((t) => {
    console.log(`  ${t.name} — ${t.description}`);
  });
}

function loadTemplate(templateName, fileName) {
  const templatePath = path.join(projectRoot, ".sdrive", "templates", templateName, fileName);
  return fs.readFileSync(templatePath, "utf8");
}

function applyVariables(content, variables) {
  let result = content;
  Object.keys(variables).forEach((key) => {
    result = result.replaceAll(`{{${key}}}`, variables[key]);
  });
  return result;
}

function generate(templateName, featureName, variables, version) {
  const templateDir = path.join(projectRoot, ".sdrive", "templates", templateName);

  if (!fs.existsSync(templateDir)) {
    console.error(`❌ Template "${templateName}" not found.`);
    listTemplates();
    process.exit(1);
  }

  // Governance JSON belongs in a version folder. Default to v1 when the caller
  // does not specify a version, matching the specification agent's convention.
  const targetVersion = version || "v1";
  const govDir = path.join(projectRoot, ".sdrive", "governance", featureName, targetVersion);

  fs.mkdirSync(govDir, { recursive: true });

  const files = ["spec.json", "plan.json", "tasks.json"];

  files.forEach((file) => {
    const templateFile = path.join(templateDir, file);

    if (!fs.existsSync(templateFile)) {
      return;
    }

    let content = loadTemplate(templateName, file);
    content = applyVariables(content, {
      featureName,
      ...variables
    });

    fs.writeFileSync(path.join(govDir, file), content);
    console.log(`✅ Generated: ${featureName}/${targetVersion}/${file}`);
  });

  console.log(`\n🎉 Feature "${featureName}" generated from "${templateName}" template.`);
}

const templateName = process.argv[2];
const featureName = process.argv[3];
const problemStatement = process.argv[4] || "";
const proposedSolution = process.argv[5] || "";
const targetAudience = process.argv[6] || "";
const approach = process.argv[7] || "";
const flowDescription = process.argv[8] || "";

if (!templateName || !featureName) {
  console.error("❌ Usage: node generate-from-template.js <template> <feature> [problem] [solution] [audience] [approach] [flow]");
  listTemplates();
  process.exit(1);
}

generate(templateName, featureName, {
  title: featureName,
  problemStatement,
  proposedSolution,
  targetAudience,
  approach,
  flowDescription
});