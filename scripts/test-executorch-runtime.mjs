import {
  mkdtempSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { spawnSync } from "node:child_process";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const root = resolve(dirname(fileURLToPath(import.meta.url)), "..");
const executableName = process.platform === "win32"
  ? "sonarcan-executorch-worker.exe"
  : "sonarcan-executorch-worker";
const worker = resolve(
  process.env.SONARCAN_EXECUTORCH_WORKER
    ?? join(root, "src-tauri/resources/executorch-runtime", executableName),
);
const modelSetting = process.env.SONARCAN_EXECUTORCH_TEST_MODEL;
if (!modelSetting) throw new Error("Set SONARCAN_EXECUTORCH_TEST_MODEL to beat-this.pte");
const model = resolve(modelSetting);
const lvModelSetting = process.env.SONARCAN_EXECUTORCH_LV_TEST_MODEL;
const lvRecurrentModelSetting = process.env.SONARCAN_EXECUTORCH_LV_RECURRENT_TEST_MODEL;
const temporary = mkdtempSync(join(tmpdir(), "sonarcan-executorch-test-"));

function run(arguments_, expectedStatus = 0) {
  const result = spawnSync(worker, arguments_, { encoding: "utf8" });
  if (result.error) throw result.error;
  if (result.status !== expectedStatus) {
    throw new Error(
      `worker exited with ${result.status}; stdout=${result.stdout}; stderr=${result.stderr}`,
    );
  }
  return result;
}

function writeZeroTensor(path, dimensions) {
  const elementCount = dimensions.reduce((total, value) => total * value, 1);
  const buffer = Buffer.alloc(8 + 4 + dimensions.length * 8 + elementCount * 4);
  buffer.write("SACTEN01", 0, "ascii");
  buffer.writeUInt32LE(dimensions.length, 8);
  dimensions.forEach((value, index) => buffer.writeBigUInt64LE(BigInt(value), 12 + index * 8));
  writeFileSync(path, buffer);
}

function readTensor(path) {
  const buffer = readFileSync(path);
  if (buffer.subarray(0, 8).toString("ascii") !== "SACTEN01") {
    throw new Error("worker returned an invalid tensor header");
  }
  const rank = buffer.readUInt32LE(8);
  const dimensions = Array.from({ length: rank }, (_, index) =>
    Number(buffer.readBigUInt64LE(12 + index * 8))
  );
  const offset = 12 + rank * 8;
  const count = dimensions.reduce((total, value) => total * value, 1);
  if (buffer.length !== offset + count * 4) throw new Error("worker returned an invalid payload size");
  for (let index = 0; index < count; ++index) {
    if (!Number.isFinite(buffer.readFloatLE(offset + index * 4))) {
      throw new Error(`worker returned a non-finite value at output index ${index}`);
    }
  }
  return dimensions;
}

try {
  const input = join(temporary, "beat.tensor");
  const output = join(temporary, "output");
  writeZeroTensor(input, [1, 1500, 128]);
  const result = run(["infer", model, output, input]);
  const response = JSON.parse(result.stdout.trim());
  if (response.outputs !== 2) throw new Error(`expected 2 outputs, received ${response.outputs}`);
  const shapes = [readTensor(join(output, "0.tensor")), readTensor(join(output, "1.tensor"))];

  if (lvModelSetting) {
    const lvInput = join(temporary, "lv-convolution.tensor");
    const lvOutput = join(temporary, "lv-convolution-output");
    writeZeroTensor(lvInput, [1, 1, 18, 252]);
    const lvResult = run([
      "infer",
      resolve(lvModelSetting),
      lvOutput,
      lvInput,
    ]);
    const lvResponse = JSON.parse(lvResult.stdout.trim());
    if (lvResponse.outputs !== 1) {
      throw new Error(`expected one LV-Chordia output, received ${lvResponse.outputs}`);
    }
    const lvShape = readTensor(join(lvOutput, "0.tensor"));
    if (JSON.stringify(lvShape) !== JSON.stringify([1, 16, 18, 252])) {
      throw new Error(`unexpected LV-Chordia output shape ${JSON.stringify(lvShape)}`);
    }
  }

  if (lvRecurrentModelSetting) {
    const recurrentInput = join(temporary, "lv-recurrent.tensor");
    const recurrentState = join(temporary, "lv-recurrent-state.tensor");
    const recurrentOutput = join(temporary, "lv-recurrent-output");
    writeZeroTensor(recurrentInput, [1, 16, 240]);
    writeZeroTensor(recurrentState, [1, 1, 96]);
    const recurrentResult = run([
      "infer",
      resolve(lvRecurrentModelSetting),
      recurrentOutput,
      recurrentInput,
      recurrentState,
      recurrentState,
    ]);
    const recurrentResponse = JSON.parse(recurrentResult.stdout.trim());
    if (recurrentResponse.outputs !== 3) {
      throw new Error(`expected three recurrent outputs, received ${recurrentResponse.outputs}`);
    }
    const recurrentShapes = [0, 1, 2].map((index) =>
      readTensor(join(recurrentOutput, `${index}.tensor`))
    );
    const expectedShapes = [[1, 16, 96], [1, 1, 96], [1, 1, 96]];
    if (JSON.stringify(recurrentShapes) !== JSON.stringify(expectedShapes)) {
      throw new Error(`unexpected LV-Chordia recurrent shapes ${JSON.stringify(recurrentShapes)}`);
    }
  }

  if (process.platform !== "win32") {
    const linkedInput = join(temporary, "linked.tensor");
    symlinkSync(input, linkedInput);
    run(["infer", model, join(temporary, "linked-output"), linkedInput], 1);
  }
  const lvStatus = lvModelSetting ? " and LV-Chordia convolution inference" : "";
  const recurrentStatus = lvRecurrentModelSetting ? " and recurrent inference" : "";
  console.log(
    `ExecuTorch Beat This inference${lvStatus}${recurrentStatus} passed with output shapes ${JSON.stringify(shapes)}`,
  );
} finally {
  rmSync(temporary, { recursive: true, force: true });
}
