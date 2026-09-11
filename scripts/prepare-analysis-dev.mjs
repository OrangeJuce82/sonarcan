import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const gpuBackend = process.env.SONARCAN_GPU_BACKEND;
const supportedAppleSilicon = process.platform === "darwin" && process.arch === "arm64";
const supportedDiscreteGpu = process.platform === "linux"
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

if (!supportedAppleSilicon && !supportedDiscreteGpu) {
  console.log(
    "Skipping analysis setup: this development build has no qualified GPU backend. "
      + "Use SONARCAN_GPU_BACKEND=nvidia on a supported Debian x64 host.",
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
