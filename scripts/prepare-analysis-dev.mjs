import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const edition = process.env.SONARCAN_EDITION ?? "full";
const gpuBackend = process.env.SONARCAN_GPU_BACKEND;
const supportedAppleSilicon = process.platform === "darwin" && process.arch === "arm64";
const supportedDiscreteGpu = ["win32", "linux"].includes(process.platform)
  && process.arch === "x64"
  && ["nvidia", "amd"].includes(gpuBackend ?? "");

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

if (edition === "light") {
  console.log("Skipping analysis setup for the Light edition.");
  process.exit(0);
}

if (!supportedAppleSilicon && !supportedDiscreteGpu) {
  console.log(
    "Skipping analysis setup: this development build has no qualified GPU backend. "
      + "Use SONARCAN_GPU_BACKEND=nvidia or amd on a supported Windows/Linux x64 host.",
  );
  process.exit(0);
}

run("uv", ["sync", "--project", "tools/sonarcan-chord-worker", "--locked"]);
run("uv", [
  "sync",
  "--project",
  supportedAppleSilicon ? "tools/sonarcan-mlx-worker" : "tools/sonarcan-torch-worker",
  "--locked",
  "--reinstall-package",
  "sonarcan-scnet-infer",
]);
run(process.execPath, ["scripts/prepare-beat-this-model.mjs"]);

console.log("Development analysis runtime is ready.");
