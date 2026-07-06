#!/usr/bin/env node
"use strict";

const crypto = require("crypto");
const fs = require("fs");
const http = require("http");
const https = require("https");
const os = require("os");
const path = require("path");
const { spawnSync } = require("child_process");
const archivePolicy = require("./tools/archive-policy.cjs");

const CLI_NAME = "matrixmul";
const CLI_SCRIPT = "matrixmul.js";
const DEFAULT_API = "https://matrixmul.com";
const MAX_NOTE_BYTES = 10 * 1024;
const MAX_ARCHIVE_BYTES = 25 * 1024 * 1024;
const REQUIRED_SHOTS = 9024;
const SCORE_MODEL = "logical_hardware_volume_v1";
const REQUIRED_BENCHMARK = "matrixmul-lv16-varq-v3";
const REQUIRED_GATE = "matrixmul_lv16_same_width_qasm_equivalence";
const REQUIRED_EDITABLE_PATHS = ["src/matmul"];
const REQUIRED_ARTIFACT = "dist/solution.qasm";
const REQUIRED_ARCHITECTURE = "src/matmul/architecture.mmd";
const REQUIRED_ARCHITECTURE_LABELS = [
  "Target circuit: MatrixMul LV16",
  "Algorithm",
  "Optimization"
];
const REQUIRED_CHECKS = [
  "same-width MatrixMul oracle validation",
  "same-width QASM ABI",
  "all deterministic product-state probes",
  "MPS tolerance"
];
const VALIDATION_CHECK_ALIASES = new Map([
  ["equivalence to official same-width reference circuit", "same-width MatrixMul oracle validation"],
  ["same-width implementation validation", "same-width MatrixMul oracle validation"],
  ["same-width MatrixMul circuit validation", "same-width MatrixMul oracle validation"]
]);
const VALUE_FLAGS = new Set([
  "--api",
  "--archive",
  "--claimed-score",
  "--manifest",
  "--model",
  "--note",
  "--note-file",
  "--out",
  "--poll-interval",
  "--source-url",
  "--target",
  "--timeout",
  "--track"
]);

const HELP_TEXT = {
  main: `${CLI_NAME} contest CLI

Usage:
  node ${CLI_SCRIPT} <command> [options]
  ${CLI_NAME} <command> [options]

Commands:
  repo         Print the contest repository path
  setup        Build the Rust verifier and generators
  preflight    Cheap source and QASM contract check, no trusted shots
  run          Run the trusted verifier and write score.json
  package      Create dist/submission.tar.gz and submission metadata
  validate     Check a local package against the contest contract
  submit       Upload a validated package to ${DEFAULT_API}
  login        Save a contest API key locally
  config       Show API endpoint and token status
  status       Show or watch submission status
  logs         Print server-side validation logs
  leaderboard  Show ranked submissions

Help:
  ${CLI_NAME} repo --help
  ${CLI_NAME} setup --help
  ${CLI_NAME} preflight --help
  ${CLI_NAME} run --help
  ${CLI_NAME} package --help
  ${CLI_NAME} validate --help
  ${CLI_NAME} submit --help
  ${CLI_NAME} login --help
  ${CLI_NAME} config --help
  ${CLI_NAME} status --help
  ${CLI_NAME} logs --help
  ${CLI_NAME} leaderboard --help

Agent workflow:
  Read README.md, benchmark.json, docs/submission-package-v1.md,
  src/matmul/mod.rs, src/matmul/architecture.mmd, and src/matmul/README.md.

  Only edit files under ${REQUIRED_EDITABLE_PATHS[0]}.

  Optimize src/matmul/mod.rs so it generates a valid OpenQASM 3.0 circuit with
  an allowed declared width from 17 through 42, supported gates only, and lower
  score while matching the same-width trusted verifier contract. Do not use a
  lower-width path that projects or truncates the fixed 42-qubit baseline.

  Keep ${REQUIRED_ARCHITECTURE} updated with the required Algorithm and
  Optimization branches, and keep src/matmul/README.md updated with the
  submitted strategy and evidence. Build within the repository workspace to
  avoid permission issues.

Local loop:
  1. Edit source files under ${REQUIRED_EDITABLE_PATHS[0]}.
  2. Keep ${REQUIRED_ARCHITECTURE} updated with Algorithm and Optimization branches that explain the submission.
  3. Run ${CLI_NAME} preflight.
  4. Run ${CLI_NAME} run only for score candidates. It evaluates all ${REQUIRED_SHOTS} shots.
  5. Run ${CLI_NAME} package --model "<model>".
  6. Run ${CLI_NAME} validate before submitting.`,

  repo: `${CLI_NAME} repo

Usage:
  ${CLI_NAME} repo
  node ${CLI_SCRIPT} repo

Prints the contest repository root used by the installed CLI wrapper. Use
\`cd "$(${CLI_NAME} repo)"\` after installing from https://matrixmul.com/install.sh.`,

  setup: `${CLI_NAME} setup

Usage:
  node ${CLI_SCRIPT} setup

Runs benchmark.json setupCommand from the repository root. Cargo is configured
to build inside this repository under target/.`,

  preflight: `${CLI_NAME} preflight

Usage:
  node ${CLI_SCRIPT} preflight [generated.qasm]

Checks the benchmark manifest, editable path, Algorithm/Optimization architecture diagram, and QASM
prescreen without running trusted shots.
This is a cheap reject path only; it is not submission evidence.`,

  run: `${CLI_NAME} run

Usage:
  node ${CLI_SCRIPT} run [generated.qasm]

Runs the Rust verifier in trusted mode and writes score.json. A ranked score
requires all ${REQUIRED_SHOTS} deterministic product-state shots.`,

  package: `${CLI_NAME} package

Usage:
  node ${CLI_SCRIPT} package --model MODEL [--note-file src/matmul/README.md] [--out dist]

Creates:
  dist/submission.tar.gz
  dist/submission-note.md
  dist/submission-metadata.json

score.json must come from a clean ${REQUIRED_SHOTS}-shot trusted run.
${REQUIRED_ARCHITECTURE} must be a Mermaid diagram committed under editable paths with the target root branching to Algorithm and Optimization explanations.
The generated note begins with \`Model: <LLM>\`; keep the note body focused on what changed and why.`,

  validate: `${CLI_NAME} validate

Usage:
  node ${CLI_SCRIPT} validate [dist/submission-metadata.json]

Checks local package metadata, archive, artifact hash, score formula, editable
path boundary, and ${REQUIRED_SHOTS}-shot validation record.`,

  submit: `${CLI_NAME} submit

Usage:
  node ${CLI_SCRIPT} submit [dist/submission-metadata.json] [--source-url URL] [--watch] [--api ${DEFAULT_API}]

Validates the package, checks the leaderboard score gate, and uploads it.
Submitting requires an API key from the contest server account page.`,

  login: `${CLI_NAME} login

Usage:
  node ${CLI_SCRIPT} login <api-key> [--api ${DEFAULT_API}]`,

  config: `${CLI_NAME} config

Usage:
  node ${CLI_SCRIPT} config [--api ${DEFAULT_API}]`,

  status: `${CLI_NAME} status

Usage:
  node ${CLI_SCRIPT} status <submission-id> [--watch] [--poll-interval 10] [--timeout 0]`,

  logs: `${CLI_NAME} logs

Usage:
  node ${CLI_SCRIPT} logs <submission-id>`,

  leaderboard: `${CLI_NAME} leaderboard

Usage:
  node ${CLI_SCRIPT} leaderboard [--track ${REQUIRED_BENCHMARK}]`
};

