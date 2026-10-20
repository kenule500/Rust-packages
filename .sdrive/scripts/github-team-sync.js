#!/usr/bin/env node

const { execSync } = require("child_process");
const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();

function getRepoInfo() {
  try {
    const remote = execSync("git config --get remote.origin.url", {
      stdio: "pipe",
      encoding: "utf8"
    }).trim();

    const match = remote.match(/github\.com[:/](.+?)\/(.+?)(\.git)?$/);

    if (!match) {
      console.error("❌ Could not detect GitHub repository.");
      return null;
    }

    return { owner: match[1], repo: match[2] };
  } catch (error) {
    console.error("❌ Git not initialized or remote missing.");
    return null;
  }
}

function fetchCollaborators(owner, repo) {
  try {
    const result = execSync(`gh api repos/${owner}/${repo}/collaborators`, {
      stdio: "pipe",
      encoding: "utf8"
    });

    return JSON.parse(result);
  } catch (error) {
    console.error("❌ Could not fetch collaborators. Is gh CLI installed and authenticated?");
    return null;
  }
}

function syncTeam() {
  const repoInfo = getRepoInfo();

  if (!repoInfo) {
    process.exit(1);
  }

  const collaborators = fetchCollaborators(repoInfo.owner, repoInfo.repo);

  if (!collaborators) {
    process.exit(1);
  }

  const teamPath = path.join(projectRoot, ".sdrive", "team.json");

  if (!fs.existsSync(teamPath)) {
    console.error("❌ team.json not found. Run `sdrive init` first.");
    process.exit(1);
  }

  const team = JSON.parse(fs.readFileSync(teamPath, "utf8"));

  collaborators.forEach((user) => {
    const existing = team.members.find((m) => m.username === user.login);

    if (!existing) {
      team.members.push({
        username: user.login,
        role: user.permissions?.admin ? "admin" : "developer"
      });
    }
  });

  fs.writeFileSync(teamPath, JSON.stringify(team, null, 2));
  console.log(`✅ Synced ${collaborators.length} collaborators from GitHub.`);
}

syncTeam();