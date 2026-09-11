import { existsSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = join(root, "src-tauri/resources/python-runtime/runtime");
const python = process.platform === "win32"
  ? join(runtime, "python.exe")
  : join(runtime, "bin/python3.13");

if (!existsSync(python)) {
  throw new Error("Missing development Python runtime. Run: npm run python:runtime");
}

const result = spawnSync(python, [join(root, "scripts/audit-executorch-export.py"), ...process.argv.slice(2)], {
  cwd: root,
  env: process.env,
  stdio: "inherit",
});
if (result.error) throw result.error;
process.exit(result.status ?? 1);
