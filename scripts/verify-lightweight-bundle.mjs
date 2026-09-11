import { lstatSync, readdirSync } from "node:fs";
import { basename, join, relative, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

export const DEFAULT_MAX_BUNDLE_BYTES = 256 * 1024 * 1024;

const forbiddenComponents = new Set(["python-runtime", "site-packages", "torch", "torchgen"]);
const forbiddenFiles = [
  /^python3(?:\.\d+)?$/i,
  /^(?:lib)?torch(?:_cpu|_cuda|_global_deps|_python)?\.(?:dylib|so(?:\.\d+)*)$/i,
];

/** @param {string} path */
export function forbiddenRuntimePath(path) {
  const components = path.split(/[\\/]+/).filter(Boolean);
  if (components.some((component) => forbiddenComponents.has(component.toLowerCase()))) return true;
  const file = basename(path);
  return forbiddenFiles.some((pattern) => pattern.test(file));
}

/** @param {string} root */
export function inspectBundle(root) {
  const absoluteRoot = resolve(root);
  let bytes = 0;
  const forbidden = [];
  const pending = [absoluteRoot];
  while (pending.length) {
    const directory = pending.pop();
    if (!directory) continue;
    for (const entry of readdirSync(directory, { withFileTypes: true })) {
      const path = join(directory, entry.name);
      const displayPath = relative(absoluteRoot, path).split(sep).join("/");
      const metadata = lstatSync(path);
      if (forbiddenRuntimePath(displayPath)) {
        forbidden.push(displayPath);
        continue;
      }
      if (metadata.isSymbolicLink()) continue;
      if (entry.isDirectory()) pending.push(path);
      if (metadata.isFile()) bytes += metadata.size;
    }
  }
  return { bytes, forbidden: forbidden.sort() };
}

function main() {
  const root = process.argv[2];
  if (!root) throw new Error("usage: verify-lightweight-bundle BUNDLE_ROOT");
  const maximum = Number(process.env.SONARCAN_MAX_BUNDLE_BYTES ?? DEFAULT_MAX_BUNDLE_BYTES);
  if (!Number.isSafeInteger(maximum) || maximum <= 0) throw new Error("invalid bundle size limit");
  const result = inspectBundle(root);
  if (result.forbidden.length) {
    throw new Error(`bundle contains forbidden Python/PyTorch runtime paths:\n${result.forbidden.join("\n")}`);
  }
  if (result.bytes > maximum) {
    throw new Error(`bundle is ${result.bytes} bytes; lightweight limit is ${maximum} bytes`);
  }
  console.log(`Lightweight bundle qualified: ${result.bytes} / ${maximum} bytes`);
}

if (resolve(process.argv[1] ?? "") === fileURLToPath(import.meta.url)) main();
