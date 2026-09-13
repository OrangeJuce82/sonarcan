import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
if (process.platform !== "darwin" || process.arch !== "arm64") {
  throw new Error("Stem runtime verification requires macOS on Apple Silicon");
}
const python = join(root, "src-tauri/resources/python-runtime/runtime/bin/python3.13");
if (!existsSync(python)) throw new Error("pinned shared Python 3.13 runtime is missing");
const commandArguments = ["-m", "sonarcan_mlx_worker", "self-test"];
const result = spawnSync(python, commandArguments, { cwd: root, stdio: "inherit" });
if (result.status !== 0) throw new Error("MLX self-test failed");
