import { existsSync, mkdtempSync, readFileSync, rmSync, statSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { performance } from "node:perf_hooks";
import { spawnSync } from "node:child_process";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const appleSilicon = process.env.SONARCAN_STEM_BACKEND !== "torch"
  && process.platform === "darwin" && process.arch === "arm64";
const python = process.platform === "win32"
  ? join(root, "src-tauri/resources/python-runtime/runtime/python.exe")
  : join(root, "src-tauri/resources/python-runtime/runtime/bin/python3.13");
const suffix = process.platform === "win32" ? ".exe" : "";
const ffmpeg = join(root, `src-tauri/resources/audio-tools/bin/ffmpeg${suffix}`);
const model = join(root, "src-tauri/resources/models/demucs-mlx");
const stems = ["vocals", "drums", "bass", "other", "guitar", "piano"];

for (const path of [python, ffmpeg]) {
  if (!existsSync(path)) throw new Error(`qualification dependency is missing: ${path}`);
}

function run(command, argumentsList) {
  const result = spawnSync(command, argumentsList, { cwd: root, stdio: "inherit" });
  if (result.status !== 0) throw new Error(`${command} failed with status ${result.status}`);
}

const temporary = mkdtempSync(join(tmpdir(), "sonarcan-stem-qualification-"));
try {
  const input = join(temporary, "input.wav");
  const output = join(temporary, "stems");
  run(ffmpeg, [
    "-hide_banner", "-loglevel", "error",
    "-f", "lavfi", "-i", "sine=frequency=220:sample_rate=44100:duration=15",
    "-f", "lavfi", "-i", "sine=frequency=330:sample_rate=44100:duration=15",
    "-filter_complex", "amerge=inputs=2", "-ac", "2", "-c:a", "pcm_f32le", "-y", input,
  ]);
  const argumentsList = appleSilicon
    ? ["-m", "sonarcan_mlx_worker", "separate", "--input", input, "--output", output, "--model-dir", model]
    : [
        "-m", "sonarcan_torch_worker", "separate", "--input", input, "--output", output,
        "--model-dir", model, "--ffmpeg", ffmpeg,
      ];
  const started = performance.now();
  run(python, argumentsList);
  const elapsedSeconds = (performance.now() - started) / 1000;
  for (const stem of stems) {
    const path = join(output, `${stem}.wav`);
    if (statSync(path).size <= 56 || readFileSync(path).subarray(0, 4).toString() !== "RIFF") {
      throw new Error(`qualification produced an invalid ${stem}.wav`);
    }
  }
  console.log(JSON.stringify({
    qualification: "htdemucs_6s",
    backend: appleSilicon ? "MLX" : "Torch",
    platform: process.platform,
    architecture: process.arch,
    inputSeconds: 15,
    elapsedSeconds: Number(elapsedSeconds.toFixed(3)),
    realtimeFactor: Number((elapsedSeconds / 15).toFixed(3)),
  }));
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