function usage(exitCode = 0, command = "main") {
  console.log(HELP_TEXT[command] || HELP_TEXT.main);
  process.exit(exitCode);
}

function hasFlag(args, name) {
  return args.includes(name);
}

function getFlag(args, name, fallback = null) {
  const index = args.indexOf(name);
  if (index === -1) return fallback;
  return args[index + 1] || fallback;
}

function numberFlag(args, name, fallback) {
  const raw = getFlag(args, name, null);
  if (raw === null) return fallback;
  const value = Number(raw);
  if (!Number.isFinite(value) || value < 0) {
    throw new Error(`${name} must be a non-negative number`);
  }
  return value;
}

function firstPositional(args) {
  for (let index = 0; index < args.length; index += 1) {
    const value = args[index];
    if (VALUE_FLAGS.has(value)) {
      index += 1;
      continue;
    }
    if (!value.startsWith("-")) return value;
  }
  return null;
}

function readJson(filePath) {
  return JSON.parse(fs.readFileSync(path.resolve(filePath), "utf8"));
}

function repoManifest(manifestPath = "benchmark.json") {
  const manifest = readJson(manifestPath);
  validateManifestContract(manifest);
  return manifest;
}

function normalizeRepoPath(value) {
  return String(value || "").replace(/\\/g, "/").replace(/^\/+|\/+$/g, "");
}

function assertRepoRelativePath(repoPath, fieldName) {
  const normalized = normalizeRepoPath(repoPath);
  if (!normalized) throw new Error(`${fieldName} must not be empty`);
  if (path.isAbsolute(String(repoPath || ""))) throw new Error(`${fieldName} must be repo-relative: ${repoPath}`);
  if (normalized.split("/").includes("..")) throw new Error(`${fieldName} must not contain '..': ${repoPath}`);
  return normalized;
}

function assertExistingRepoPath(repoPath, fieldName) {
  const normalized = assertRepoRelativePath(repoPath, fieldName);
  if (!fs.existsSync(path.resolve(normalized))) throw new Error(`${fieldName} does not exist: ${normalized}`);
  return normalized;
}

function validateManifestContract(manifest) {
  if (manifest.schemaVersion !== 1) throw new Error("benchmark.json schemaVersion must be 1");
  if (manifest.name !== REQUIRED_BENCHMARK) throw new Error("unsupported benchmark");
  if (manifest.scoreModel !== SCORE_MODEL) throw new Error(`scoreModel must be ${SCORE_MODEL}`);

  const editablePaths = Array.isArray(manifest.editablePaths)
    ? manifest.editablePaths.map((entry) => assertRepoRelativePath(entry, "editablePaths"))
    : [];
  if (!sameStringArray(editablePaths.slice().sort(), REQUIRED_EDITABLE_PATHS.slice().sort())) {
    throw new Error(`editablePaths must be exactly ${REQUIRED_EDITABLE_PATHS.join(", ")}`);
  }
  for (const editablePath of editablePaths) assertExistingRepoPath(editablePath, "editablePaths");

  if (assertRepoRelativePath(manifest.artifact || REQUIRED_ARTIFACT, "artifact") !== REQUIRED_ARTIFACT) {
    throw new Error(`artifact must be ${REQUIRED_ARTIFACT}`);
  }
  if (assertExistingRepoPath(manifest.architectureDiagram || REQUIRED_ARCHITECTURE, "architectureDiagram") !== REQUIRED_ARCHITECTURE) {
    throw new Error(`architectureDiagram must be ${REQUIRED_ARCHITECTURE}`);
  }
  assertExistingRepoPath(manifest.targetPath, "targetPath");
  if (manifest.referencePath) assertExistingRepoPath(manifest.referencePath, "referencePath");
  if (manifest.notePath) assertExistingRepoPath(manifest.notePath, "notePath");
  if (normalizeRepoPath(manifest.scorePath || "score.json") !== "score.json") throw new Error("scorePath must be score.json");
  if (manifest.trustedShots !== REQUIRED_SHOTS) throw new Error(`trustedShots must be ${REQUIRED_SHOTS}`);
  if (manifest.validationGate !== REQUIRED_GATE) throw new Error(`validationGate must be ${REQUIRED_GATE}`);
  const checks = Array.isArray(manifest.requiredValidationChecks) ? manifest.requiredValidationChecks : [];
  for (const check of REQUIRED_CHECKS) {
    if (!checks.includes(check)) throw new Error(`requiredValidationChecks missing ${check}`);
  }
  return { editablePaths };
}

function sha256File(filePath) {
  const hash = crypto.createHash("sha256");
  hash.update(fs.readFileSync(path.resolve(filePath)));
  return hash.digest("hex");
}

function utf8Bytes(value) {
  return Buffer.byteLength(value, "utf8");
}

function architecturePath(manifest) {
  return normalizeRepoPath(manifest.architectureDiagram || REQUIRED_ARCHITECTURE);
}

