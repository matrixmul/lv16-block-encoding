"use strict";

const { spawnSync } = require("child_process");

const ALLOWED_TAR_TYPES = new Set(["-", "0", "d"]);

function normalizeArchiveEntry(entry) {
  return String(entry || "").replace(/\\/g, "/").replace(/^\.\/+/g, "").replace(/^\/+|\/+$/g, "");
}

function pathSegments(filePath) {
  return normalizeArchiveEntry(filePath).split("/").filter(Boolean);
}

function isSystemMetadataPath(filePath) {
  return pathSegments(filePath).some((segment) => (
    segment === "__MACOSX" ||
    segment === ".DS_Store" ||
    segment === ".AppleDouble" ||
    segment === "PaxHeader" ||
    segment === "pax_global_header" ||
    segment.startsWith("._")
  ));
}

function isEntryInEditableScope(entry, editablePaths) {
  return editablePaths.some((editablePath) => (
    entry === editablePath ||
    entry.startsWith(`${editablePath}/`) ||
    editablePath.startsWith(`${entry}/`)
  ));
}

function tarList(archivePath, args, cwd) {
  const result = spawnSync("tar", [...args, archivePath], {
    cwd,
    encoding: "utf8",
    stdio: ["ignore", "pipe", "pipe"],
    shell: false
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const output = [result.stdout, result.stderr].filter(Boolean).join("\n");
    throw new Error(output || `tar ${args.join(" ")} failed with exit code ${result.status}`);
  }
  return result.stdout.split(/\r?\n/).map((line) => line.trimEnd()).filter(Boolean);
}

function tarTypeName(typeFlag) {
  if (typeFlag === "-" || typeFlag === "0") return "file";
  if (typeFlag === "d") return "directory";
  if (typeFlag === "l") return "symlink";
  if (typeFlag === "h") return "hardlink";
  if (typeFlag === "c") return "character device";
  if (typeFlag === "b") return "block device";
  if (typeFlag === "p") return "fifo";
  return `tar entry type '${typeFlag || "unknown"}'`;
}

function listArchiveEntriesDetailed(archivePath, options = {}) {
  const cwd = options.cwd || process.cwd();
  const names = tarList(archivePath, ["-tzf"], cwd)
    .map((entry) => entry.trim())
    .filter(Boolean);
  const details = tarList(archivePath, ["-tvzf"], cwd);
  if (details.length !== names.length) {
    throw new Error(`tar listing mismatch: ${names.length} names but ${details.length} detail rows`);
  }
  return names.map((raw, index) => {
    const typeFlag = details[index][0] || "";
    return {
      raw,
      normalized: normalizeArchiveEntry(raw),
      typeFlag,
      type: tarTypeName(typeFlag)
    };
  });
}

function validateArchiveEntries(options) {
  const archivePath = options.archivePath;
  const editablePaths = (options.editablePaths || []).map(normalizeArchiveEntry).filter(Boolean);
  const requiredFiles = (options.requiredFiles || []).map(normalizeArchiveEntry).filter(Boolean);
  const label = options.label || "submission archive";
  const entries = listArchiveEntriesDetailed(archivePath, { cwd: options.cwd });
  const errors = [];

  if (entries.length === 0) errors.push(`${label} must not be empty`);
  for (const entry of entries) {
    if (entry.raw.startsWith("/") || entry.normalized.split("/").includes("..")) {
      errors.push(`${label} contains unsafe entry: ${entry.raw}`);
      continue;
    }
    if (isSystemMetadataPath(entry.normalized)) {
      errors.push(`${label} contains system metadata entry: ${entry.normalized}`);
      continue;
    }
    if (!ALLOWED_TAR_TYPES.has(entry.typeFlag)) {
      errors.push(`${label} contains unsupported ${entry.type}: ${entry.normalized}`);
      continue;
    }
    if (!isEntryInEditableScope(entry.normalized, editablePaths)) {
      errors.push(`${label} contains entry outside editable paths: ${entry.normalized}`);
    }
  }

  const files = new Set(entries.filter((entry) => entry.typeFlag === "-" || entry.typeFlag === "0").map((entry) => entry.normalized));
  for (const requiredFile of requiredFiles) {
    if (!files.has(requiredFile)) errors.push(`${label} is missing required file: ${requiredFile}`);
  }
  return { entries, errors };
}

module.exports = {
  isSystemMetadataPath,
  listArchiveEntriesDetailed,
  normalizeArchiveEntry,
  validateArchiveEntries
};
