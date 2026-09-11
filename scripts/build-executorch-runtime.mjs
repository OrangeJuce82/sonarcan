import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceSetting = process.env.SONARCAN_EXECUTORCH_SOURCE;
const programDirectorySetting = process.env.SONARCAN_EXECUTORCH_PTE_DIR;
if (!sourceSetting) {
  throw new Error("Set SONARCAN_EXECUTORCH_SOURCE to an ExecuTorch 1.4.1 source checkout");
}
const source = resolve(sourceSetting);
const programDirectory = programDirectorySetting ? resolve(programDirectorySetting) : undefined;
const buildDirectory = resolve(
  process.env.SONARCAN_EXECUTORCH_BUILD_DIR
    ?? join(root, "tools/sonarcan-executorch-worker/build"),
);
const resourceDirectory = resolve(
  process.env.SONARCAN_EXECUTORCH_OUTPUT_DIR
    ?? join(root, "src-tauri/resources/executorch-runtime"),
);
const executableName = process.platform === "win32"
  ? "sonarcan-executorch-worker.exe"
  : "sonarcan-executorch-worker";
const bundledPython = process.platform === "win32"
  ? join(root, "src-tauri/resources/python-runtime/runtime/python.exe")
  : join(root, "src-tauri/resources/python-runtime/runtime/bin/python3.13");
const python = resolve(process.env.SONARCAN_EXECUTORCH_PYTHON ?? bundledPython);
const pythonPath = [
  source,
  process.env.SONARCAN_EXECUTORCH_PYTHONPATH,
  process.env.PYTHONPATH,
].filter(Boolean).join(process.platform === "win32" ? ";" : ":");

if (!existsSync(join(source, "CMakeLists.txt"))) {
  throw new Error("Set SONARCAN_EXECUTORCH_SOURCE to an ExecuTorch 1.4.1 source checkout");
}
const sourceVersion = readFileSync(join(source, "version.txt"), "utf8").trim();
if (sourceVersion !== "1.4.1") {
  throw new Error(`ExecuTorch 1.4.1 is required, found ${sourceVersion}`);
}
if (programDirectory && !existsSync(programDirectory)) {
  throw new Error("Set SONARCAN_EXECUTORCH_PTE_DIR to a directory containing release PTE files");
}
if (!existsSync(python)) throw new Error("Missing build-time Python with ExecuTorch installed");

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

const operatorManifest = join(root, "tools/sonarcan-executorch-worker/operators.txt");
const pinnedOperators = readFileSync(operatorManifest, "utf8")
  .split(/\r?\n/u)
  .map((value) => value.trim())
  .filter(Boolean)
  .sort();
if (pinnedOperators.length === 0) throw new Error("The pinned operator manifest is empty");

function run(command, commandArguments, options = {}) {
  const result = spawnSync(command, commandArguments, {
    cwd: root,
    env: options.env ?? process.env,
    encoding: "utf8",
    stdio: options.capture ? ["ignore", "pipe", "inherit"] : "inherit",
  });
  if (result.error) throw result.error;
  if (result.status !== 0) throw new Error(`${command} exited with status ${result.status}`);
  return result.stdout?.trim() ?? "";
}

let programs = [];
if (programDirectory) {
  programs = filesBelow(programDirectory).filter((path) => path.endsWith(".pte")).sort();
  if (programs.length === 0) throw new Error("No PTE files found for operator-manifest verification");
  const discoveredOperators = run(
    python,
    [join(root, "scripts/list-executorch-operators.py"), ...programs],
    { capture: true, env: { ...process.env, PYTHONPATH: pythonPath } },
  ).split(",").sort();
  if (JSON.stringify(discoveredOperators) !== JSON.stringify(pinnedOperators)) {
    throw new Error("Release PTE operators do not match the pinned selective-runtime manifest");
  }
}
const operators = pinnedOperators.join(",");
run("cmake", [
  "-Wno-deprecated",
  "-S", join(root, "tools/sonarcan-executorch-worker"),
  "-B", buildDirectory,
  "-DCMAKE_BUILD_TYPE=Release",
  `-DPYTHON_EXECUTABLE=${python}`,
  `-DSONARCAN_EXECUTORCH_SOURCE=${source}`,
  `-DEXECUTORCH_SELECT_OPS_LIST=${operators}`,
]);
run("cmake", ["--build", buildDirectory, "--target", "sonarcan-executorch-worker", "-j", "8"]);

const builtExecutable = join(buildDirectory, executableName);
if (!existsSync(builtExecutable)) throw new Error(`Missing built worker: ${builtExecutable}`);
mkdirSync(resourceDirectory, { recursive: true });
copyFileSync(builtExecutable, join(resourceDirectory, executableName));
if (process.platform !== "win32") run("chmod", ["755", join(resourceDirectory, executableName)]);
const verification = programs.length === 0 ? "the pinned operator manifest" : `${programs.length} PTE files`;
console.log(`Built selective runtime verified against ${verification} in ${resourceDirectory}`);