function validateArchitectureFile(manifest) {
  const relPath = architecturePath(manifest);
  const filePath = path.resolve(relPath);
  if (!fs.existsSync(filePath)) throw new Error(`architecture diagram missing: ${relPath}`);
  const buffer = fs.readFileSync(filePath);
  const maxBytes = manifest.limits?.maxArchitectureBytes || 1024 * 1024;
  if (buffer.length <= 0 || buffer.length > maxBytes) {
    throw new Error(`architecture diagram must be between 1 and ${maxBytes} bytes`);
  }
  const text = buffer.toString("utf8");
  if (text.includes("\uFFFD")) throw new Error(`${relPath} must be valid UTF-8`);
  const meaningfulLines = text
    .split(/\r?\n/)
    .map((line) => line.replace(/%%.*$/u, "").trim())
    .filter(Boolean);
  if (!/^(flowchart|graph)\s+(TD|TB|BT|LR|RL)\b/u.test(meaningfulLines[0] || "")) {
    throw new Error(`${relPath} must start with a Mermaid flowchart or graph declaration`);
  }
  for (const label of REQUIRED_ARCHITECTURE_LABELS) {
    if (!text.includes(label)) throw new Error(`${relPath} missing Mermaid label: ${label}`);
  }
  return {
    path: relPath,
    bytes: buffer.length,
    sha256: crypto.createHash("sha256").update(buffer).digest("hex")
  };
}

function configuredTargetDirEnv() {
  if (process.env.CARGO_TARGET_DIR) return {};
  const cargoConfig = path.resolve(".cargo", "config.toml");
  if (!fs.existsSync(cargoConfig)) return {};
  const text = fs.readFileSync(cargoConfig, "utf8");
  const match = text.match(/^\s*target-dir\s*=\s*["']([^"']+)["']/m);
  return match ? { CARGO_TARGET_DIR: match[1] } : {};
}

function runProcess(program, args, options = {}) {
  const result = spawnSync(program, args, {
    cwd: process.cwd(),
    env: { ...process.env, ...configuredTargetDirEnv() },
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "pipe"] : "inherit",
    shell: false
  });
  if (result.error) throw result.error;
  if (result.status !== 0) {
    if (options.capture && result.stderr) process.stderr.write(result.stderr);
    throw new Error(`${program} ${args.join(" ")} failed with exit code ${result.status}`);
  }
  return result.stdout || "";
}

function runManifestCommand(field) {
  const manifest = repoManifest();
  const command = manifest[field];
  if (!Array.isArray(command) || command.length === 0) throw new Error(`benchmark.json ${field} is missing`);
  const [program, ...args] = command;
  console.log(`> ${[program, ...args].join(" ")}`);
  runProcess(program, args);
}

function generateArtifact(manifest, artifact = REQUIRED_ARTIFACT) {
  const normalized = assertRepoRelativePath(artifact, "artifact");
  runProcess("cargo", [
    "run",
    "--release",
    "--bin",
    "generate-solution",
    "--",
    "--target",
    manifest.targetPath,
    "--output",
    normalized
  ]);
  return normalized;
}

function verifierArgs(candidate, extra = []) {
  const manifest = repoManifest();
  const args = [
    "run",
    "--release",
    "--bin",
    "verify",
    "--",
    candidate,
    "--target",
    manifest.targetPath,
    ...extra
  ];
  if (manifest.referencePath) args.splice(args.length - extra.length, 0, "--reference", manifest.referencePath);
  return args;
}

function runVerifier(candidate, extra = []) {
  const stdout = runProcess("cargo", verifierArgs(candidate, [...extra, "--json"]), { capture: true });
  const start = stdout.indexOf("{");
  if (start === -1) throw new Error("verifier did not print JSON");
  return JSON.parse(stdout.slice(start));
}

function targetMetadata(manifest) {
  return readJson(manifest.targetPath);
}

function scoreFromReport(report, candidate, manifest) {
  const target = targetMetadata(manifest);
  const validation = report.validation || {};
  const ranked = Boolean(report.ok)
    && validation.mode === "trusted"
    && validation.evaluated_shots === REQUIRED_SHOTS
    && validation.trusted_shots === REQUIRED_SHOTS;
  return {
    schemaVersion: 1,
    benchmark: manifest.name,
    targetId: report.target_id,
    targetMetadataSha256: target.metadata_sha256,
    status: ranked ? "ranked" : report.ok ? "checked" : "failed",
    score: report.score,
    score_model: report.score_breakdown?.model || manifest.scoreModel,
    scoreBreakdown: report.score_breakdown,
    costGuard: report.cost_guard,
    metrics: report.metrics,
    validation: {
      shots: validation.evaluated_shots || 0,
      trustedShots: validation.trusted_shots || REQUIRED_SHOTS,
      mode: validation.mode || "unknown",
      gate: REQUIRED_GATE,
      checks: REQUIRED_CHECKS
    },
    artifact: normalizeRepoPath(candidate),
    artifactSha256: report.candidate_sha256,
    referenceSha256: report.reference_sha256,
    generatedAt: new Date().toISOString()
  };
}

function writeJson(filePath, value) {
  fs.writeFileSync(path.resolve(filePath), `${JSON.stringify(value, null, 2)}\n`, "utf8");
}

function printVerifierSummary(report) {
  console.log(`ok: ${Boolean(report.ok)}`);
  console.log(`score: ${Number(report.score || 0).toFixed(6)}`);
  const validation = report.validation || {};
  console.log(`validation: ${validation.mode || "unknown"}, evaluated_shots=${validation.evaluated_shots || 0}, trusted_shots=${validation.trusted_shots || REQUIRED_SHOTS}`);
  const summary = report.shot_summary || {};
  if (summary.evaluated_shots) {
    console.log(`shot summary: min_fidelity=${Number(summary.min_fidelity || 0).toFixed(12)}, max_infidelity=${Number(summary.max_infidelity || 0).toExponential(3)}, max_norm_delta=${Number(summary.max_norm_delta || 0).toExponential(3)}`);
  }
  if (Array.isArray(report.errors) && report.errors.length) {
    console.log("errors:");
    for (const error of report.errors) console.log(`  - ${error}`);
  }
}

