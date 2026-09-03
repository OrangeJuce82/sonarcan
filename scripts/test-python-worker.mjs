import { existsSync } from "node:fs";
import { delimiter, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const worker = process.argv[2];
const configurations = {
  mlx: { root: "tools/sonarcan-mlx-worker", module: "sonarcan_mlx_worker" },
  torch: { root: "tools/sonarcan-torch-worker", module: "sonarcan_torch_worker" },
};
const configuration = configurations[worker];
if (!configuration) throw new Error("usage: node scripts/test-python-worker.mjs <mlx|torch>");
const workerRoot = join(root, configuration.root);
const virtualPython = process.platform === "win32"
  ? join(workerRoot, ".venv/Scripts/python.exe")
  : join(workerRoot, ".venv/bin/python");
const python = existsSync(virtualPython) ? virtualPython : process.platform === "win32" ? "python" : "python3";
const environment = {
  ...process.env,
  PYTHONPATH: [join(workerRoot, "src"), process.env.PYTHONPATH].filter(Boolean).join(delimiter),
};
const result = spawnSync(
  python,
  ["-m", "unittest", "discover", "-s", join(workerRoot, "tests"), "-v"],
  { cwd: root, env: environment, stdio: "inherit" },
);
if (result.status !== 0) process.exit(result.status ?? 1);
