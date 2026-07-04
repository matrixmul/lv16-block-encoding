#!/usr/bin/env node
import crypto from "node:crypto";
import fs from "node:fs";
import path from "node:path";
import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { fileURLToPath, pathToFileURL } from "node:url";

const require = createRequire(import.meta.url);
const archivePolicy = require("./archive-policy.cjs");

const ROOT_DIR = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const CLI_PATH = path.join(ROOT_DIR, "matrixmul.js");
const apiUrl = String(process.env.MATRIXMUL_WORKER_API_URL || process.env.BEML_WORKER_API_URL || "").replace(/\/$/u, "");
const requestedSubmissionId = String(process.env.MATRIXMUL_WORKER_SUBMISSION_ID || process.env.BEML_WORKER_SUBMISSION_ID || "").trim();
const workerClaim = process.env.MATRIXMUL_WORKER_CLAIM || process.env.BEML_WORKER_CLAIM || "";
const dryRun = ["1", "true", "yes"].includes(String(process.env.MATRIXMUL_WORKER_DRY_RUN || process.env.BEML_WORKER_DRY_RUN || "").toLowerCase());
const workDir = path.join(ROOT_DIR, ".trusted-worker");
const archivePath = path.join(workDir, "submission.tar.gz");
const submissionPath = path.join(workDir, "submission.json");
const validationResultPath = path.join(workDir, "validation-result.json");
const NOTE_TEXT = "trusted worker reproduction";
const VALIDATION_CHECK_ALIASES = new Map([
  ["equivalence to official same-width reference circuit", "same-width MatrixMul oracle validation"],
  ["same-width implementation validation", "same-width MatrixMul oracle validation"],
  ["same-width MatrixMul circuit validation", "same-width MatrixMul oracle validation"]
]);

function fail(message) {
  throw new Error(message);
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(path.resolve(filePath), "utf8"));
}