function preflight(args) {
  const manifest = repoManifest(getFlag(args, "--manifest", "benchmark.json"));
  validateArchitectureFile(manifest);
  const candidate = firstPositional(args) || generateArtifact(manifest, manifest.artifact || REQUIRED_ARTIFACT);
  assertExistingRepoPath(candidate, "candidate");
  const report = runVerifier(candidate, ["--preflight"]);
  printVerifierSummary(report);
  process.exit(report.ok ? 0 : 1);
}

function runTrusted(args) {
  const manifest = repoManifest(getFlag(args, "--manifest", "benchmark.json"));
  const candidate = firstPositional(args) || generateArtifact(manifest, manifest.artifact || REQUIRED_ARTIFACT);
  const report = runVerifier(candidate);
  printVerifierSummary(report);
  const score = scoreFromReport(report, candidate, manifest);
  writeJson(manifest.scorePath || "score.json", score);
  console.log(`score: ${manifest.scorePath || "score.json"}`);
  process.exit(report.ok ? 0 : 1);
}

function sameStringArray(left, right) {
  if (left.length !== right.length) return false;
  return left.every((value, index) => value === right[index]);
}

function normalizeValidationCheck(check) {
  return VALIDATION_CHECK_ALIASES.get(check) || check;
}

function hasValidationCheck(checks, required) {
  const normalized = new Set(checks.map(normalizeValidationCheck));
  return normalized.has(normalizeValidationCheck(required));
}

function scoresMatch(left, right) {
  if (!Number.isFinite(left) || !Number.isFinite(right)) return false;
  return Math.abs(left - right) <= Number.EPSILON * Math.max(1, Math.abs(left), Math.abs(right)) * 8;
}

function localScore(metrics, scoreBreakdown) {
  const qubits = Number(scoreBreakdown?.qubits || 0);
  const weightedGateVolume = Number(metrics?.weighted_gate_volume || 0);
  const weightedDepth = Number(metrics?.weighted_depth || 0);
  return qubits * Math.sqrt(weightedGateVolume * weightedDepth);
}

function validateScoreJson(score, manifest) {
  if (score.schemaVersion !== 1) throw new Error("score.json schemaVersion must be 1");
  if (score.benchmark !== manifest.name) throw new Error(`score.json benchmark must be ${manifest.name}`);
  if (score.status !== "ranked") throw new Error("score.json status must be ranked");
  if (score.score_model !== SCORE_MODEL) throw new Error(`score.json score_model must be ${SCORE_MODEL}`);
  if (score.artifact !== REQUIRED_ARTIFACT) throw new Error(`score.json artifact must be ${REQUIRED_ARTIFACT}`);
  if (score.validation?.shots !== REQUIRED_SHOTS || score.validation?.trustedShots !== REQUIRED_SHOTS) {
    throw new Error(`score.json must record all ${REQUIRED_SHOTS} trusted shots`);
  }
  if (score.validation?.gate !== REQUIRED_GATE) throw new Error(`score.json validation.gate must be ${REQUIRED_GATE}`);
  for (const check of REQUIRED_CHECKS) {
    if (!hasValidationCheck(score.validation?.checks || [], check)) throw new Error(`score.json validation.checks missing ${check}`);
  }
  const recomputed = localScore(score.metrics, score.scoreBreakdown);
  if (!scoresMatch(Number(score.score), recomputed)) {
    throw new Error(`score.json score does not match weighted hardware formula (${recomputed})`);
  }
  if (!fs.existsSync(path.resolve(score.artifact))) throw new Error(`artifact missing: ${score.artifact}`);
  if (sha256File(score.artifact) !== String(score.artifactSha256).toLowerCase()) {
    throw new Error(`artifact hash mismatch: ${score.artifact}`);
  }
}

function archivePackageErrors(manifest, metadataPath, metadata) {
  if (!metadataPath || !metadata?.archive) return [];
  if (metadata.archive !== "submission.tar.gz") return [];
  const errors = [];
  const archivePath = path.resolve(path.dirname(path.resolve(metadataPath)), metadata.archive);
  if (!fs.existsSync(archivePath)) {
    errors.push(`${metadata.archive} is missing beside ${path.basename(metadataPath)}`);
    return errors;
  }

  const stat = fs.statSync(archivePath);
  if (Number.isInteger(metadata.archiveBytes) && metadata.archiveBytes !== stat.size) {
    errors.push(`archiveBytes does not match local ${metadata.archive}`);
  }

  let result;
  try {
    result = archivePolicy.validateArchiveEntries({
      archivePath,
      editablePaths: manifest.editablePaths || [],
      requiredFiles: [architecturePath(manifest), manifest.notePath].filter(Boolean),
      label: metadata.archive,
      cwd: process.cwd()
    });
  } catch (error) {
    errors.push(`could not inspect ${metadata.archive}: ${error.message}`);
    return errors;
  }
  errors.push(...result.errors);
  return errors;
}

function packageSidecarErrors(metadataPath, metadata) {
  if (!metadataPath || !metadata?.note) return [];
  if (metadata.note !== "submission-note.md") return [];
  const errors = [];
  const notePath = path.resolve(path.dirname(path.resolve(metadataPath)), metadata.note);
  if (!fs.existsSync(notePath)) {
    errors.push(`${metadata.note} is missing beside ${path.basename(metadataPath)}`);
    return errors;
  }
  const noteBytes = fs.statSync(notePath).size;
  if (Number.isInteger(metadata.noteBytes) && metadata.noteBytes !== noteBytes) {
    errors.push(`noteBytes does not match local ${metadata.note}`);
  }
  return errors;
}

