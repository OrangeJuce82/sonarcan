import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

if (process.env.SONARCAN_EDITION === "light") {
  console.log("Skipping chord-worker tests: the Light edition excludes chords and Torch.");
  process.exit(0);
}

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const result = spawnSync("uv", [
  "run",
  "--project",
  "tools/sonarcan-chord-worker",
  "--locked",
  "python",
  "-m",
  "unittest",
  "discover",
  "-s",
  "tools/sonarcan-chord-worker/tests",
  "-v",
], {
  cwd: repositoryRoot,
  stdio: "inherit",
});

if (result.error) {
  throw result.error;
}
process.exit(result.status ?? 1);
