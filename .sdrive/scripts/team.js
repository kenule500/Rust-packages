const fs = require("fs");
const path = require("path");

const projectRoot = process.cwd();
const teamPath = path.join(projectRoot, ".sdrive", "team.json");

function loadTeam() {
  if (!fs.existsSync(teamPath)) {
    console.error("❌ team.json not found. Run `sdrive init` first.");
    process.exit(1);
  }

  try {
    return JSON.parse(fs.readFileSync(teamPath, "utf8"));
  } catch (error) {
    console.error(`❌ Invalid team.json: ${error.message}`);
    process.exit(1);
  }
}

function saveTeam(team) {
  fs.writeFileSync(teamPath, JSON.stringify(team, null, 2));
}

function getUserRole(username) {
  const team = loadTeam();
  const member = team.members?.find((m) => m.username === username);
  return member ? member.role : null;
}

function canApprove(username) {
  const role = getUserRole(username);
  if (!role) return false;

  const team = loadTeam();
  return team.rules?.approval_required?.includes(role) || false;
}

function canImplement(username) {
  const role = getUserRole(username);
  if (!role) return false;

  const team = loadTeam();
  return team.rules?.implementation_allowed?.includes(role) || false;
}

function listMembers() {
  const team = loadTeam();
  return team.members || [];
}

function addMember(username, role) {
  const team = loadTeam();

  if (!team.members) {
    team.members = [];
  }

  const existing = team.members.find((m) => m.username === username);
  if (existing) {
    console.error(`❌ User "${username}" already exists.`);
    return false;
  }

  team.members.push({ username, role });
  saveTeam(team);
  console.log(`✅ Added ${username} as ${role}.`);
  return true;
}

function removeMember(username) {
  const team = loadTeam();

  const before = team.members?.length || 0;
  team.members = (team.members || []).filter((m) => m.username !== username);
  const after = team.members.length;

  if (before === after) {
    console.error(`❌ User "${username}" not found.`);
    return false;
  }

  saveTeam(team);
  console.log(`✅ Removed ${username}.`);
  return true;
}

function updateMember(username, role) {
  const team = loadTeam();

  const member = team.members?.find((m) => m.username === username);

  if (!member) {
    console.error(`❌ User "${username}" not found.`);
    return false;
  }

  member.role = role;
  saveTeam(team);
  console.log(`✅ Updated ${username} to ${role}.`);
  return true;
}

module.exports = {
  loadTeam,
  saveTeam,
  getUserRole,
  canApprove,
  canImplement,
  listMembers,
  addMember,
  removeMember,
  updateMember
};