function packageSubmission(args) {
  const manifest = repoManifest(getFlag(args, "--manifest", "benchmark.json"));
  const model = getFlag(args, "--model");
  if (!model || !model.trim()) throw new Error("--model is required");
  const noteFile = getFlag(args, "--note-file", manifest.notePath || "src/matmul/README.md");
  if (!fs.existsSync(path.resolve(noteFile))) throw new Error(`note file not found: ${noteFile}`);
  const rawNote = fs.readFileSync(path.resolve(noteFile), "utf8");
  if (!rawNote.trim()) throw new Error("submission note must not be empty");
  const submissionNote = `Model: ${model.trim()}\n\n${rawNote}`;
  const noteBytes = utf8Bytes(submissionNote);
  if (noteBytes > MAX_NOTE_BYTES) throw new Error(`submission note must be at most ${MAX_NOTE_BYTES} bytes`);

  const score = readJson(manifest.scorePath || "score.json");
  generateArtifact(manifest, manifest.artifact || REQUIRED_ARTIFACT);
  validateScoreJson(score, manifest);
  const architecture = validateArchitectureFile(manifest);

  const editablePaths = (manifest.editablePaths || []).map(normalizeRepoPath);
  const outDir = getFlag(args, "--out", "dist");
  fs.mkdirSync(path.resolve(outDir), { recursive: true });
  const archivePath = path.resolve(outDir, "submission.tar.gz");
  const notePath = path.resolve(outDir, "submission-note.md");
  const metadataPath = path.resolve(outDir, "submission-metadata.json");
  try { fs.unlinkSync(archivePath); } catch {}

  const tar = spawnSync("tar", ["--format=ustar", "-czf", archivePath, "-C", process.cwd(), ...editablePaths], {
    env: {
      ...process.env,
      COPYFILE_DISABLE: "1",
      COPY_EXTENDED_ATTRIBUTES_DISABLE: "1"
    },
    stdio: "inherit",
    shell: false
  });
  if (tar.error) throw tar.error;
  if (tar.status !== 0) throw new Error(`tar failed with exit code ${tar.status}`);
  const archiveBytes = fs.statSync(archivePath).size;
  if (archiveBytes > MAX_ARCHIVE_BYTES) throw new Error(`submission archive must be at most ${MAX_ARCHIVE_BYTES} bytes`);
  const archiveSha256 = sha256File(archivePath);

  fs.writeFileSync(notePath, submissionNote, "utf8");
  const artifactBytes = fs.statSync(path.resolve(score.artifact)).size;
  const metadata = {
    schemaVersion: 1,
    benchmark: manifest.name,
    editablePaths,
    archive: "submission.tar.gz",
    archiveBytes,
    archiveSha256,
    note: "submission-note.md",
    noteBytes,
    model: model.trim(),
    claimedScore: getFlag(args, "--claimed-score") ? Number(getFlag(args, "--claimed-score")) : null,
    localScore: score.score,
    scoreModel: score.score_model,
    targetId: score.targetId,
    targetMetadataSha256: score.targetMetadataSha256,
    scoreBreakdown: score.scoreBreakdown,
    costGuard: score.costGuard,
    metrics: score.metrics,
    validation: score.validation,
    artifact: score.artifact,
    artifactBytes,
    artifactSha256: score.artifactSha256,
    architectureDiagram: architecture,
    generatedAt: new Date().toISOString()
  };
  writeJson(metadataPath, metadata);
  const packageErrors = [
    ...archivePackageErrors(manifest, metadataPath, metadata),
    ...packageSidecarErrors(metadataPath, metadata)
  ];
  if (packageErrors.length > 0) throw new Error(packageErrors[0]);
  console.log(`Packaged editable paths: ${editablePaths.join(", ")}`);
  console.log(`Archive: ${path.relative(process.cwd(), archivePath)} (${archiveBytes} bytes)`);
  console.log(`Artifact: ${score.artifact} (${artifactBytes} bytes, sha256 ${score.artifactSha256})`);
  console.log(`Architecture: ${architecture.path} (${architecture.bytes} bytes, sha256 ${architecture.sha256})`);
  console.log(`Note: ${path.relative(process.cwd(), notePath)} (${noteBytes} bytes)`);
  console.log(`Metadata: ${path.relative(process.cwd(), metadataPath)}`);
}

function defaultSubmissionPath() {
  for (const candidate of [path.resolve("dist", "submission-metadata.json"), path.resolve("submission-metadata.json")]) {
    if (fs.existsSync(candidate)) return candidate;
  }
  throw new Error(`submission metadata not found; run node ${CLI_SCRIPT} package or pass a metadata path`);
}

