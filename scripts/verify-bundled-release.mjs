import { existsSync, readdirSync, statSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

let root = process.cwd();

/** @param {string} directory @param {number} depth @returns {string | undefined} */
function findResourceRoot(directory, depth = 0) {
  if (depth > 10) return undefined;
  if (existsSync(join(directory, "executorch-runtime")) && existsSync(join(directory, "audio-tools"))) return directory;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.isSymbolicLink()) continue;
    const found = findResourceRoot(join(directory, entry.name), depth + 1);
    if (found) return found;
  }
  return undefined;
}

/** @param {string} path @param {string} label @returns {string} */
function required(path, label) {
  if (!existsSync(path)) throw new Error(`${label} is missing from the bundle: ${path}`);
  return path;
}

/** @param {string} directory @param {number} depth @returns {string | undefined} */
function findForbiddenModelCheckpoint(directory, depth = 0) {
  if (depth > 12) return undefined;
  const forbidden = new Set([
    "955717e8-8726e21a.th",
    "htdemucs.safetensors",
    "htdemucs_6s.safetensors",
    "SCNet-large_starrytong_fixed.ckpt",
    "final0.ckpt",
  ]);
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (entry.isSymbolicLink()) continue;
    const path = join(directory, entry.name);
    if (entry.isFile() && forbidden.has(entry.name)) return path;
    if (entry.isDirectory()) {
      const found = findForbiddenModelCheckpoint(path, depth + 1);
      if (found) return found;
    }
  }
  return undefined;
}

/**
 * @param {string} command
 * @param {string[]} argumentsList
 * @param {string} label
 * @param {boolean} capture
 * @returns {string}
 */
function run(command, argumentsList, label, capture = false) {
  const result = spawnSync(command, argumentsList, {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
    stdio: capture ? "pipe" : "inherit",
  });
  if (result.status !== 0) {
    const detail = capture ? `\n${result.stderr || result.stdout}` : "";
    throw new Error(`${label} failed with status ${result.status}${detail}`);
  }
  return capture ? result.stdout.trim() : "";
}

/** @param {string} output @returns {string | undefined} */
export function forbiddenInferenceDependency(output) {
  return output
    .split(/\r?\n/u)
    .map((line) => line.trim())
    .find((line) => /(?:libpython|libtorch|site-packages)/iu.test(line));
}

/** @param {string} output @param {NodeJS.Platform} platform */
export function validateProductionBackends(output, platform) {
  let backends;
  try {
    backends = JSON.parse(output);
  } catch {
    throw new Error("bundled ExecuTorch worker returned an invalid backend contract");
  }
  const expected = platform === "darwin" ? "MLXBackend" : "CudaBackend";
  const forbidden = platform === "darwin" ? "CudaBackend" : "MLXBackend";
  if (backends[expected] !== true || backends[forbidden] === true || backends.XnnpackBackend === true) {
    throw new Error(`bundled ExecuTorch worker does not expose the required GPU-only ${expected} contract`);
  }
}

/** @param {string} worker */
function verifyNativeInferenceDependencies(worker) {
  const command = process.platform === "darwin" ? "otool" : "ldd";
  const argumentsList = process.platform === "darwin" ? ["-L", worker] : [worker];
  const dependencies = run(command, argumentsList, "native dependency inspection", true);
  const forbidden = forbiddenInferenceDependency(dependencies);
  if (forbidden) {
    throw new Error(`bundled ExecuTorch worker links a forbidden Python/PyTorch runtime: ${forbidden}`);
  }
}

function main() {
  const bundleRoot = process.argv[2];
  if (!bundleRoot) {
    throw new Error("usage: node scripts/verify-bundled-release.mjs <bundle-or-install-root>");
  }
  root = resolve(bundleRoot);
  if (!existsSync(root) || !statSync(root).isDirectory()) {
    throw new Error(`bundle root is not a directory: ${root}`);
  }

  const resources = findResourceRoot(root);
  if (!resources) throw new Error(`could not locate SonArcan resources inside ${root}`);
  const forbiddenModelCheckpoint = findForbiddenModelCheckpoint(resources);
  if (forbiddenModelCheckpoint) {
    throw new Error(`first-run model checkpoints must not be bundled: ${forbiddenModelCheckpoint}`);
  }

  const suffix = "";
  const executorchWorker = required(
    join(resources, "executorch-runtime", `sonarcan-executorch-worker${suffix}`),
    "bundled ExecuTorch worker",
  );
  const executorchVersion = run(executorchWorker, ["--version"], "bundled ExecuTorch worker", true);
  if (executorchVersion !== "sonarcan-executorch-worker 1.4.1 SACTEN01") {
    throw new Error("bundled ExecuTorch worker returned an invalid version contract");
  }
  validateProductionBackends(
    run(executorchWorker, ["--backends"], "bundled ExecuTorch backend contract", true),
    process.platform,
  );
  verifyNativeInferenceDependencies(executorchWorker);
  const ffmpeg = required(join(resources, "audio-tools", "bin", `ffmpeg${suffix}`), "bundled FFmpeg");
  const ffprobe = required(join(resources, "audio-tools", "bin", `ffprobe${suffix}`), "bundled FFprobe");
  run(ffmpeg, ["-hide_banner", "-version"], "bundled FFmpeg");
  run(ffprobe, ["-hide_banner", "-version"], "bundled FFprobe");

  const ytdlp = required(join(resources, "ytdlp-search", `yt-dlp${suffix}`), "bundled standalone yt-dlp");
  run(ytdlp, ["--version"], "bundled standalone yt-dlp");

  console.log(JSON.stringify({
    verifiedBundle: root,
    resources,
    platform: process.platform,
    architecture: process.arch,
    inferenceRuntime: "ExecuTorch",
    legacyRuntimePresent: false,
  }));
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) main();
