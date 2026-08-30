import { createHash } from "node:crypto";
import { readFile, rename, rm, writeFile } from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const repositoryRoot = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const resourceDirectory = path.join(repositoryRoot, "src-tauri/resources/ytdlp-search");
const manifest = JSON.parse(await readFile(path.join(resourceDirectory, "manifest.json"), "utf8"));
const destination = path.join(resourceDirectory, manifest.filename);
const verifyOnly = process.argv.includes("--verify");

async function verify(bytes) {
  const actual = createHash("sha256").update(bytes).digest("hex");
  if (actual !== manifest.sha256) {
    throw new Error(`yt-dlp search checksum mismatch: expected ${manifest.sha256}, received ${actual}`);
  }
}

if (verifyOnly) {
  await verify(await readFile(destination));
  console.log(`Verified yt-dlp search ${manifest.version}`);
} else {
  const response = await fetch(manifest.url, { redirect: "follow" });
  if (!response.ok) throw new Error(`Could not download yt-dlp search artifact: HTTP ${response.status}`);
  const declaredLength = Number(response.headers.get("content-length") ?? 0);
  if (declaredLength > 8 * 1024 * 1024) throw new Error("yt-dlp search artifact exceeds 8 MiB");
  const bytes = Buffer.from(await response.arrayBuffer());
  if (bytes.length > 8 * 1024 * 1024) throw new Error("yt-dlp search artifact exceeds 8 MiB");
  await verify(bytes);
  const temporary = `${destination}.tmp`;
  await rm(temporary, { force: true });
  await writeFile(temporary, bytes, { mode: 0o644 });
  await rename(temporary, destination);
  console.log(`Prepared yt-dlp search ${manifest.version}`);
}
