#!/usr/bin/env node

const fs = require("fs");
const path = require("path");
const { execSync } = require("child_process");

const SERVER_NAME = "specdrive";
const PROTOCOL_VERSION = "2025-06-18";
const SUPPORTED_PROTOCOL_VERSIONS = ["2025-06-18", "2025-03-26", "2024-11-05"];

// Gate state comes from the same rules module the CLI uses. The MCP server can
// run from the package (`mcp/../commands`) or from an installed project
// (`.sdrive/mcp/../commands`), so this resolves relative to this file.
let gateRules = null;

try {
  gateRules = require(path.join(__dirname, "..", "commands", "gates.js"));
} catch (error) {
  logDebug(`gate rules unavailable: ${error.message}`);
}

function getServerVersion() {
  try {
    return require("../package.json").version;
  } catch (error) {
    const metadataPath = path.join(__dirname, "mcp.json");
    if (fs.existsSync(metadataPath)) {
      return JSON.parse(fs.readFileSync(metadataPath, "utf8")).version || "0.1.10";
    }
    return "0.1.10";
  }
}

const SERVER_VERSION = getServerVersion();

function logDebug(message) {
  if (process.env.SPECDRIVE_DEBUG === "1") {
    fs.appendFileSync(path.join(process.cwd(), ".sdrive", "mcp-debug.log"), `${new Date().toISOString()} ${message}\n`);
  }
}

function loadWorkflowState() {
  const statePath = path.join(process.cwd(), ".sdrive", "workflow-state.json");
  if (!fs.existsSync(statePath)) return { features: [] };

  try {
    return JSON.parse(fs.readFileSync(statePath, "utf8"));
  } catch (error) {
    logDebug(`loadWorkflowState parse error: ${error.message}`);
    return { features: [] };
  }
}

function listFeatures() {
  const state = loadWorkflowState();
  return state.features || [];
}

function getFeature(name) {
  return listFeatures().find((feature) => feature.name === name) || null;
}

function resolveVersion(featureName, requestedVersion) {
  const feature = getFeature(featureName);
  if (!feature || !Array.isArray(feature.versions)) return null;

  if (!requestedVersion) return feature.currentVersion || feature.versions[feature.versions.length - 1]?.id || null;

  const match = feature.versions.find((version) => version.id === requestedVersion || version.name === requestedVersion);
  return match ? match.id : null;
}

function governancePath(featureName, versionId, fileName) {
  const base = path.join(process.cwd(), ".sdrive", "governance", featureName);
  const versioned = path.join(base, versionId, fileName);
  if (fs.existsSync(versioned)) return versioned;

  const legacy = path.join(base, fileName);
  if (fs.existsSync(legacy)) return legacy;

  return null;
}

function readGovernanceJson(featureName, versionId, fileName) {
  const targetPath = governancePath(featureName, versionId, fileName);
  if (!targetPath || !fs.existsSync(targetPath)) return null;

  try {
    return JSON.parse(fs.readFileSync(targetPath, "utf8"));
  } catch (error) {
    logDebug(`readGovernanceJson parse error for ${featureName}/${fileName}: ${error.message}`);
    return null;
  }
}

function getSpec(featureName, versionId) {
  const resolvedVersion = resolveVersion(featureName, versionId) || versionId;
  return readGovernanceJson(featureName, resolvedVersion, "spec.json") || readGovernanceJson(featureName, versionId, "spec.json");
}

function getTraceability(featureName, versionId) {
  const resolvedVersion = resolveVersion(featureName, versionId) || versionId;
  return readGovernanceJson(featureName, resolvedVersion, "traceability.json") || readGovernanceJson(featureName, versionId, "traceability.json");
}

function runValidator() {
  const validatorPath = path.join(process.cwd(), ".sdrive", "scripts", "validate-governance.js");
  if (!fs.existsSync(validatorPath)) {
    return { success: false, message: "Validator not found" };
  }

  try {
    execSync(`node "${validatorPath}"`, { stdio: "pipe" });
    return { success: true };
  } catch (error) {
    return { success: false, message: error.message };
  }
}

function currentVersion(feature) {
  if (!feature) return null;
  const versions = feature.versions || [];
  return versions.find((version) => version.id === feature.currentVersion) || versions[0] || null;
}

