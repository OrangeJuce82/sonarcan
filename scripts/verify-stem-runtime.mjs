import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const appleSilicon = process.env.SONARCAN_STEM_BACKEND !== "torch"
  && process.platform === "darwin" && process.arch === "arm64";
const python = join(root, "src-tauri/resources/python-runtime/runtime/bin/python3.13");
if (!existsSync(python)) throw new Error("pinned shared Python 3.13 runtime is missing");
const commandArguments = appleSilicon
  ? ["-m", "sonarcan_mlx_worker", "self-test"]
  : ["-m", "sonarcan_torch_worker.worker", "self-test"];
const result = spawnSync(python, commandArguments, { cwd: root, stdio: "inherit" });
if (result.status !== 0) throw new Error(`${appleSilicon ? "MLX" : "Torch"} self-test failed`);
