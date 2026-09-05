import { cpSync, existsSync, mkdirSync, readdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtime = join(repositoryRoot, "src-tauri/resources/light-python-runtime/runtime");
const ytdlp = join(repositoryRoot, "src-tauri/resources/ytdlp-search/yt-dlp");

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
  });
  if (result.status !== 0) {
    const detail = options.capture ? `\n${result.stderr || result.stdout}` : "";
    throw new Error(`${command} failed with status ${result.status}${detail}`);
  }
  return options.capture ? result.stdout.trim() : "";
}

if (!existsSync(ytdlp)) {
  throw new Error("yt-dlp search artifact is missing; run npm run ytdlp:search");
}

const managedPython = run("uv", ["python", "find", "--managed-python", "3.13.5"], { capture: true });
const managedRoot = process.platform === "win32" ? dirname(managedPython) : dirname(dirname(managedPython));
rmSync(runtime, { recursive: true, force: true });
mkdirSync(dirname(runtime), { recursive: true });
cpSync(managedRoot, runtime, { recursive: true, dereference: false });

const sitePackages = process.platform === "win32"
  ? join(runtime, "Lib/site-packages")
  : join(runtime, "lib/python3.13/site-packages");
rmSync(sitePackages, { recursive: true, force: true });
mkdirSync(sitePackages, { recursive: true });

const binDirectory = process.platform === "win32" ? runtime : join(runtime, "bin");
for (const entry of readdirSync(binDirectory)) {
  if (/^(pip|idle|pydoc|2to3)/i.test(entry)) {
    rmSync(join(binDirectory, entry), { force: true });
  }
}

const runtimePython = process.platform === "win32"
  ? join(runtime, "python.exe")
  : join(runtime, "bin/python3.13");
run(runtimePython, ["-c", "import json, ssl, urllib.request"]);
run(runtimePython, [ytdlp, "--version"]);
console.log(`Minimal SonArcan Light Python runtime assembled in ${runtime}`);