// Mirrors the CLI's `sdrive status` next-action rule so an AI client can ask
// "what now?" instead of guessing from the gates.
function nextAction(feature) {
  const version = currentVersion(feature);

  if (!version) {
    return {
      command: `sdrive version:new ${feature.name}`,
      reason: "The feature has no versions."
    };
  }

  const order = gateRules
    ? gateRules.GATE_ORDER
    : ["specification", "planning", "implementation", "review"];
  const gates = version.gates || {};
  const pending = gateRules
    ? order.find((gate) => !gateRules.isGateApprovedFor(version, gate))
    : order[0];

  if (!pending) {
    return {
      command: `sdrive archive:check ${feature.name}`,
      reason: "Every gate is approved; check whether the feature can be archived."
    };
  }

  if (pending === "review") {
    return {
      command: `sdrive test ${feature.name}`,
      reason: "Review needs passing test evidence; run the tests, then approve review."
    };
  }

  return {
    command: `sdrive approve ${feature.name} ${pending}`,
    reason: `The "${pending}" gate is ${gates[pending] || "pending"}.`
  };
}

function featureStatus(feature) {
  const version = currentVersion(feature);

  return {
    name: feature.name,
    currentVersion: feature.currentVersion || null,
    assignee: feature.assignee || null,
    branch: feature.branch || null,
    base: feature.base || null,
    phase: version ? version.phase : null,
    currentStep: version ? version.currentStep : null,
    gates: version ? version.gates || {} : {},
    nextAction: nextAction(feature)
  };
}

function listResources() {
  const resources = [];

  for (const feature of listFeatures()) {
    const version = currentVersion(feature);
    if (!version) continue;

    for (const kind of ["spec", "plan", "tasks", "traceability"]) {
      resources.push({
        uri: `specdrive://features/${feature.name}/${kind}`,
        name: `${feature.name} ${kind}`,
        description: `${kind}.json for ${feature.name} ${version.id}`,
        mimeType: "application/json"
      });
    }
  }

  return resources;
}

function readResource(uri) {
  const match = /^specdrive:\/\/features\/([^/]+)\/(spec|plan|tasks|traceability)$/.exec(uri || "");

  if (!match) return null;

  const [, featureName, kind] = match;
  const feature = getFeature(featureName);
  if (!feature) return null;

  const version = currentVersion(feature);
  const value = readGovernanceJson(featureName, version ? version.id : null, `${kind}.json`);

  if (!value) return null;

  return {
    contents: [
      {
        uri,
        mimeType: "application/json",
        text: JSON.stringify(value, null, 2)
      }
    ]
  };
}

const TOOLS = [
  {
    name: "specdrive.list_features",
    description: "List all SpecDrive features",
    inputSchema: {
      type: "object",
      properties: {},
      additionalProperties: false
    }
  },
  {
    name: "specdrive.get_feature",
    description: "Get a feature by name",
    inputSchema: {
      type: "object",
      properties: {
        name: { type: "string" }
      },
      required: ["name"],
      additionalProperties: false
    }
  },
  {
    name: "specdrive.get_spec",
    description: "Get a feature spec by name and optional version",
    inputSchema: {
      type: "object",
      properties: {
        name: { type: "string" },
        version: { type: "string" }
      },
      required: ["name"],
      additionalProperties: false
    }
  },
  {
    name: "specdrive.get_traceability",
    description: "Get traceability for a feature and optional version",
    inputSchema: {
      type: "object",
      properties: {
        name: { type: "string" },
        version: { type: "string" }
      },
      required: ["name"],
      additionalProperties: false
    }
  },
  {
    name: "specdrive.validate",
    description: "Run the governance validator",
    inputSchema: {
      type: "object",
      properties: {},
      additionalProperties: false
    }
  },
  {
    name: "specdrive.status",
    description: "Get the feature status summary, including the next action",
    inputSchema: {
      type: "object",
      properties: {},
      additionalProperties: false
    }
  },
  {
    name: "specdrive.next_action",
    description: "Get the next command to run for a feature",
    inputSchema: {
      type: "object",
      properties: {
        name: { type: "string" }
      },
      required: ["name"],
      additionalProperties: false
    }
  }
];

