import { existsSync, readdirSync } from "node:fs";
import { join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const root = resolve("src-tauri/resources");
const runtime = join(root, "light-python-runtime/runtime");
const python = process.platform === "win32"
  ? join(runtime, "python.exe")
  : join(runtime, "bin/python3.13");
const sitePackages = process.platform === "win32"
  ? join(runtime, "Lib/site-packages")
  : join(runtime, "lib/python3.13/site-packages");
const ytdlp = join(root, "ytdlp-search/yt-dlp");

for (const [path, label] of [[python, "Light Python"], [sitePackages, "Light site-packages"], [ytdlp, "yt-dlp"]]) {
  if (!existsSync(path)) throw new Error(`${label} is missing: ${path}`);
}
const forbidden = ["torch", "mlx", "lv_chordia", "beat_this", "demucs_mlx", "madmom", "scipy", "numpy"];
const packaged = new Set(readdirSync(sitePackages).map((entry) => entry.toLowerCase()));
for (const module of forbidden) {
  if ([...packaged].some((entry) => entry === module || entry.startsWith(`${module}-`))) {
    throw new Error(`heavy analysis package leaked into Light: ${module}`);
  }
}
const result = spawnSync(python, [ytdlp, "--version"], { encoding: "utf8" });
if (result.status !== 0) throw new Error(`Light yt-dlp verification failed:\n${result.stderr || result.stdout}`);
console.log(`Verified SonArcan Light runtime with yt-dlp ${result.stdout.trim()}`);
