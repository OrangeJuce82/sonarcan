import { createHash } from "node:crypto";
import { audioToolsRelease } from "./audio-tools-release.mjs";

const base = `https://github.com/BtbN/FFmpeg-Builds/releases/download/${audioToolsRelease.tag}`;
const response = await fetch(`${base}/checksums.sha256`, { redirect: "follow" });
if (!response.ok) throw new Error(`FFmpeg checksum manifest is unavailable (${response.status})`);
const bytes = Buffer.from(await response.arrayBuffer());
const actualHash = createHash("sha256").update(bytes).digest("hex");
if (actualHash !== audioToolsRelease.checksumsSha256) {
  throw new Error("FFmpeg checksum manifest no longer matches the pinned SHA-256");
}

const manifest = bytes.toString("utf8");
const releaseResponse = await fetch(
  `https://api.github.com/repos/BtbN/FFmpeg-Builds/releases/tags/${audioToolsRelease.tag}`,
);
if (!releaseResponse.ok) throw new Error(`FFmpeg release metadata is unavailable (${releaseResponse.status})`);
const release = await releaseResponse.json();

for (const [platform, asset] of Object.entries(audioToolsRelease.assets)) {
  const checksumLine = manifest.split(/\r?\n/).find((line) => line.endsWith(`  ${asset}`));
  if (!checksumLine) {
    throw new Error(`FFmpeg checksum is missing for ${asset}`);
  }
  const expectedHash = checksumLine.split(/\s+/)[0];
  const assetId = audioToolsRelease.assetIds[platform];
  const metadata = release.assets.find((candidate) => candidate.id === assetId);
  if (
    !metadata
    || metadata.name !== asset
    || metadata.state !== "uploaded"
    || metadata.size <= 0
    || metadata.digest !== `sha256:${expectedHash}`
  ) {
    throw new Error(`FFmpeg release metadata is invalid for ${asset}`);
  }
  const assetResponse = await fetch(metadata.url, {
    headers: { Accept: "application/octet-stream", Range: "bytes=0-0" },
    redirect: "follow",
  });
  if (assetResponse.status !== 206) {
    await assetResponse.body?.cancel();
    throw new Error(`FFmpeg archive is unavailable (${assetResponse.status}): ${asset}`);
  }
  await assetResponse.body?.cancel();
}

console.log(`FFmpeg source release ${audioToolsRelease.tag} is available and verified.`);
