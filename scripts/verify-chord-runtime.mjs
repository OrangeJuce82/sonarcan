import { existsSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = process.platform === "win32"
  ? join(root, "src-tauri/resources/python-runtime/runtime/python.exe")
  : join(root, "src-tauri/resources/python-runtime/runtime/bin/python3.13");
const model = join(root, "src-tauri/resources/models/beat-this/final0.ckpt");

if (!existsSync(runtime)) {
  throw new Error("Missing shared Python runtime. Run: npm run python:runtime");
}

const result = spawnSync(runtime, [
  "-m", "sonarcan_chord_worker.worker", "--self-test", "--downbeat-model", model,
], { encoding: "utf8" });
if (result.status !== 0) {
  throw new Error(`LV-Chordia runtime self-test failed:\n${result.stderr || result.stdout}`);
}

const health = JSON.parse(result.stdout.trim());
const expectedModes = ["complete", "essential", "standard"];
if (
  !health.ok
  || JSON.stringify(health.modes) !== JSON.stringify(expectedModes)
  || !String(health.downbeatModelVersion ?? "").startsWith("beat-this@")
) {
  throw new Error("The bundled chord/downbeat worker is stale or has an invalid contract. Run: npm run python:runtime");
}
console.log(JSON.stringify(health));
