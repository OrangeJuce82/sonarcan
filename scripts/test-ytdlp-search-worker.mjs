import { spawnSync } from "node:child_process";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const result = spawnSync("python3", [
  "-m",
  "unittest",
  "discover",
  "-s",
  "tools/sonarcan-ytdlp-worker/tests",
  "-v",
], {
  cwd: repositoryRoot,
  stdio: "inherit",
});

if (result.error) throw result.error;
process.exit(result.status ?? 1);
