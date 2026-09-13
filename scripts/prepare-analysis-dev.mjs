import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const supportedAppleSilicon = process.platform === "darwin" && process.arch === "arm64";

function run(command, arguments_) {
  const result = spawnSync(command, arguments_, {
    cwd: repositoryRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) process.exit(result.status ?? 1);
}

if (!supportedAppleSilicon) {
  throw new Error("SonArcan analysis development requires macOS on Apple Silicon");
}

run("uv", ["sync", "--project", "tools/sonarcan-chord-worker", "--locked"]);
run("uv", [
  "sync",
  "--project",
  "tools/sonarcan-mlx-worker",
  "--locked",
  "--reinstall-package",
  "sonarcan-scnet-infer",
]);
run(process.execPath, ["scripts/prepare-beat-this-model.mjs"]);

console.log("Development analysis runtime is ready.");