function validatePackage(metadata, options = {}) {
  const logs = [];
  const error = (code, message) => logs.push({ level: "error", code, message });
  const info = (code, message) => logs.push({ level: "info", code, message });
  const manifest = options.manifest || repoManifest();

  if (!metadata || typeof metadata !== "object" || Array.isArray(metadata)) {
    error("PACKAGE_ROOT", "submission metadata must be a JSON object");
    return { ok: false, logs, score: null, trackId: manifest.name };
  }
  if (metadata.artifact === REQUIRED_ARTIFACT) {
    try {
      generateArtifact(manifest, metadata.artifact);
    } catch (artifactError) {
      error("PACKAGE_ARTIFACT_GENERATION", artifactError.message);
    }
  }
  if (metadata.schemaVersion !== 1) error("PACKAGE_SCHEMA_VERSION", "schemaVersion must be 1");
  if (metadata.benchmark !== manifest.name) error("PACKAGE_BENCHMARK", `benchmark must be ${manifest.name}`);
  const editablePaths = Array.isArray(metadata.editablePaths) ? metadata.editablePaths.map(normalizeRepoPath) : [];
  const expectedEditablePaths = (manifest.editablePaths || []).map(normalizeRepoPath);
  if (!sameStringArray(editablePaths.slice().sort(), expectedEditablePaths.slice().sort())) {
    error("PACKAGE_EDITABLE_PATHS", `editablePaths must be exactly ${expectedEditablePaths.join(", ")}`);
  }
  if (metadata.archive !== "submission.tar.gz") error("PACKAGE_ARCHIVE", "archive must be submission.tar.gz");
  if (!Number.isInteger(metadata.archiveBytes) || metadata.archiveBytes <= 0 || metadata.archiveBytes > MAX_ARCHIVE_BYTES) {
    error("PACKAGE_ARCHIVE_BYTES", `archiveBytes must be between 1 and ${MAX_ARCHIVE_BYTES}`);
  }
  if (typeof metadata.archiveSha256 !== "string" || !/^[0-9a-f]{64}$/i.test(metadata.archiveSha256)) {
    error("PACKAGE_ARCHIVE_SHA256", "archiveSha256 must be a 64-character SHA-256 hex digest");
  }
  for (const message of archivePackageErrors(manifest, options.metadataPath || null, metadata)) {
    error("PACKAGE_ARCHIVE", message);
  }
  if (options.metadataPath && metadata.archive === "submission.tar.gz") {
    const archivePath = path.resolve(path.dirname(path.resolve(options.metadataPath)), metadata.archive);
    if (fs.existsSync(archivePath)) {
      const archiveBytes = fs.statSync(archivePath).size;
      const archiveSha256 = sha256File(archivePath);
      if (metadata.archiveBytes !== archiveBytes) error("PACKAGE_ARCHIVE_BYTES", `archiveBytes does not match local ${metadata.archive}`);
      if (metadata.archiveSha256?.toLowerCase() !== archiveSha256) error("PACKAGE_ARCHIVE_SHA256", `archiveSha256 does not match local ${metadata.archive}`);
    }
  }
  if (metadata.note !== "submission-note.md") error("PACKAGE_NOTE", "note must be submission-note.md");
  if (!Number.isInteger(metadata.noteBytes) || metadata.noteBytes <= 0 || metadata.noteBytes > MAX_NOTE_BYTES) {
    error("PACKAGE_NOTE_BYTES", `noteBytes must be between 1 and ${MAX_NOTE_BYTES}`);
  }
  for (const message of packageSidecarErrors(options.metadataPath || null, metadata)) {
    error("PACKAGE_NOTE", message);
  }
  if (typeof metadata.model !== "string" || !metadata.model.trim()) error("PACKAGE_MODEL", "model must be a non-empty string");
  if (metadata.scoreModel !== SCORE_MODEL) error("PACKAGE_SCORE_MODEL", `scoreModel must be ${SCORE_MODEL}`);
  if (metadata.targetId !== manifest.name) error("PACKAGE_TARGET_ID", `targetId must be ${manifest.name}`);
  const target = targetMetadata(manifest);
  const targetHash = target.metadata_sha256;
  if (metadata.targetMetadataSha256 !== targetHash) error("PACKAGE_TARGET_HASH", `targetMetadataSha256 must be ${targetHash}`);
  const qubits = Number(metadata.scoreBreakdown?.qubits);
  const minQubits = Number(target.limits?.min_qubits ?? target.logical_level ?? 1);
  const maxQubits = Number(target.limits?.max_qubits ?? target.qubits);
  if (!Number.isFinite(qubits) || qubits < 0) {
    error("PACKAGE_QUBITS", "scoreBreakdown.qubits must be a non-negative finite number");
  } else {
    if (Number.isFinite(minQubits) && qubits < minQubits) {
      error("PACKAGE_QUBITS", `scoreBreakdown.qubits must be at least ${minQubits}`);
    }
    if (Number.isFinite(maxQubits) && qubits > maxQubits) {
      error("PACKAGE_QUBITS", `scoreBreakdown.qubits must be at most ${maxQubits}`);
    }
  }

  const score = localScore(metadata.metrics, metadata.scoreBreakdown);
  if (!scoresMatch(Number(metadata.localScore), score)) {
    error("PACKAGE_SCORE", `localScore must equal qubits * sqrt(weighted_gate_volume * weighted_depth) (${score})`);
  }
  if (metadata.claimedScore !== null && metadata.claimedScore !== undefined && !scoresMatch(Number(metadata.claimedScore), score)) {
    logs.push({ level: "warn", code: "PACKAGE_CLAIM_MISMATCH", message: `claimedScore=${metadata.claimedScore} differs from recomputed ${score}` });
  }
  if (metadata.validation?.shots !== REQUIRED_SHOTS || metadata.validation?.trustedShots !== REQUIRED_SHOTS) {
    error("PACKAGE_VALIDATION_SHOTS", `validation shots must be ${REQUIRED_SHOTS}`);
  }
  if (metadata.validation?.mode !== "trusted") error("PACKAGE_VALIDATION_MODE", "validation.mode must be trusted");
  if (metadata.validation?.gate !== REQUIRED_GATE) error("PACKAGE_VALIDATION_GATE", `validation.gate must be ${REQUIRED_GATE}`);
  const checks = Array.isArray(metadata.validation?.checks) ? metadata.validation.checks : [];
  for (const check of REQUIRED_CHECKS) {
    if (!hasValidationCheck(checks, check)) error("PACKAGE_VALIDATION_CHECK", `validation.checks must include '${check}'`);
  }
  if (metadata.artifact !== REQUIRED_ARTIFACT) error("PACKAGE_ARTIFACT", `artifact must be ${REQUIRED_ARTIFACT}`);
  if (!Number.isInteger(metadata.artifactBytes) || metadata.artifactBytes <= 0) error("PACKAGE_ARTIFACT_BYTES", "artifactBytes must be a positive integer");
  if (typeof metadata.artifactSha256 !== "string" || !/^[0-9a-f]{64}$/i.test(metadata.artifactSha256)) {
    error("PACKAGE_ARTIFACT_SHA256", "artifactSha256 must be a 64-character SHA-256 hex digest");
  }
  if (fs.existsSync(path.resolve(metadata.artifact || ""))) {
    const stat = fs.statSync(path.resolve(metadata.artifact));
    const digest = sha256File(metadata.artifact);
    if (metadata.artifactBytes !== stat.size) error("PACKAGE_ARTIFACT_BYTES", `artifactBytes does not match local ${metadata.artifact}`);
    if (metadata.artifactSha256?.toLowerCase() !== digest) error("PACKAGE_ARTIFACT_SHA256", `artifactSha256 does not match local ${metadata.artifact}`);
  }
  let localArchitecture = null;
  try {
    localArchitecture = validateArchitectureFile(manifest);
  } catch (architectureError) {
    error("PACKAGE_ARCHITECTURE_LOCAL", architectureError.message);
  }
  const architecture = metadata.architectureDiagram;
  if (!architecture || typeof architecture !== "object" || Array.isArray(architecture)) {
    error("PACKAGE_ARCHITECTURE_METADATA", "architectureDiagram is required");
  } else {
    if (normalizeRepoPath(architecture.path) !== architecturePath(manifest)) {
      error("PACKAGE_ARCHITECTURE_METADATA", `architectureDiagram.path must be ${architecturePath(manifest)}`);
    }
    if (!Number.isInteger(architecture.bytes) || architecture.bytes <= 0 || architecture.bytes > (manifest.limits?.maxArchitectureBytes || 1024 * 1024)) {
      error("PACKAGE_ARCHITECTURE_METADATA", "architectureDiagram.bytes is invalid");
    }
    if (typeof architecture.sha256 !== "string" || !/^[0-9a-f]{64}$/.test(architecture.sha256)) {
      error("PACKAGE_ARCHITECTURE_METADATA", "architectureDiagram.sha256 must be a lowercase SHA-256 hex digest");
    }
    if (localArchitecture) {
      if (architecture.bytes !== localArchitecture.bytes) error("PACKAGE_ARCHITECTURE_METADATA", "architectureDiagram.bytes does not match local architecture.mmd");
      if (architecture.sha256 !== localArchitecture.sha256) error("PACKAGE_ARCHITECTURE_METADATA", "architectureDiagram.sha256 does not match local architecture.mmd");
    }
  }

  if (!logs.some((entry) => entry.level === "error")) {
    info("PACKAGE_OK", "submission metadata matches MatrixMul package contract");
    info("METRICS_OK", `score=${score}`);
    info("FUNCTIONAL_OK", `${REQUIRED_SHOTS} ${REQUIRED_GATE} shots recorded by trusted evaluator`);
    info("ARCHITECTURE_METADATA_OK", `${architecture.path} sha256=${architecture.sha256} bytes=${architecture.bytes}`);
  }
  return { ok: !logs.some((entry) => entry.level === "error"), logs, score, trackId: manifest.name };
}

