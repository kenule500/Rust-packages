const fs = require("fs");
const path = require("path");
const crypto = require("crypto");

function exists(file) {
  return fs.existsSync(file);
}

function readText(file) {
  try {
    return fs.readFileSync(file, "utf8");
  } catch (error) {
    return null;
  }
}

function readJson(file, fallback = null) {
  try {
    return JSON.parse(fs.readFileSync(file, "utf8"));
  } catch (error) {
    return fallback;
  }
}

function ensureDir(dir) {
  fs.mkdirSync(dir, { recursive: true });
}

function listDirs(dir) {
  if (!fs.existsSync(dir)) return [];

  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isDirectory())
    .map((entry) => entry.name);
}

function listFiles(dir) {
  if (!fs.existsSync(dir)) return [];

  return fs
    .readdirSync(dir, { withFileTypes: true })
    .filter((entry) => entry.isFile())
    .map((entry) => entry.name);
}

function findFirst(dir, names) {
  for (const name of names) {
    const full = path.join(dir, name);
    if (fs.existsSync(full)) return full;
  }

  return null;
}

function slugify(value) {
  return String(value)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-+|-+$/g, "")
    .slice(0, 80);
}

function hashFile(file) {
  try {
    return `sha256:${crypto.createHash("sha256").update(fs.readFileSync(file)).digest("hex")}`;
  } catch (error) {
    return null;
  }
}

function relative(root, file) {
  return path.relative(root, file).split(path.sep).join("/");
}

function writeJsonAtomic(file, data) {
  ensureDir(path.dirname(file));
  const temp = `${file}.${process.pid}.${Date.now()}.tmp`;
  fs.writeFileSync(temp, `${JSON.stringify(data, null, 2)}\n`);
  fs.renameSync(temp, file);
}

module.exports = {
  exists,
  readText,
  readJson,
  ensureDir,
  listDirs,
  listFiles,
  findFirst,
  slugify,
  hashFile,
  relative,
  writeJsonAtomic
};
