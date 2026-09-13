import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const directory = join(root, "src-tauri/resources/audio-tools");
const manifestPath = join(directory, "manifest.json");
if (!existsSync(manifestPath)) throw new Error("audio-tools manifest is missing");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
if (process.platform !== "darwin" || process.arch !== "arm64") {
  throw new Error("audio-tools verification requires macOS Apple Silicon");
}
if (manifest.architecture !== "arm64" || manifest.platform !== "darwin") {
  throw new Error("audio-tools does not target macOS Apple Silicon");
}
if (manifest.minimumMacosVersion !== "14.0") {
  throw new Error(`audio-tools requires macOS ${manifest.minimumMacosVersion ?? "unknown"}; expected 14.0`);
}
const suffix = "";
for (const name of ["ffmpeg", "ffprobe"]) {
  const executable = join(directory, "bin", `${name}${suffix}`);
  const result = spawnSync(executable, ["-hide_banner", "-version"], { stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${name} is missing or cannot execute`);
}
