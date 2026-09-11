import { createHash } from "node:crypto";
import { chmod, readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const resourceDirectory = path.join(repositoryRoot, "src-tauri/resources/ytdlp-search");
const manifest = JSON.parse(await readFile(path.join(resourceDirectory, "manifest.json"), "utf8"));
const artifact = manifest.artifacts[`${process.platform}-${process.arch}`];
if (!artifact) throw new Error(`No pinned yt-dlp artifact for ${process.platform}-${process.arch}`);
const destinationName = process.platform === "win32" ? "yt-dlp.exe" : "yt-dlp";
const destination = path.join(resourceDirectory, destinationName);
const verifyOnly = process.argv.includes("--verify");
const maximumBytes = 100 * 1024 * 1024;

async function verify(bytes) {
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== artifact.sha256) {
    throw new Error(`yt-dlp checksum mismatch: expected ${artifact.sha256}, received ${actual}`);
  }
}

if (verifyOnly) {
  await verify(await readFile(destination));
  console.log(`Verified standalone yt-dlp ${manifest.version} for ${process.platform}-${process.arch}`);
} else {
  const url = `https://github.com/yt-dlp/yt-dlp/releases/download/${manifest.version}/${artifact.filename}`;
  const response = await fetch(url, { redirect: "follow" });
  if (!response.ok) throw new Error(`Could not download standalone yt-dlp: HTTP ${response.status}`);
  const declaredLength = Number(response.headers.get("content-length") ?? 0);
  if (declaredLength > maximumBytes) throw new Error("standalone yt-dlp exceeds 100 MiB");
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length > maximumBytes) throw new Error("standalone yt-dlp exceeds 100 MiB");
  await verify(bytes);
  const temporary = `${destination}.tmp`;
  await rm(temporary, { force: true });
  await writeFile(temporary, bytes, { mode: 0o644 });
  if (process.platform !== "win32") await chmod(temporary, 0o755);
  await rename(temporary, destination);
  console.log(`Prepared standalone yt-dlp ${manifest.version} for ${process.platform}-${process.arch}`);
}
