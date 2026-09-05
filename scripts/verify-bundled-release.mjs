import { existsSync, readdirSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { spawnSync } from "node:child_process";

const bundleRoot = process.argv[2];
if (!bundleRoot) throw new Error("usage: node scripts/verify-bundled-release.mjs <bundle-or-install-root>");

const root = resolve(bundleRoot);
if (!existsSync(root) || !statSync(root).isDirectory()) {
  throw new Error(`bundle root is not a directory: ${root}`);
}

function findResourceRoot(directory, depth = 0) {
  if (depth > 10) return undefined;
  if (existsSync(join(directory, "python-runtime")) && existsSync(join(directory, "audio-tools"))) return directory;
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    if (!entry.isDirectory() || entry.isSymbolicLink()) continue;
    const found = findResourceRoot(join(directory, entry.name), depth + 1);
    if (found) return found;
  }
  return undefined;
}

function required(path, label) {
  if (!existsSync(path)) throw new Error(`${label} is missing from the bundle: ${path}`);
  return path;
}

function run(command, argumentsList, label, capture = false) {
  const result = spawnSync(command, argumentsList, {
    cwd: root,
    encoding: "utf8",
    env: { ...process.env, PYTHONDONTWRITEBYTECODE: "1" },
    stdio: capture ? "pipe" : "inherit",
  });
  if (result.status !== 0) {
    const detail = capture ? `\n${result.stderr || result.stdout}` : "";
    throw new Error(`${label} failed with status ${result.status}${detail}`);
  }
  return capture ? result.stdout.trim() : "";
}

const resources = findResourceRoot(root);
if (!resources) throw new Error(`could not locate SonArcan resources inside ${root}`);

const windows = process.platform === "win32";
const appleSilicon = process.platform === "darwin" && process.arch === "arm64";
const gpuBackend = process.env.SONARCAN_GPU_BACKEND;
const suffix = windows ? ".exe" : "";
const fullEdition = existsSync(join(resources, "models", "beat-this", "final0.ckpt"));
const sharedPython = required(
  windows
    ? join(resources, "python-runtime", "runtime", "python.exe")
    : join(resources, "python-runtime", "runtime", "bin", "python3.13"),
  "bundled shared Python 3.13",
);
if (fullEdition) {
  const beatModel = required(join(resources, "models", "beat-this", "final0.ckpt"), "Beat This model");
  const chordOutput = run(sharedPython, [
    "-m", "sonarcan_chord_worker.worker", "--self-test", "--downbeat-model", beatModel,
  ], "bundled chord/downbeat worker", true);
  const chordHealth = JSON.parse(chordOutput);
  if (!chordHealth.ok || chordHealth.modes?.join(",") !== "complete,essential,standard") {
    throw new Error("bundled chord/downbeat worker returned an invalid contract");
  }
  const stemModule = appleSilicon ? "sonarcan_mlx_worker" : "sonarcan_torch_worker.worker";
  const model = required(join(resources, "models", "demucs-mlx", "htdemucs_6s.safetensors"), "HTDemucs model");
  run(sharedPython, [
    "-m", stemModule, "self-test", "--model-dir", dirname(model),
  ], `bundled ${appleSilicon ? "MLX" : "Torch"} stem worker`);
  if (appleSilicon) {
    const chordAcceleratorOutput = run(sharedPython, [
      "-m", "sonarcan_chord_worker.worker", "--accelerator-self-test", "--downbeat-model", beatModel,
    ], "bundled MPS chord/downbeat accelerator", true);
    const chordAccelerator = JSON.parse(chordAcceleratorOutput);
    if (!chordAccelerator.accelerated || chordAccelerator.backend !== "MPS") {
      throw new Error("bundled chord/downbeat worker did not qualify MPS");
    }
    run(sharedPython, [
      "-m", stemModule, "accelerator-self-test", "--model-dir", dirname(model),
    ], "bundled MLX stem accelerator");
  } else if (gpuBackend) {
    const qualification = gpuBackend === "nvidia"
      ? "assert torch.version.cuda and not torch.version.hip"
      : "assert torch.version.hip";
    run(sharedPython, ["-c", `import torch; ${qualification}`], `bundled ${gpuBackend} GPU runtime`);
  }
} else {
  run(sharedPython, [
    "-c",
    "import importlib.util; forbidden=('torch','mlx','lv_chordia','beat_this','demucs_mlx'); assert not any(importlib.util.find_spec(name) for name in forbidden)",
  ], "Light runtime heavy-package exclusion");
}

const ffmpeg = required(join(resources, "audio-tools", "bin", `ffmpeg${suffix}`), "bundled FFmpeg");
const ffprobe = required(join(resources, "audio-tools", "bin", `ffprobe${suffix}`), "bundled FFprobe");
run(ffmpeg, ["-hide_banner", "-version"], "bundled FFmpeg");
run(ffprobe, ["-hide_banner", "-version"], "bundled FFprobe");

const ytdlp = required(join(resources, "ytdlp-search", "yt-dlp"), "bundled yt-dlp search artifact");
run(sharedPython, [ytdlp, "--version"], "bundled yt-dlp search artifact");

console.log(JSON.stringify({
  verifiedBundle: root,
  resources,
  platform: process.platform,
  architecture: process.arch,
  edition: fullEdition ? "full" : "light",
  stemBackend: fullEdition ? appleSilicon ? "MLX" : "Torch" : null,
  analysisAcceleratorQualified: fullEdition && (appleSilicon || Boolean(gpuBackend)),
}));
