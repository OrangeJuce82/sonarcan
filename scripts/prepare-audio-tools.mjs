import { createHash } from "node:crypto";
import { cpSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { audioToolsRelease } from "./audio-tools-release.mjs";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const output = join(root, "src-tauri/resources/audio-tools");
const { assetIds, assets, checksumsAssetId, checksumsSha256, tag: releaseTag } = audioToolsRelease;
const githubToken = process.env.GITHUB_TOKEN ?? process.env.GH_TOKEN;
const githubAssetHeaders = {
  Accept: "application/octet-stream",
  "User-Agent": "SonArcan-release-build",
  ...(githubToken ? { Authorization: `Bearer ${githubToken}` } : {}),
};

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, { stdio: "inherit", ...options });
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status}`);
}

function sha256(path) {
  return createHash("sha256").update(readFileSync(path)).digest("hex");
}

async function download(url, destination, headers = {}) {
  const response = await fetch(url, { headers, redirect: "follow" });
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
const assetId = assetIds[platform];
if (!asset || !assetId) throw new Error(`no pinned audio-tools archive is defined for ${platform}`);
const temporary = mkdtempSync(join(tmpdir(), "sonarcan-audio-tools-"));
try {
  const checksums = join(temporary, "checksums.sha256");
  await download(
    `https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/assets/${checksumsAssetId}`,
    checksums,
    githubAssetHeaders,
  );
  if (sha256(checksums) !== checksumsSha256) throw new Error("FFmpeg checksum manifest is invalid");
  const line = readFileSync(checksums, "utf8").split(/\r?\n/).find((value) => value.endsWith(`  ${asset}`));
  if (!line) throw new Error(`FFmpeg checksum is missing for ${asset}`);
  const expectedArchiveHash = line.split(/\s+/)[0];
  const archive = join(temporary, asset);
  await download(
    `https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/assets/${assetId}`,
    archive,
    githubAssetHeaders,
  );
  if (sha256(archive) !== expectedArchiveHash) throw new Error("FFmpeg archive checksum is invalid");
  const extracted = join(temporary, "extracted");
  mkdirSync(extracted);
  run("tar", ["-xf", archive, "-C", extracted]);
  const suffix = "";
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