function printValidation(result) {
  console.log(`track: ${result.trackId || "unknown"}`);
  for (const entry of result.logs) console.log(`${entry.level.toUpperCase()} ${entry.code}: ${entry.message}`);
  if (result.ok) console.log(`score: ${result.score}`);
  console.log(`status: ${result.ok ? "ranked" : "failed"}`);
}

function configPath() {
  if (process.env.MATRIXMUL_CONFIG) return process.env.MATRIXMUL_CONFIG;
  if (process.env.BEML_CONFIG) return process.env.BEML_CONFIG;
  const base = process.env.APPDATA || path.join(os.homedir(), ".config");
  return path.join(base, "matrixmul", "config.json");
}

function readConfig() {
  try {
    return JSON.parse(fs.readFileSync(configPath(), "utf8"));
  } catch {
    return {};
  }
}

function writeConfig(config) {
  const target = configPath();
  fs.mkdirSync(path.dirname(target), { recursive: true });
  fs.writeFileSync(target, `${JSON.stringify(config, null, 2)}\n`, { mode: 0o600 });
}

function apiUrl(args = []) {
  return (getFlag(args, "--api") || process.env.MATRIXMUL_API_URL || process.env.BEML_API_URL || readConfig().api_url || DEFAULT_API).replace(/\/$/, "");
}

function apiToken() {
  return process.env.MATRIXMUL_API_TOKEN || process.env.MATRIXMUL_API_KEY || process.env.BEML_API_TOKEN || process.env.BEML_API_KEY || readConfig().api_token || "";
}

function authHeaders() {
  const token = apiToken();
  return token ? { authorization: `Bearer ${token}` } : {};
}

function nodeFetch(url, options = {}, redirects = 0) {
  return new Promise((resolve, reject) => {
    const target = new URL(url);
    const client = target.protocol === "http:" ? http : https;
    const request = client.request(target, { method: options.method || "GET", headers: options.headers || {} }, (response) => {
      const chunks = [];
      response.on("data", (chunk) => chunks.push(chunk));
      response.on("end", async () => {
        const body = Buffer.concat(chunks).toString("utf8");
        if (response.statusCode >= 300 && response.statusCode < 400 && response.headers.location) {
          if (redirects >= 5) return reject(new Error("too many redirects"));
          try {
            resolve(await nodeFetch(new URL(response.headers.location, target).toString(), options, redirects + 1));
          } catch (error) {
            reject(error);
          }
          return;
        }
        resolve({ ok: response.statusCode >= 200 && response.statusCode < 300, status: response.statusCode, text: async () => body });
      });
    });
    request.on("error", reject);
    if (options.body) request.write(options.body);
    request.end();
  });
}

async function requestJson(url, options = {}) {
  const response = await nodeFetch(url, { ...options, headers: { "content-type": "application/json", ...(options.headers || {}) } });
  const text = await response.text();
  const json = text ? JSON.parse(text) : null;
  if (!response.ok) throw new Error(json?.error || `HTTP ${response.status}`);
  return json;
}

async function assertScoreImprovesLeaderboard(trackId, localScore, args = []) {
  const response = await requestJson(`${apiUrl(args)}/api/leaderboard?track_id=${encodeURIComponent(trackId)}`);
  const rows = Array.isArray(response.rows) ? response.rows : [];
  const best = rows
    .filter((row) => Number.isFinite(Number(row.score)))
    .sort((left, right) => Number(left.score) - Number(right.score))[0];
  if (!best) {
    console.log("score_gate: no ranked submissions yet");
    return;
  }
  const bestScore = Number(best.score);
  if (localScore >= bestScore) {
    const id = best.submission_id || best.id || "unknown";
    throw new Error(`local score ${localScore} is not better than current best ${bestScore} for ${trackId} (${id})`);
  }
  console.log(`score_gate: local score ${localScore} beats current best ${bestScore}`);
}

async function login(token, args) {
  if (!token) usage(1, "login");
  const targetApi = apiUrl(args);
  const response = await requestJson(`${targetApi}/api/me`, { headers: { authorization: `Bearer ${token}` } });
  writeConfig({ ...readConfig(), api_url: targetApi, api_token: token });
  console.log(`logged in: @${response.user.github_login}`);
  console.log(`api: ${targetApi}`);
}

function showConfig(args) {
  const token = apiToken();
  console.log(`api: ${apiUrl(args)}`);
  console.log(`token: ${token ? `${token.slice(0, 12)}...${token.slice(-6)}` : "(none)"}`);
  console.log(`config: ${configPath()}`);
}