function writeJson(filePath, value) {
  fs.mkdirSync(path.dirname(path.resolve(filePath)), { recursive: true });
  fs.writeFileSync(path.resolve(filePath), `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function sha256Buffer(buffer) {
  return crypto.createHash("sha256").update(buffer).digest("hex");
}

function sha256File(filePath) {
  return sha256Buffer(fs.readFileSync(path.resolve(filePath)));
}

function requireApiInputs() {
  if (!apiUrl) fail("MATRIXMUL_WORKER_API_URL is required");
  if (!requestedSubmissionId) fail("MATRIXMUL_WORKER_SUBMISSION_ID is required");
}

function workerHeaders(extra = {}) {
  if (!workerClaim) fail("MATRIXMUL_WORKER_CLAIM is required");
  return {
    "x-matrixmul-worker-token": workerClaim,
    "x-beml-worker-token": workerClaim,
    "user-agent": "matrixmul-trusted-worker",
    ...extra
  };
}

async function requestJson(url, options = {}) {
  const response = await fetch(url, {
    ...options,
    headers: workerHeaders({
      accept: "application/json",
      ...(options.body ? { "content-type": "application/json" } : {}),
      ...(options.headers || {})
    })
  });
  const text = await response.text();
  let body = null;
  try {
    body = text ? JSON.parse(text) : null;
  } catch {}
  if (!response.ok) fail(body?.error || text || `HTTP ${response.status}`);
  return body;
}

function run(program, args, options = {}) {
  console.log(`> ${program} ${args.join(" ")}`);
  const result = spawnSync(program, args, {
    cwd: options.cwd || ROOT_DIR,
    env: options.env || process.env,
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
    encoding: options.capture ? "utf8" : undefined,
    shell: false
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    const output = [result.stdout, result.stderr].filter(Boolean).join("\n");
    throw new Error(output || `${program} failed with exit code ${result.status}`);
  }
  return result.stdout || "";
}

function validationEnv() {
  const env = { ...process.env };
  for (const name of [
    "MATRIXMUL_WORKER_CLAIM",
    "BEML_WORKER_CLAIM",
    "MATRIXMUL_GITHUB_TOKEN",
    "BEML_GITHUB_TOKEN",
    "GITHUB_TOKEN"
  ]) {
    delete env[name];
  }
  return env;
}

function assertEqual(label, actual, expected) {
  if (expected !== null && expected !== undefined && expected !== "" && actual !== expected) {
    fail(`${label} mismatch: reproduced=${actual} submitted=${expected}`);
  }
}

function scoresMatch(left, right) {
  const a = Number(left);
  const b = Number(right);
  return Number.isFinite(a) && Number.isFinite(b) && Math.abs(a - b) <= Number.EPSILON * Math.max(1, Math.abs(a), Math.abs(b)) * 8;
}

function validateArchiveEntries(manifest, targetArchivePath = archivePath) {
  const { errors } = archivePolicy.validateArchiveEntries({
    archivePath: targetArchivePath,
    editablePaths: manifest.editablePaths || [],
    requiredFiles: [manifest.architectureDiagram, manifest.notePath].filter(Boolean),
    label: "submission archive",
    cwd: ROOT_DIR
  });
  if (errors.length > 0) fail(errors[0]);
}

function safeRepoPath(repoPath) {
  const normalized = archivePolicy.normalizeArchiveEntry(repoPath);
  if (!normalized) fail("repo path must not be empty");
  if (path.isAbsolute(String(repoPath || ""))) fail(`repo path must be relative: ${repoPath}`);
  if (normalized.split("/").includes("..")) fail(`repo path must not contain '..': ${repoPath}`);
  const fullPath = path.resolve(ROOT_DIR, normalized);
  if (fullPath !== ROOT_DIR && !fullPath.startsWith(`${ROOT_DIR}${path.sep}`)) fail(`repo path escapes root: ${repoPath}`);
  return fullPath;
}

function removeSystemMetadataUnder(dirPath) {
  if (!fs.existsSync(dirPath) || !fs.statSync(dirPath).isDirectory()) return;
  for (const entry of fs.readdirSync(dirPath, { withFileTypes: true })) {
    const fullPath = path.join(dirPath, entry.name);
    if (archivePolicy.isSystemMetadataPath(entry.name)) {
      fs.rmSync(fullPath, { recursive: true, force: true });
      console.log(`removed system metadata: ${path.relative(ROOT_DIR, fullPath) || entry.name}`);
      continue;
    }
    if (entry.isDirectory()) removeSystemMetadataUnder(fullPath);
  }
}

function resetGeneratedOutputs() {
  for (const filePath of ["score.json"]) fs.rmSync(path.join(ROOT_DIR, filePath), { force: true });
  fs.rmSync(path.join(ROOT_DIR, "dist"), { recursive: true, force: true });
}

function removeEditablePaths(manifest) {
  for (const editablePath of manifest.editablePaths || []) {
    fs.rmSync(safeRepoPath(editablePath), { recursive: true, force: true });
  }
}

function scrubSubmissionMetadata(manifest) {
  for (const entry of fs.readdirSync(ROOT_DIR, { withFileTypes: true })) {
    if (archivePolicy.isSystemMetadataPath(entry.name)) fs.rmSync(path.join(ROOT_DIR, entry.name), { recursive: true, force: true });
  }
  for (const editablePath of manifest.editablePaths || []) removeSystemMetadataUnder(safeRepoPath(editablePath));
}

function extractSubmission(manifest, targetArchivePath = archivePath) {
  validateArchiveEntries(manifest, targetArchivePath);
  resetGeneratedOutputs();
  removeEditablePaths(manifest);
  run("tar", ["-xzf", targetArchivePath, "-C", ROOT_DIR]);
  scrubSubmissionMetadata(manifest);
}

async function downloadArchive(submission, targetPath) {
  if (!submission.archive_url) fail("pending submission has no archive_url");
  const response = await fetch(new URL(submission.archive_url, apiUrl).toString(), { headers: workerHeaders() });
  if (!response.ok) fail(`archive download failed with HTTP ${response.status}: ${await response.text().catch(() => "")}`);
  const buffer = Buffer.from(await response.arrayBuffer());
  assertEqual("archive_size_bytes", buffer.length, submission.archive_size_bytes);
  assertEqual("archive_sha256", sha256Buffer(buffer), submission.archive_sha256);
  fs.mkdirSync(path.dirname(targetPath), { recursive: true });
  fs.writeFileSync(targetPath, buffer);
}

function noteFileFor() {
  const notePath = ".trusted-worker-note.md";
  fs.writeFileSync(path.join(ROOT_DIR, notePath), `${NOTE_TEXT}\n`, "utf8");
  return notePath;
}

function canonicalJson(value) {
  if (Array.isArray(value)) return value.map(canonicalJson);
  if (!value || typeof value !== "object") return value;
  return Object.fromEntries(
    Object.keys(value)
      .sort()
      .map((key) => [key, canonicalJson(value[key])])
  );
}

function assertSameJson(label, actual, expected) {
  if (JSON.stringify(canonicalJson(actual)) !== JSON.stringify(canonicalJson(expected))) fail(`${label} mismatch`);
}

function normalizeValidationCheck(check) {
  return VALIDATION_CHECK_ALIASES.get(check) || check;
}

function normalizeValidationForComparison(validation) {
  if (!validation || typeof validation !== "object" || Array.isArray(validation)) return validation;
  return {
    ...validation,
    checks: Array.isArray(validation.checks)
      ? validation.checks.map(normalizeValidationCheck)
      : validation.checks
  };
}

function compareReproducedMetadata(submitted, reproduced) {
  assertEqual("benchmark", reproduced.benchmark, submitted.benchmark);
  assertSameJson("editablePaths", reproduced.editablePaths, submitted.editablePaths);
  assertEqual("scoreModel", reproduced.scoreModel, submitted.scoreModel);
  assertEqual("targetId", reproduced.targetId, submitted.targetId);
  assertEqual("targetMetadataSha256", reproduced.targetMetadataSha256, submitted.targetMetadataSha256);
  if (!scoresMatch(reproduced.localScore, submitted.localScore)) fail(`trusted score ${reproduced.localScore} does not match submitted localScore ${submitted.localScore}`);
  assertSameJson("metrics", reproduced.metrics, submitted.metrics);
  assertSameJson("scoreBreakdown", reproduced.scoreBreakdown, submitted.scoreBreakdown);
  assertSameJson(
    "validation",
    normalizeValidationForComparison(reproduced.validation),
    normalizeValidationForComparison(submitted.validation)
  );
  assertEqual("artifact", reproduced.artifact, submitted.artifact);
  assertEqual("artifactBytes", reproduced.artifactBytes, submitted.artifactBytes);
  assertEqual("artifactSha256", reproduced.artifactSha256, submitted.artifactSha256);
  assertEqual("architectureDiagram.path", reproduced.architectureDiagram?.path, submitted.architectureDiagram?.path);
  assertEqual("architectureDiagram.bytes", reproduced.architectureDiagram?.bytes, submitted.architectureDiagram?.bytes);
  assertEqual("architectureDiagram.sha256", reproduced.architectureDiagram?.sha256, submitted.architectureDiagram?.sha256);
}

function hasStagedChanges() {
  const result = spawnSync("git", ["diff", "--cached", "--quiet"], { cwd: ROOT_DIR, stdio: "ignore", shell: false });
  if (result.error) throw result.error;
  return result.status === 1;
}

function currentCommit() {
  return run("git", ["rev-parse", "HEAD"], { capture: true }).trim();
}

function coAuthorTrailer(submission) {
  if (!submission.author_github_login) return "";
  const login = String(submission.author_github_login).trim();
  if (!/^[A-Za-z0-9](?:[A-Za-z0-9-]{0,37}[A-Za-z0-9])?$/.test(login)) fail("invalid author_github_login for co-author trailer");
  const id = String(submission.author_github_id || submission.author_github_user_id || "").trim();
  const email = /^\d+$/u.test(id) ? `${id}+${login}@users.noreply.github.com` : `${login}@users.noreply.github.com`;
  return `Co-authored-by: ${login} <${email}>`;
}

function acceptCommitMessage(submission) {
  const title = `Accept ${submission.track_id} submission ${submission.submission_id}`;
  const trailer = coAuthorTrailer(submission);
  return trailer ? `${title}\n\n${trailer}` : title;
}

function commitAndPush(submission, manifest) {
  for (const editablePath of manifest.editablePaths || []) run("git", ["add", editablePath]);
  if (!hasStagedChanges()) {
    console.log("No editable-path changes to commit; using current HEAD.");
    return currentCommit();
  }
  run("git", ["config", "user.name", "matrixmul-trusted-worker"]);
  run("git", ["config", "user.email", "matrixmul-trusted-worker@users.noreply.github.com"]);
  run("git", ["commit", "-m", acceptCommitMessage(submission)]);
  run("git", ["push", "origin", "HEAD:main"]);
  return currentCommit();
}

async function callback(submissionId, payload) {
  if (dryRun) {
    console.log(`dry_run callback ${submissionId}: ${JSON.stringify(payload, null, 2)}`);
    return null;
  }
  return requestJson(`${apiUrl}/api/submissions/${encodeURIComponent(submissionId)}/trusted-pass`, {
    method: "POST",
    body: JSON.stringify(payload)
  });
}

async function fetchSubmission() {
  requireApiInputs();
  const manifest = readJson(path.join(ROOT_DIR, "benchmark.json"));
  const pending = await requestJson(`${apiUrl}/api/trusted-worker/submissions/pending?submission_id=${encodeURIComponent(requestedSubmissionId)}&limit=1`);
  const row = (pending.rows || [])[0];
  if (!row) {
    console.log(`No pending submission ${requestedSubmissionId}`);
    return;
  }
  fs.rmSync(workDir, { recursive: true, force: true });
  fs.mkdirSync(workDir, { recursive: true });
  await downloadArchive(row, archivePath);
  validateArchiveEntries(manifest, archivePath);
  writeJson(submissionPath, row);
  console.log(`fetched ${row.submission_id} (${row.track_id})`);
}

function validateSubmission() {
  const manifest = readJson(path.join(ROOT_DIR, "benchmark.json"));
  const submission = readJson(submissionPath);
  console.log(`\nValidating ${submission.submission_id} (${submission.track_id}) without callback credentials`);

  extractSubmission(manifest, archivePath);

  const env = validationEnv();
  run(process.execPath, [CLI_PATH, "preflight"], { env });
  run(process.execPath, [CLI_PATH, "setup"], { env });
  run(process.execPath, [CLI_PATH, "run"], { env });
  run(process.execPath, [CLI_PATH, "package", "--note-file", noteFileFor(), "--model", submission.submitted_model || submission.metadata?.model || "trusted-worker"], { env });
  run(process.execPath, [CLI_PATH, "validate", path.join(ROOT_DIR, "dist", "submission-metadata.json")], { env });

  const reproduced = readJson(path.join(ROOT_DIR, "dist", "submission-metadata.json"));
  compareReproducedMetadata(submission.metadata || {}, reproduced);
  writeJson(validationResultPath, {
    status: "passed",
    submission_id: submission.submission_id,
    archive_sha256: submission.archive_sha256 || sha256File(archivePath),
    score: reproduced.localScore,
    artifact_sha256: reproduced.artifactSha256
  });
  console.log(`trusted validation passed for ${submission.submission_id}`);
}

async function finalizePassedSubmission() {
  requireApiInputs();
  const manifest = readJson(path.join(ROOT_DIR, "benchmark.json"));
  const submission = readJson(submissionPath);
  console.log(`\nFinalizing trusted pass for ${submission.submission_id} (${submission.track_id})`);

  if (dryRun) {
    console.log("dry-run: not committing source or posting trusted-pass");
    return;
  }

  extractSubmission(manifest, archivePath);
  const acceptedCommit = commitAndPush(submission, manifest);
  const validationResult = fs.existsSync(validationResultPath) ? readJson(validationResultPath) : {};
  await callback(submission.submission_id, {
    status: "passed",
    report: {
      repository: process.env.GITHUB_REPOSITORY || null,
      workflow: process.env.GITHUB_WORKFLOW || "Trusted MatrixMul worker",
      run_id: process.env.GITHUB_RUN_ID || null,
      accepted_commit_sha: acceptedCommit,
      archive_sha256: submission.archive_sha256 || sha256File(archivePath),
      score: validationResult.score || submission.metadata?.localScore || null,
      artifact_sha256: validationResult.artifact_sha256 || submission.metadata?.artifactSha256 || null
    }
  });
  console.log(`trusted validation accepted for ${submission.submission_id}`);
}

async function finalizeFailedSubmission() {
  requireApiInputs();
  const submission = readJson(submissionPath);
  await callback(submission.submission_id, {
    status: "failed",
    message: "trusted validation job failed",
    report: {
      repository: process.env.GITHUB_REPOSITORY || null,
      workflow: process.env.GITHUB_WORKFLOW || "Trusted MatrixMul worker",
      run_id: process.env.GITHUB_RUN_ID || null,
      archive_sha256: submission.archive_sha256 || null,
      error: "trusted validation job failed"
    }
  });
}

function printUsage() {
  console.log("Usage: node tools/trusted-worker.mjs <fetch|validate|finalize-pass|finalize-fail>");
}

async function main() {
  const mode = process.argv[2] || "";
  if (mode === "fetch") return fetchSubmission();
  if (mode === "validate") return validateSubmission();
  if (mode === "finalize-pass") return finalizePassedSubmission();
  if (mode === "finalize-fail") return finalizeFailedSubmission();
  printUsage();
  process.exit(mode ? 1 : 0);
}

export {
  acceptCommitMessage,
  coAuthorTrailer,
  archivePolicy as archivePolicyForTests,
  validateArchiveEntries
};

if (process.argv[1] && import.meta.url === pathToFileURL(path.resolve(process.argv[1])).href) {
  main().catch((error) => {
    console.error(`trusted-worker error: ${error.message}`);
    process.exit(1);
  });
}
