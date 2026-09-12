import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";

const models = new Set(["beat-this", "lv-chordia", "htdemucs", "scnet"]);
const targets = new Set(["macos-arm64", "linux-x86_64", "linux-arm64"]);
const backends = new Set(["mlx", "cuda"]);
const precisions = new Set(["fp32", "fp16"]);
const comparisonKinds = new Set(["audio", "timeline", "tensor"]);

/**
 * @typedef {object} QualificationReport
 * @property {number} schemaVersion
 * @property {string} model
 * @property {string} target
 * @property {string} backendUsed
 * @property {string} backendEvidence
 * @property {string} precision
 * @property {string} referenceRevision
 * @property {string} candidateRevision
 * @property {{kind: string, metric: string, value: number, threshold: number, passed: boolean}} resultComparison
 * @property {{inferenceTimeMs: number, realTimeFactor: number, peakRssBytes: number, loadTimeMs: number, runtimeBytes: number, programBytes: number}} measurements
 * @property {boolean} productionReady
 */

/** @param {unknown} value @param {string} name */
function requireText(value, name) {
  if (typeof value !== "string" || value.trim() === "") throw new Error(`${name} is required`);
}

/**
 * @param {unknown} value
 * @param {string} name
 * @param {{positive?: boolean}} [options]
 */
function requireFinite(value, name, { positive = false } = {}) {
  if (typeof value !== "number" || !Number.isFinite(value) || value < 0 || (positive && value === 0)) {
    throw new Error(`${name} must be a ${positive ? "positive" : "non-negative"} finite number`);
  }
}

/** @param {unknown} value @returns {QualificationReport} */
export function verifyQualification(value) {
  if (!value || typeof value !== "object") throw new Error("qualification report must be an object");
  const report = /** @type {Partial<QualificationReport>} */ (value);
  if (report?.schemaVersion !== 1) throw new Error("schemaVersion must be 1");
  if (!models.has(report.model ?? "")) throw new Error("unsupported model");
  if (!targets.has(report.target ?? "")) throw new Error("unsupported target");
  if (!backends.has(report.backendUsed ?? "")) throw new Error("unsupported backendUsed");
  if (!precisions.has(report.precision ?? "")) throw new Error("precision must be fp32 or fp16");
  requireText(report.backendEvidence, "backendEvidence");
  requireText(report.referenceRevision, "referenceRevision");
  requireText(report.candidateRevision, "candidateRevision");

  const comparison = report.resultComparison;
  if (!comparisonKinds.has(comparison?.kind ?? "")) throw new Error("invalid resultComparison.kind");
  if (!comparison) throw new Error("resultComparison is required");
  requireText(comparison.metric, "resultComparison.metric");
  requireFinite(comparison.value, "resultComparison.value");
  requireFinite(comparison.threshold, "resultComparison.threshold");
  if (comparison.value > comparison.threshold || comparison.passed !== true) {
    throw new Error("result parity did not pass its declared threshold");
  }

  const measurementNames = /** @type {const} */ ([
    "inferenceTimeMs",
    "realTimeFactor",
    "peakRssBytes",
    "loadTimeMs",
    "runtimeBytes",
    "programBytes",
  ]);
  for (const name of measurementNames) {
    requireFinite(report.measurements?.[name], `measurements.${name}`, { positive: true });
  }
  if (report.productionReady !== true) throw new Error("productionReady must be true");
  return /** @type {QualificationReport} */ (report);
}

function main() {
  const paths = process.argv.slice(2);
  if (paths.length === 0) throw new Error("usage: verify-executorch-qualification REPORT.json...");
  for (const path of paths) {
    const absolute = resolve(path);
    verifyQualification(JSON.parse(readFileSync(absolute, "utf8")));
    console.log(`ExecuTorch qualification accepted: ${absolute}`);
  }
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) main();