async function submit(filePath, args) {
  filePath = filePath || defaultSubmissionPath();
  const metadata = readJson(filePath);
  const result = validatePackage(metadata, { manifest: repoManifest(getFlag(args, "--manifest", "benchmark.json")), metadataPath: filePath });
  printValidation(result);
  if (!result.ok) process.exit(1);
  const notePath = path.resolve(path.dirname(path.resolve(filePath)), metadata.note);
  const archivePath = getFlag(args, "--archive")
    ? path.resolve(getFlag(args, "--archive"))
    : path.resolve(path.dirname(path.resolve(filePath)), metadata.archive);
  const archiveBytes = fs.statSync(archivePath).size;
  const archiveSha256 = sha256File(archivePath);
  if (metadata.archiveBytes !== archiveBytes) {
    throw new Error(`archive size mismatch: metadata=${metadata.archiveBytes}, upload=${archiveBytes}`);
  }
  if (metadata.archiveSha256?.toLowerCase() !== archiveSha256) {
    throw new Error(`archive hash mismatch: metadata=${metadata.archiveSha256}, upload=${archiveSha256}`);
  }
  await assertScoreImprovesLeaderboard(result.trackId, result.score, args);
  const payload = {
    track_id: result.trackId,
    metadata,
    model: getFlag(args, "--model", metadata.model || ""),
    note: fs.existsSync(notePath) ? fs.readFileSync(notePath, "utf8") : "",
    source_url: getFlag(args, "--source-url", ""),
    archive_sha256: archiveSha256,
    archive_size_bytes: archiveBytes,
    archive_base64: fs.readFileSync(archivePath).toString("base64")
  };
  const response = await requestJson(`${apiUrl(args)}/api/submissions`, {
    method: "POST",
    headers: authHeaders(),
    body: JSON.stringify(payload)
  });
  console.log(`submission_id: ${response.submission_id}`);
  console.log(`server_status: ${response.status}`);
  console.log(`rank_status: ${response.rank_status}`);
  if (hasFlag(args, "--watch")) await pollSubmissionStatus(response.submission_id, args);
}

async function fetchSubmissionStatus(id, args = []) {
  return requestJson(`${apiUrl(args)}/api/submissions/${encodeURIComponent(id)}`, { headers: authHeaders() });
}

function isTerminalSubmission(response) {
  return response.status === "ranked" || response.status === "failed";
}

function sleep(ms) {
  return new Promise((resolve) => setTimeout(resolve, ms));
}

function printSubmissionStatus(response) {
  console.log(`submission_id: ${response.submission_id || response.id}`);
  console.log(`track: ${response.track_id}`);
  console.log(`server_status: ${response.status}`);
  console.log(`rank_status: ${response.rank_status}`);
  if (response.metrics?.score !== undefined) console.log(`score: ${response.metrics.score}`);
  if (response.failure_code) console.log(`failure_code: ${response.failure_code}`);
  if (response.accepted_by_github_login) console.log(`accepted_by: @${response.accepted_by_github_login}`);
  if (response.trusted_worker_passed_at) console.log(`trusted_worker_passed_at: ${response.trusted_worker_passed_at}`);
  if (response.merge_url) console.log(`merge_url: ${response.merge_url}`);
  if (response.merge_commit_sha) console.log(`merge_commit_sha: ${response.merge_commit_sha}`);
}

async function pollSubmissionStatus(id, args = []) {
  const intervalSeconds = numberFlag(args, "--poll-interval", 10);
  const timeoutSeconds = numberFlag(args, "--timeout", 0);
  const started = Date.now();
  let lastKey = "";
  while (true) {
    const response = await fetchSubmissionStatus(id, args);
    const key = `${response.status}:${response.rank_status}:${response.merge_commit_sha || ""}:${response.failure_code || ""}`;
    if (key !== lastKey) {
      printSubmissionStatus(response);
      lastKey = key;
    } else {
      console.log(`waiting: status=${response.status} rank_status=${response.rank_status}`);
    }
    if (isTerminalSubmission(response)) return response;
    if (timeoutSeconds > 0 && Date.now() - started >= timeoutSeconds * 1000) {
      throw new Error(`timed out waiting for ${id}`);
    }
    await sleep(intervalSeconds * 1000);
  }
}

async function status(id, args) {
  if (!id) usage(1, "status");
  if (hasFlag(args, "--watch")) {
    await pollSubmissionStatus(id, args);
    return;
  }
  printSubmissionStatus(await fetchSubmissionStatus(id, args));
}

async function logs(id, args) {
  if (!id) usage(1, "logs");
  const response = await requestJson(`${apiUrl(args)}/api/submissions/${encodeURIComponent(id)}/logs`, { headers: authHeaders() });
  for (const entry of response.logs) console.log(`${entry.level.toUpperCase()} ${entry.code}: ${entry.message}`);
}

async function leaderboard(args) {
  const track = getFlag(args, "--track", repoManifest().name);
  const response = await requestJson(`${apiUrl(args)}/api/leaderboard?track_id=${encodeURIComponent(track)}`);
  if (!response.rows.length) {
    console.log("No accepted submissions yet.");
    return;
  }
  response.rows.forEach((row, index) => {
    const author = row.author_github_login ? `@${row.author_github_login}` : row.author_display_name;
    console.log(`${index + 1}. ${row.submission_name} ${row.score} ${row.submission_id || row.id} ${author}`);
  });
}

async function main() {
  const [command, first, ...rest] = process.argv.slice(2);
  if (!command || command === "--help" || command === "-h") usage(0);
  const args = [first, ...rest].filter(Boolean);
  if (first === "--help" || first === "-h") usage(0, command);

  if (command === "repo") {
    console.log(__dirname);
    return;
  }
  if (command === "setup") return runManifestCommand("setupCommand");
  if (command === "preflight") return preflight(args);
  if (command === "run") return runTrusted(args);
  if (command === "package") return packageSubmission(args);
  if (command === "validate") {
    const filePath = firstPositional(args) || defaultSubmissionPath();
    const result = validatePackage(readJson(filePath), { manifest: repoManifest(getFlag(args, "--manifest", "benchmark.json")), metadataPath: filePath });
    printValidation(result);
    process.exit(result.ok ? 0 : 1);
  }
  if (command === "submit") return submit(firstPositional(args), args);
  if (command === "login") return login(first, rest);
  if (command === "config") return showConfig(args);
  if (command === "status") return status(first, rest);
  if (command === "logs") return logs(first, rest);
  if (command === "leaderboard") return leaderboard(args);
  usage(1);
}

main().catch((error) => {
  console.error(`error: ${error.message}`);
  process.exit(1);
});
