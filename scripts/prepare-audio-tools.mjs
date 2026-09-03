import { createHash } from "node:crypto";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = join(root, "src-tauri/resources/audio-tools");
const releaseTag = "autobuild-2026-08-29-13-12";
const checksumsSha256 = "f9bc6cb691090bdc377dbc0befd2658a2b21501bac21b0c1334328670c6f7957";
const assets = {
  "linux-x64": "ffmpeg-master-latest-linux64-lgpl.tar.xz",
  "win32-x64": "ffmpeg-master-latest-win64-lgpl.zip",
};

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, { stdio: "inherit", ...options });
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status}`);
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

async function download(url, destination) {
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) throw new Error(`download failed (${response.status}): ${url}`);
  writeFileSync(destination, Buffer.from(await response.arrayBuffer()));
}

function findFile(directory, fileName) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const path = join(directory, entry.name);
    if (entry.isDirectory()) {
      const nested = findFile(path, fileName);
      if (nested) return nested;
    } else if (entry.name.toLowerCase() === fileName.toLowerCase()) {
      return path;
    }
  }
  return undefined;
}

if (process.platform === "darwin") {
  run("bash", [join(root, "scripts/build-ffmpeg-runtime.sh")]);
  process.exit(0);
}

const platform = `${process.platform}-${process.arch}`;
const asset = assets[platform];
if (!asset) throw new Error(`no pinned audio-tools archive is defined for ${platform}`);
const temporary = mkdtempSync(join(tmpdir(), "sonarcan-audio-tools-"));
try {
  const base = `https://github.com/BtbN/FFmpeg-Builds/releases/download/${releaseTag}`;
  const checksums = join(temporary, "checksums.sha256");
  await download(`${base}/checksums.sha256`, checksums);
  if (sha256(checksums) !== checksumsSha256) throw new Error("FFmpeg checksum manifest is invalid");
  const line = readFileSync(checksums, "utf8").split(/\r?\n/).find((value) => value.endsWith(`  ${asset}`));
  if (!line) throw new Error(`FFmpeg checksum is missing for ${asset}`);
  const expectedArchiveHash = line.split(/\s+/)[0];
  const archive = join(temporary, asset);
  await download(`${base}/${asset}`, archive);
  if (sha256(archive) !== expectedArchiveHash) throw new Error("FFmpeg archive checksum is invalid");
  const extracted = join(temporary, "extracted");
  mkdirSync(extracted);
  run("tar", ["-xf", archive, "-C", extracted]);
  const suffix = process.platform === "win32" ? ".exe" : "";
  const ffmpeg = findFile(extracted, `ffmpeg${suffix}`);
  const ffprobe = findFile(extracted, `ffprobe${suffix}`);
  if (!ffmpeg || !ffprobe) throw new Error("FFmpeg archive is missing ffmpeg or ffprobe");
  rmSync(output, { recursive: true, force: true });
  mkdirSync(join(output, "bin"), { recursive: true });
  cpSync(ffmpeg, join(output, "bin", `ffmpeg${suffix}`));
  cpSync(ffprobe, join(output, "bin", `ffprobe${suffix}`));
  writeFileSync(join(output, "manifest.json"), `${JSON.stringify({
    architecture: process.arch,
    platform: process.platform,
    source: "BtbN/FFmpeg-Builds",
    releaseTag,
    asset,
    archiveSha256: expectedArchiveHash,
    checksumsSha256,
  }, null, 2)}\n`);
  run(join(output, "bin", `ffmpeg${suffix}`), ["-hide_banner", "-version"]);
  run(join(output, "bin", `ffprobe${suffix}`), ["-hide_banner", "-version"]);
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
