import { readdir, rm } from "node:fs/promises";
import { existsSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const argumentsSet = new Set(process.argv.slice(2));
const supportedArguments = new Set(["--dependencies", "--dry-run"]);
const unsupported = [...argumentsSet].filter((argument) => !supportedArguments.has(argument));

if (unsupported.length > 0) {
  throw new Error(`Unsupported clean option: ${unsupported.join(", ")}`);
}

const includeDependencies = argumentsSet.has("--dependencies");
const dryRun = argumentsSet.has("--dry-run");
const removalOptions = {
  recursive: true,
  force: true,
  maxRetries: 5,
  retryDelay: 100,
};

const generatedArtifacts = [
  "dist",
  "target",
  "src-tauri/target",
  "release-artifacts",
  "coverage",
  ".svelte-kit",
  "tools/sonarcan-chord-worker/build",
  "tools/sonarcan-torch-worker/build",
  "tools/sonarcan-mlx-worker/build",
];

const localDependencies = [
  "node_modules",
  "tools/sonarcan-mlx-worker/.venv",
  "tools/sonarcan-chord-worker/.venv",
  "tools/sonarcan-torch-worker/.venv",
  "src-tauri/resources/chord-runtime/runtime",
  "src-tauri/resources/mlx-runtime/runtime",
  "src-tauri/resources/stem-runtime/runtime",
  "src-tauri/resources/audio-tools/bin",
  "src-tauri/resources/audio-tools/licenses",
  "src-tauri/resources/audio-tools/manifest.json",
  "src-tauri/resources/ytdlp-search/yt-dlp",
  "src-tauri/resources/models/demucs-mlx/htdemucs_6s.safetensors",
  "src-tauri/resources/models/demucs-mlx/htdemucs_6s_config.json",
];

function safePath(relativePath) {
  const resolved = path.resolve(repositoryRoot, relativePath);
  if (resolved === repositoryRoot || !resolved.startsWith(`${repositoryRoot}${path.sep}`)) {
    throw new Error(`Refusing to clean outside the repository: ${relativePath}`);
  }
  return resolved;
}

async function pythonCaches(directory) {
  const root = safePath(directory);
  if (!existsSync(root)) return [];
  const results = [];
  const entries = await readdir(root, { withFileTypes: true });
  for (const entry of entries) {
    if (entry.isSymbolicLink()) continue;
    const relative = path.join(directory, entry.name);
    if (entry.isDirectory() && (["__pycache__", ".pytest_cache", ".mypy_cache", ".ruff_cache"].includes(entry.name) || entry.name.endsWith(".egg-info"))) {
      results.push(relative);
    } else if (entry.isDirectory() && entry.name !== ".venv") {
      results.push(...await pythonCaches(relative));
    }
  }
  return results;
}

const targets = [
  ...generatedArtifacts,
  ...await pythonCaches("tools/sonarcan-mlx-worker"),
  ...await pythonCaches("tools/sonarcan-chord-worker"),
  ...await pythonCaches("tools/sonarcan-torch-worker"),
  ...(includeDependencies ? localDependencies : []),
];

let removed = 0;
for (const relativePath of [...new Set(targets)]) {
  const target = safePath(relativePath);
  if (!existsSync(target)) continue;
  if (dryRun) {
    console.log(`[dry-run] ${relativePath}`);
  } else {
    await rm(target, removalOptions);
    console.log(`removed ${relativePath}`);
  }
  removed += 1;
}

console.log(`${dryRun ? "would remove" : "removed"} ${removed} generated path${removed === 1 ? "" : "s"}`);
