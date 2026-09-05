import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { madmomBuildDependencies, runtimePipArguments } from "./python-runtime-install.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const gpuBackend = process.env.SONARCAN_GPU_BACKEND;
if (gpuBackend && !["nvidia", "amd"].includes(gpuBackend)) {
  throw new Error(`unsupported SONARCAN_GPU_BACKEND: ${gpuBackend}`);
}
if (gpuBackend && process.platform === "darwin") {
  throw new Error("SONARCAN_GPU_BACKEND is only valid for Windows and Linux runtimes");
}
const runtimeProject = gpuBackend === "nvidia"
  ? "sonarcan-python-runtime-cuda"
  : gpuBackend === "amd"
    ? "sonarcan-python-runtime-rocm"
    : "sonarcan-python-runtime";
const project = join(repositoryRoot, `tools/${runtimeProject}`);
const runtime = join(repositoryRoot, "src-tauri/resources/python-runtime/runtime");
const requirements = join(runtime, "requirements.lock.txt");
const beatModel = join(repositoryRoot, "src-tauri/resources/models/beat-this/final0.ckpt");
const stemModel = join(repositoryRoot, "src-tauri/resources/models/demucs-mlx");
const appleSilicon = process.platform === "darwin" && process.arch === "arm64";
const stemPackage = appleSilicon ? "sonarcan-mlx-worker" : "sonarcan-torch-worker";
const stemModule = appleSilicon ? "sonarcan_mlx_worker" : "sonarcan_torch_worker.worker";

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, {
    cwd: repositoryRoot,
    encoding: "utf8",
    stdio: options.capture ? "pipe" : "inherit",
    ...options,
  });
  if (result.status !== 0) {
    const detail = options.capture ? `\n${result.stderr || result.stdout}` : "";
    throw new Error(`${command} failed with status ${result.status}${detail}`);
  }
  return options.capture ? result.stdout.trim() : "";
}

for (const model of [beatModel, join(stemModel, "htdemucs_6s.safetensors")]) {
  if (!existsSync(model)) {
    throw new Error(`required release model is missing: ${model}`);
  }
}

const managedPython = run("uv", ["python", "find", "--managed-python", "3.13.5"], {
  capture: true,
});
const managedRoot = process.platform === "win32" ? dirname(managedPython) : dirname(dirname(managedPython));
rmSync(runtime, { recursive: true, force: true });
mkdirSync(dirname(runtime), { recursive: true });
cpSync(managedRoot, runtime, { recursive: true, dereference: false });
const runtimePython = process.platform === "win32"
  ? join(runtime, "python.exe")
  : join(runtime, "bin/python3.13");

run("uv", [
  "export", "--quiet", "--project", project, "--locked", "--no-dev", "--no-editable",
  "--output-file", requirements,
]);
run("uv", [
  "pip", "install", "--system", "--break-system-packages", "--python", runtimePython,
  ...(gpuBackend ? ["--no-cache"] : []),
  ...madmomBuildDependencies,
]);
run("uv", [
  "pip", "install", "--system", "--break-system-packages", "--python", runtimePython,
  ...(gpuBackend ? ["--no-cache"] : []),
  "--no-build-isolation-package", "madmom",
  "--reinstall-package", "sonarcan-lv-chordia-worker",
  "--reinstall-package", stemPackage,
  ...runtimePipArguments(process.platform, gpuBackend), "--requirement", requirements,
], { cwd: project });

const sitePackages = process.platform === "win32"
  ? join(runtime, "Lib", "site-packages")
  : join(runtime, "lib", "python3.13", "site-packages");
const madmomModels = join(sitePackages, "madmom", "models");
for (const directory of ["beats", "chords", "chroma", "downbeats", "key", "notes", "onsets", "patterns"]) {
  rmSync(join(madmomModels, directory), { recursive: true, force: true });
}

run(runtimePython, [
  "-m", "sonarcan_chord_worker.worker", "--self-test", "--downbeat-model", beatModel,
]);
run(runtimePython, [
  "-m", stemModule, "self-test", "--model-dir", stemModel,
]);
if (gpuBackend) {
  const expected = gpuBackend === "nvidia" ? "CUDA" : "ROCm";
  run(runtimePython, [
    "-c",
    `import torch; assert torch.version.cuda if ${JSON.stringify(gpuBackend)} == 'nvidia' else torch.version.hip; print('${expected} runtime present')`,
  ]);
}

console.log(`Pinned ${gpuBackend ?? (appleSilicon ? "apple" : "cpu")} Python 3.13 runtime assembled in ${runtime}`);
