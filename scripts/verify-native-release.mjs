import { readFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const tauri = JSON.parse(readFileSync(join(root, "src-tauri/tauri.conf.json"), "utf8"));
const resources = tauri.bundle?.resources ?? {};
const beforeBuild = tauri.build?.beforeBuildCommand ?? "";

const serializedResources = JSON.stringify(resources).toLowerCase();
if (/(python|pytorch|torch-worker|mlx-worker|site-packages)/u.test(serializedResources)) {
  throw new Error("release resources must not contain a Python/PyTorch runtime");
}
if (!Object.hasOwn(resources, "resources/executorch-runtime")) {
  throw new Error("release resources must contain the selective ExecuTorch runtime");
}
if (/(python:runtime|verify:stem-release|verify:chord-release|verify:gpu-runtime)/u.test(beforeBuild)) {
  throw new Error("the release build command still depends on a legacy inference runtime");
}

console.log("Native release contract verified: ExecuTorch only, no Python/PyTorch fallback");