function sendResult(id, result) {
  const payload = {
    jsonrpc: "2.0",
    id,
    result
  };
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

function sendError(id, code, message, data) {
  const payload = {
    jsonrpc: "2.0",
    id,
    error: {
      code,
      message,
      ...(data ? { data } : {})
    }
  };
  process.stdout.write(`${JSON.stringify(payload)}\n`);
}

function handleRequest(message) {
  const { id, method, params = {} } = message;

  if (method === "initialize") {
    const requested = params.protocolVersion;
    const protocolVersion = SUPPORTED_PROTOCOL_VERSIONS.includes(requested)
      ? requested
      : PROTOCOL_VERSION;

    return {
      protocolVersion,
      capabilities: {
        tools: {},
        resources: {}
      },
      serverInfo: {
        name: SERVER_NAME,
        version: SERVER_VERSION
      },
      instructions:
        "SpecDrive governs features through gates. Use specdrive.status / specdrive.next_action to see what is blocking a feature, specdrive.get_spec and the specdrive:// resources to read its artifacts, and specdrive.validate to check governance."
    };
  }

  if (method === "notifications/initialized" || method === "notifications/cancelled") {
    return null;
  }

  if (method === "ping") {
    return { ok: true };
  }

  if (method === "tools/list") {
    return { tools: TOOLS };
  }

  if (method === "resources/list") {
    return { resources: listResources() };
  }

  if (method === "resources/read") {
    const resource = readResource(params.uri);

    if (!resource) {
      throw new Error(`Unknown or unreadable resource: ${params.uri}`);
    }

    return resource;
  }

  if (method === "tools/call") {
    const { name, arguments: args = {} } = params;
    const toolMap = {
      "specdrive.list_features": () => ({ features: listFeatures() }),
      "specdrive.get_feature": () => ({ feature: getFeature(args.name) }),
      "specdrive.get_spec": () => ({ spec: getSpec(args.name, args.version) }),
      "specdrive.get_traceability": () => ({ traceability: getTraceability(args.name, args.version) }),
      "specdrive.validate": () => runValidator(),
      "specdrive.status": () => ({
        count: listFeatures().length,
        features: listFeatures().map(featureStatus)
      }),
      "specdrive.next_action": () => {
        const feature = getFeature(args.name);
        if (!feature) throw new Error(`Feature not found: ${args.name}`);
        return { feature: feature.name, nextAction: nextAction(feature) };
      }
    };

    const handler = toolMap[name];
    if (!handler) {
      throw new Error(`Unknown tool: ${name}`);
    }

    return {
      content: [
        {
          type: "text",
          text: JSON.stringify(handler(), null, 2)
        }
      ]
    };
  }

  throw new Error(`Unknown method: ${method}`);
}

let buffer = "";

process.stdin.on("data", (chunk) => {
  buffer += chunk.toString();

  let newlineIndex;
  while ((newlineIndex = buffer.indexOf("\n")) >= 0) {
    const line = buffer.slice(0, newlineIndex).trim();
    buffer = buffer.slice(newlineIndex + 1);

    if (!line) continue;

    let message;

    try {
      message = JSON.parse(line);
    } catch (error) {
      // A malformed line must not kill the server. The old code called
      // JSON.parse(line) again inside the catch, which threw again and exited.
      sendError(null, -32700, "Parse error: the request was not valid JSON");
      continue;
    }

    if (message.method === "notifications/initialized" || message.method === "notifications/cancelled") {
      continue;
    }

    try {
      const result = handleRequest(message);
      if (result !== null) {
        sendResult(message.id, result);
      }
    } catch (error) {
      sendError(message.id ?? null, -32603, error.message || "Internal error", {
        stack: process.env.SPECDRIVE_DEBUG === "1" ? error.stack : undefined
      });
    }
  }
});

process.stdin.on("end", () => {
  if (buffer.trim()) {
    try {
      const message = JSON.parse(buffer.trim());
      const result = handleRequest(message);
      if (result !== null) {
        sendResult(message.id, result);
      }
    } catch (error) {
      sendError(null, -32603, error.message || "Internal error");
    }
  }
});