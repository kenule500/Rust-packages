#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { resolveGovernanceFile, resolveGovernanceDir } = require("./lib/governance-paths");

const projectRoot = process.cwd();

function loadPlan(featureName, version) {
  const planPath = resolveGovernanceFile(projectRoot, featureName, version, "plan.json");

  if (!planPath) {
    console.error(`❌ plan.json not found for feature "${featureName}".`);
    process.exit(1);
  }

  return JSON.parse(fs.readFileSync(planPath, "utf8"));
}

function generateOpenAPI(featureName, version) {
  const plan = loadPlan(featureName, version);

  const openapi = {
    openapi: "3.0.0",
    info: {
      title: `${featureName} API`,
      version: "1.0.0",
      description: `API specification for ${featureName}`
    },
    paths: {}
  };

  const contracts = plan.apiContracts || [];

  contracts.forEach((contract) => {
    const method = contract.method.toLowerCase();
    const apiPath = contract.path;

    if (!openapi.paths[apiPath]) {
      openapi.paths[apiPath] = {};
    }

    openapi.paths[apiPath][method] = {
      summary: `${contract.method} ${apiPath}`,
      security: contract.authenticationRequired ? [{ bearerAuth: [] }] : [],
      responses: {
        200: {
          description: "Successful response"
        }
      }
    };
  });

  openapi.components = {
    securitySchemes: {
      bearerAuth: {
        type: "http",
        scheme: "bearer"
      }
    }
  };

  const outputDir = resolveGovernanceDir(projectRoot, featureName, version);
  const outputPath = path.join(outputDir, "openapi.json");
  fs.writeFileSync(outputPath, JSON.stringify(openapi, null, 2));

  console.log(`✅ Generated OpenAPI spec.`);
  console.log(`   File: ${outputPath}`);
}

const featureName = process.argv[2];
const version = process.argv[3];

if (!featureName) {
  console.error("❌ Usage: node generate-openapi.js <feature-name> [version]");
  process.exit(1);
}

generateOpenAPI(featureName, version);