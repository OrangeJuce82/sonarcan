import assert from "node:assert/strict";
import test from "node:test";

import { runtimePipArguments } from "../../scripts/python-runtime-install.mjs";

test("portable stem runtimes resolve PyTorch from the CPU wheel index", () => {
  assert.deepEqual(runtimePipArguments("stem", "linux"), ["--torch-backend", "cpu"]);
  assert.deepEqual(runtimePipArguments("stem", "win32"), ["--torch-backend", "cpu"]);
});

test("Apple Silicon and chord runtimes keep their locked default indexes", () => {
  assert.deepEqual(runtimePipArguments("stem", "darwin"), []);
  assert.deepEqual(runtimePipArguments("chord", "linux"), []);
  assert.deepEqual(runtimePipArguments("chord", "win32"), []);
});
