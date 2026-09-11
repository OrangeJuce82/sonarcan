import { existsSync } from "node:fs";
import { delimiter, dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const worker = process.argv[2];
const configurations = {
  mlx: { root: "tools/sonarcan-mlx-worker", sharedTests: true },
  torch: { root: "tools/sonarcan-torch-worker", sharedTests: false },
};
const configuration = configurations[worker];
if (!configuration) throw new Error("usage: node scripts/test-python-worker.mjs <mlx|torch>");
const workerRoot = join(root, configuration.root);
const virtualPython = join(workerRoot, ".venv/bin/python");
const runtimeExecutable = "bin/python3.13";
const runtimePython = [
  process.env.SONARCAN_PYTHON_RUNTIME_DIR,
  join(root, "src-tauri/resources/python-runtime/runtime"),
]
  .filter(Boolean)
  .map((directory) => join(directory, runtimeExecutable))
  .find((candidate) => existsSync(candidate));
const python = existsSync(virtualPython)
  ? virtualPython
  : runtimePython
    ? runtimePython
    : "python3.13";
const environment = {
  ...process.env,
  PYTHONPATH: [
    join(workerRoot, "src"),
    join(root, "tools/sonarcan-scnet-infer/src"),
    process.env.PYTHONPATH,
  ].filter(Boolean).join(delimiter),
};
const testDirectories = [join(workerRoot, "tests")];
if (configuration.sharedTests) testDirectories.push(join(root, "tools/sonarcan-scnet-infer/tests"));
for (const tests of testDirectories) {
  const result = spawnSync(
    python,
    ["-m", "unittest", "discover", "-s", tests, "-v"],
    { cwd: root, env: environment, stdio: "inherit" },
  );
  if (result.status !== 0) process.exit(result.status ?? 1);
}
