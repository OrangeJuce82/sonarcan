import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const backend = process.env.SONARCAN_GPU_BACKEND;
if (!["nvidia", "amd"].includes(backend)) {
  throw new Error("SONARCAN_GPU_BACKEND must be nvidia or amd");
}

const root = resolve("src-tauri/resources/python-runtime/runtime");
const python = process.platform === "win32" ? join(root, "python.exe") : join(root, "bin/python3.13");
if (!existsSync(python)) throw new Error(`shared Python runtime is missing: ${python}`);

const expression = backend === "nvidia"
  ? "assert torch.version.cuda and not torch.version.hip; print(torch.__version__, torch.version.cuda)"
  : "assert torch.version.hip; print(torch.__version__, torch.version.hip)";
const result = spawnSync(python, ["-c", `import torch; ${expression}`], { encoding: "utf8", stdio: "pipe" });
if (result.status !== 0) {
  throw new Error(`${backend} PyTorch runtime qualification failed:\n${result.stderr || result.stdout}`);
}
console.log(`${backend} GPU runtime qualified: ${result.stdout.trim()}`);
