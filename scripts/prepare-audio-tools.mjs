import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, { stdio: "inherit", ...options });
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status}`);
}

if (process.platform !== "darwin" || process.arch !== "arm64") {
  throw new Error("The SonArcan FFmpeg runtime requires macOS on Apple Silicon");
}
run("bash", [join(root, "scripts/build-ffmpeg-runtime.sh")]);
