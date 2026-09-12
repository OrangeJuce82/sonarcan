import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const pythonSetting = process.env.SONARCAN_EXECUTORCH_PYTHON;

if (!pythonSetting) {
  throw new Error("Set SONARCAN_EXECUTORCH_PYTHON to the external model-export Python executable");
}
const python = resolve(pythonSetting);
if (!existsSync(python)) throw new Error(`Model-export Python does not exist: ${python}`);

const result = spawnSync(python, [join(root, "scripts/audit-executorch-export.py"), ...process.argv.slice(2)], {
  cwd: root,
  env: process.env,
  stdio: "inherit",
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
