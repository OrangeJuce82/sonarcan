import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const appleSilicon = process.env.SONARCAN_STEM_BACKEND !== "torch"
  && process.platform === "darwin" && process.arch === "arm64";
const runtime = appleSilicon ? "mlx-runtime" : "stem-runtime";
const python = process.platform === "win32"
  ? join(root, `src-tauri/resources/${runtime}/runtime/python.exe`)
  : join(root, `src-tauri/resources/${runtime}/runtime/bin/${appleSilicon ? "python3.13" : "python3.12"}`);
const model = join(root, "src-tauri/resources/models/demucs-mlx");
if (!existsSync(python)) throw new Error(`pinned ${runtime} Python is missing`);
const commandArguments = appleSilicon
  ? ["-m", "sonarcan_mlx_worker", "self-test", "--model-dir", model]
  : ["-m", "sonarcan_torch_worker.worker", "self-test", "--model-dir", model];
const result = spawnSync(python, commandArguments, { cwd: root, stdio: "inherit" });
if (result.status !== 0) throw new Error(`${runtime} self-test failed`);
