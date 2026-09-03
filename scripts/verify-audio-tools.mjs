import { existsSync, readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const directory = join(root, "src-tauri/resources/audio-tools");
const manifestPath = join(directory, "manifest.json");
if (!existsSync(manifestPath)) throw new Error("audio-tools manifest is missing");
const manifest = JSON.parse(readFileSync(manifestPath, "utf8"));
const expectedArchitecture = process.platform === "darwin" && process.arch === "x64" ? "x86_64" : process.arch;
if (manifest.architecture !== expectedArchitecture) throw new Error("audio-tools architecture does not match this build host");
if (manifest.platform && manifest.platform !== process.platform) throw new Error("audio-tools platform does not match this build host");
const suffix = process.platform === "win32" ? ".exe" : "";
for (const name of ["ffmpeg", "ffprobe"]) {
  const executable = join(directory, "bin", `${name}${suffix}`);
  const result = spawnSync(executable, ["-hide_banner", "-version"], { stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${name} is missing or cannot execute`);
}
