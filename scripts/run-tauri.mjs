import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const arguments_ = process.argv.slice(2);

function run(script, scriptArguments) {
  const result = spawnSync(process.execPath, [script, ...scriptArguments], {
    cwd: repositoryRoot,
    env: process.env,
    stdio: "inherit",
  });
  if (result.error) throw result.error;
  if (result.signal) process.kill(process.pid, result.signal);
  process.exitCode = result.status ?? 1;
  return result.status === 0;
}

run(join(repositoryRoot, "node_modules/@tauri-apps/cli/tauri.js"), arguments_);
