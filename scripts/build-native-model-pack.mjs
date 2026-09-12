import { createHash } from "node:crypto";
import { closeSync, openSync, readdirSync, readFileSync, statSync, writeSync } from "node:fs";
import { relative, resolve, sep } from "node:path";

const MAGIC = Buffer.from("SACPKG01", "ascii");
const MAX_PACK_BYTES = 512 * 1024 * 1024;
const [rootSetting, outputSetting] = process.argv.slice(2);
if (!rootSetting || !outputSetting) {
  throw new Error("Usage: node scripts/build-native-model-pack.mjs ROOT OUTPUT.sacmodels");
}
const root = resolve(rootSetting);
const output = resolve(outputSetting);

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = resolve(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

const files = filesBelow(root).sort();
if (files.length === 0 || files.length > 4096) throw new Error("Model pack file count is invalid");
const descriptor = files.map((path) => {
  const name = relative(root, path).split(sep).join("/");
  const nameBytes = Buffer.from(name, "utf8");
  const size = statSync(path).size;
  if (!name || name.startsWith("/") || name.includes("..") || nameBytes.length > 4096) {
    throw new Error(`Unsafe model path: ${name}`);
  }
  return { path, name, nameBytes, size };
});
const predicted = 12 + descriptor.reduce((total, file) => total + 2 + 8 + 32 + file.nameBytes.length + file.size, 0);
if (predicted > MAX_PACK_BYTES) throw new Error(`Model pack exceeds 512 MiB: ${predicted} bytes`);

const fd = openSync(output, "wx");
const whole = createHash("sha256");
function append(value) {
  writeSync(fd, value);
  whole.update(value);
}
try {
  append(MAGIC);
  const count = Buffer.alloc(4);
  count.writeUInt32LE(descriptor.length);
  append(count);
  for (const file of descriptor) {
    const payload = readFileSync(file.path);
    const header = Buffer.alloc(10);
    header.writeUInt16LE(file.nameBytes.length, 0);
    header.writeBigUInt64LE(BigInt(file.size), 2);
    append(header);
    append(createHash("sha256").update(payload).digest());
    append(file.nameBytes);
    append(payload);
  }
} finally {
  closeSync(fd);
}
console.log(JSON.stringify({ output, files: files.length, bytes: predicted, sha256: whole.digest("hex") }));
