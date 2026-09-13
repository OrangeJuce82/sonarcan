import { copyFileSync, existsSync, mkdirSync, readFileSync, readdirSync, statSync } from "node:fs";
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
const executableName = "sonarcan-executorch-worker";
if (process.platform !== "darwin" || process.arch !== "arm64") {
  throw new Error("The production ExecuTorch runtime is built only for macOS Apple Silicon");
}
const maximumRuntimeBytes = Number(
  process.env.SONARCAN_MAX_EXECUTORCH_RUNTIME_BYTES ?? 96 * 1024 * 1024,
);
const pythonSetting = process.env.SONARCAN_EXECUTORCH_PYTHON;
if (!pythonSetting) {
  throw new Error("Set SONARCAN_EXECUTORCH_PYTHON to an external build-time Python executable");
}
const python = resolve(pythonSetting);
const pythonPath = [
  source,
  process.env.SONARCAN_EXECUTORCH_PYTHONPATH,
  process.env.PYTHONPATH,
].filter(Boolean).join(":");

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
if (!Number.isSafeInteger(maximumRuntimeBytes) || maximumRuntimeBytes <= 0) {
  throw new Error("SONARCAN_MAX_EXECUTORCH_RUNTIME_BYTES must be a positive integer");
}

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

const clangModuleCache = join(buildDirectory, "clang-module-cache");
mkdirSync(clangModuleCache, { recursive: true });
const buildEnvironment = {
  ...process.env,
  CLANG_MODULE_CACHE_PATH: process.env.CLANG_MODULE_CACHE_PATH ?? clangModuleCache,
};

let programs = [];
if (programDirectory) {
  programs = filesBelow(programDirectory).filter((path) => path.endsWith(".pte")).sort();
  if (programs.length === 0) throw new Error("No PTE files found for operator-manifest verification");
  const discoveredOutput = run(
    python,
    [join(root, "scripts/list-executorch-operators.py"), "--allow-empty", ...programs],
    { capture: true, env: { ...process.env, PYTHONPATH: pythonPath } },
  );
  const discoveredOperators = discoveredOutput ? discoveredOutput.split(",").sort() : [];
  if (discoveredOperators.length !== 0) {
    throw new Error("Production PTEs must be fully delegated and contain no portable ATen operators");
  }
}
const operators = pinnedOperators.join(",");
const cmakeArguments = [
  "-Wno-deprecated",
  "-S", join(root, "tools/sonarcan-executorch-worker"),
  "-B", buildDirectory,
  "-DCMAKE_BUILD_TYPE=Release",
  `-DPYTHON_EXECUTABLE=${python}`,
  `-DSONARCAN_EXECUTORCH_SOURCE=${source}`,
];
if (operators) cmakeArguments.push(`-DEXECUTORCH_SELECT_OPS_LIST=${operators}`);
run("cmake", cmakeArguments, { env: buildEnvironment });
run("cmake", ["--build", buildDirectory, "--target", "sonarcan-executorch-worker", "-j", "8"], {
  env: buildEnvironment,
});

const builtExecutable = [
  join(buildDirectory, executableName),
  join(buildDirectory, "Release", executableName),
].find((candidate) => existsSync(candidate));
if (!builtExecutable) throw new Error(`Missing built worker in ${buildDirectory}`);
mkdirSync(resourceDirectory, { recursive: true });
copyFileSync(builtExecutable, join(resourceDirectory, executableName));
const installedExecutable = join(resourceDirectory, executableName);
run("chmod", ["755", installedExecutable]);
const registeredBackends = JSON.parse(run(installedExecutable, ["--backends"], { capture: true }));
const expectedBackends = ["MLXBackend"];
if (registeredBackends.XnnpackBackend === true) {
  throw new Error("Production runtime must not register the XNNPACK CPU backend");
}
const missingBackends = expectedBackends.filter((backend) => registeredBackends[backend] !== true);
if (missingBackends.length) {
  throw new Error(`Selective runtime failed to register: ${missingBackends.join(", ")}`);
}
const mlxMetallib = filesBelow(buildDirectory).find((path) => path.endsWith("/mlx.metallib"));
if (!mlxMetallib) throw new Error("MLX runtime was built without mlx.metallib");
copyFileSync(mlxMetallib, join(resourceDirectory, "mlx.metallib"));
const runtimeFiles = [
  installedExecutable,
  join(resourceDirectory, "mlx.metallib"),
];
const runtimeBytes = runtimeFiles.reduce((total, path) => total + statSync(path).size, 0);
if (runtimeBytes > maximumRuntimeBytes) {
  throw new Error(
    `Selective ExecuTorch runtime is ${runtimeBytes} bytes; limit is ${maximumRuntimeBytes} bytes`,
  );
}
const verification = programs.length === 0 ? "the bootstrap kernel manifest" : `${programs.length} fully delegated PTE files`;
console.log(`Built ${runtimeBytes}-byte selective runtime verified against ${verification} in ${resourceDirectory}`);
