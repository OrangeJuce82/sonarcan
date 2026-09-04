import assert from "node:assert/strict";
import test from "node:test";

import { runtimePipArguments } from "../../scripts/python-runtime-install.mjs";

test("portable shared runtimes resolve PyTorch from the CPU wheel index", () => {
  assert.deepEqual(runtimePipArguments("linux"), ["--torch-backend", "cpu"]);
  assert.deepEqual(runtimePipArguments("win32"), ["--torch-backend", "cpu"]);
});

test("Apple Silicon keeps its locked default index", () => {
  assert.deepEqual(runtimePipArguments("darwin"), []);
});
