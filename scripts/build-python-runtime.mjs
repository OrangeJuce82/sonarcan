import { cpSync, existsSync, mkdirSync, rmSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";
import { runtimePipArguments } from "./python-runtime-install.mjs";

const repositoryRoot = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const runtimeName = process.argv[2];
const configurations = {
  chord: {
    project: "tools/sonarcan-chord-worker",
    resource: "chord-runtime",
    package: "sonarcan-lv-chordia-worker",
    module: "sonarcan_chord_worker.worker",
    healthArguments: [
      "--self-test",
      "--downbeat-model",
      join(repositoryRoot, "src-tauri/resources/models/beat-this/final0.ckpt"),
    ],
  },
  stem: {
    project: "tools/sonarcan-torch-worker",
    resource: "stem-runtime",
    package: "sonarcan-torch-worker",
    module: "sonarcan_torch_worker.worker",
    healthArguments: [
      "self-test",
      "--model-dir",
      join(repositoryRoot, "src-tauri/resources/models/demucs-mlx"),
    ],
  },
};

const configuration = configurations[runtimeName];
if (!configuration) {
  throw new Error("usage: node scripts/build-python-runtime.mjs <chord|stem>");
}

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

const project = join(repositoryRoot, configuration.project);
const runtime = join(repositoryRoot, "src-tauri/resources", configuration.resource, "runtime");
const requirements = join(runtime, "requirements.lock.txt");
const model = configuration.healthArguments.at(-1);
if (!existsSync(model)) {
  throw new Error(`required release model is missing: ${model}`);
}

const managedPython = run("uv", ["python", "find", "--managed-python", "3.12.12"], {
  capture: true,
});
const managedRoot = process.platform === "win32" ? dirname(managedPython) : dirname(dirname(managedPython));
rmSync(runtime, { recursive: true, force: true });
mkdirSync(dirname(runtime), { recursive: true });
cpSync(managedRoot, runtime, { recursive: true, dereference: false });
const runtimePython = process.platform === "win32"
  ? join(runtime, "python.exe")
  : join(runtime, "bin/python3.12");
const pipArguments = runtimePipArguments(runtimeName, process.platform);

run("uv", [
  "export", "--quiet", "--project", project, "--locked", "--no-dev", "--no-editable",
  "--output-file", requirements,
]);
run("uv", [
  "pip", "sync", "--system", "--break-system-packages", "--python", runtimePython,
  "--reinstall-package", configuration.package, ...pipArguments, requirements,
], { cwd: project });
if (runtimeName === "chord") {
  const sitePackages = process.platform === "win32"
    ? join(runtime, "Lib", "site-packages")
    : join(runtime, "lib", "python3.12", "site-packages");
  const madmomModels = join(sitePackages, "madmom", "models");
  for (const directory of ["beats", "chords", "chroma", "downbeats", "key", "notes", "onsets", "patterns"]) {
    rmSync(join(madmomModels, directory), { recursive: true, force: true });
  }
}
run(runtimePython, ["-m", configuration.module, ...configuration.healthArguments]);

console.log(`Pinned ${runtimeName} runtime assembled in ${runtime}`);
