import { existsSync, readFileSync, readdirSync } from "node:fs";
import { spawnSync } from "node:child_process";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const sourceSetting = process.env.SONARCAN_EXECUTORCH_SOURCE;
const programDirectorySetting = process.env.SONARCAN_EXECUTORCH_PTE_DIR;
if (!sourceSetting) {
  throw new Error("Set SONARCAN_EXECUTORCH_SOURCE to an ExecuTorch 1.4.1 source checkout");
}
if (!programDirectorySetting) {
  throw new Error("Set SONARCAN_EXECUTORCH_PTE_DIR to a directory containing release PTE files");
}
const source = resolve(sourceSetting);
const programDirectory = resolve(programDirectorySetting);
const buildDirectory = resolve(
  process.env.SONARCAN_EXECUTORCH_BUILD_DIR
    ?? join(root, "tools/sonarcan-executorch-probe/build"),
);
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
if (!existsSync(programDirectory)) {
  throw new Error("Set SONARCAN_EXECUTORCH_PTE_DIR to a directory containing release PTE files");
}
if (!existsSync(python)) {
  throw new Error("Missing build-time Python with ExecuTorch installed");
}

function filesBelow(directory) {
  return readdirSync(directory, { withFileTypes: true }).flatMap((entry) => {
    const path = join(directory, entry.name);
    return entry.isDirectory() ? filesBelow(path) : [path];
  });
}

const programs = filesBelow(programDirectory).filter((path) => path.endsWith(".pte")).sort();
if (programs.length === 0) throw new Error("No PTE files found for selective runtime generation");

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

const operators = run(
  python,
  [join(root, "scripts/list-executorch-operators.py"), ...programs],
  {
    capture: true,
    env: { ...process.env, PYTHONPATH: pythonPath },
  },
);

const windowsCompilerArguments = process.platform === "win32"
  ? [
      "-G", "Ninja",
      "-DCMAKE_C_COMPILER=clang",
      "-DCMAKE_CXX_COMPILER=clang++",
      "-DCMAKE_POSITION_INDEPENDENT_CODE=OFF",
    ]
  : [];
run("cmake", [
  ...windowsCompilerArguments,
  "-S", join(root, "tools/sonarcan-executorch-probe"),
  "-B", buildDirectory,
  "-DCMAKE_BUILD_TYPE=Release",
  `-DPYTHON_EXECUTABLE=${python}`,
  `-DSONARCAN_EXECUTORCH_SOURCE=${source}`,
  `-DEXECUTORCH_SELECT_OPS_LIST=${operators}`,
]);
// ExecuTorch 1.4.1 omits the .exe suffix from these ExternalProject
// byproducts. Ninja therefore cannot infer how to create the imported host
// tools when they are first needed by schema generation on Windows.
if (process.platform === "win32") {
  run("cmake", [
    "--build", buildDirectory,
    "--target", "flatbuffers_ep", "flatcc_ep",
    "-j", "8",
  ]);
}
run("cmake", ["--build", buildDirectory, "--target", "sonarcan-executorch-probe", "-j", "8"]);

console.log(`Built selective probe for ${programs.length} PTE files in ${buildDirectory}`);
