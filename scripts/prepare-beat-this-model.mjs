import { createHash } from "node:crypto";
import { createWriteStream, existsSync } from "node:fs";
import { mkdir, readFile, rename, rm } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { Readable } from "node:stream";
import { pipeline } from "node:stream/promises";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const modelDirectory = join(root, "src-tauri/resources/models/beat-this");
const model = join(modelDirectory, "final0.ckpt");
const temporary = `${model}.tmp`;
const expectedSha256 = "8c328b45f59d8dd3dff219253ff6a8d6482be57d0133a29140e2febbf8eb8331";
const modelUrl = "https://cloud.cp.jku.at/public.php/dav/files/7ik4RrBKTS273gp/final0.ckpt";

async function sha256(path) {
  return createHash("sha256").update(await readFile(path)).digest("hex");
}

async function verify(path) {
  const actual = await sha256(path);
  if (actual !== expectedSha256) {
    throw new Error(`Beat This! final0 checkpoint checksum mismatch: expected ${expectedSha256}, received ${actual}`);
  }
}

await mkdir(modelDirectory, { recursive: true });
if (!existsSync(model)) {
  await rm(temporary, { force: true });
  try {
    const response = await fetch(modelUrl, { redirect: "follow" });
    if (!response.ok || !response.body) {
      throw new Error(`Could not download Beat This! final0 checkpoint: HTTP ${response.status}`);
    }
    await pipeline(Readable.fromWeb(response.body), createWriteStream(temporary, { flags: "wx" }));
    await verify(temporary);
    await rename(temporary, model);
  } finally {
    await rm(temporary, { force: true });
  }
}

await verify(model);
console.log(`Pinned Beat This! final0 checkpoint is ready in ${modelDirectory}`);
