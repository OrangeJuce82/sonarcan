import assert from "node:assert/strict";
import test from "node:test";

import {
  madmomBuildDependencies,
  runtimePipArguments,
} from "../../scripts/python-runtime-install.mjs";

test("portable shared runtimes resolve PyTorch from the CPU wheel index", () => {
  assert.deepEqual(runtimePipArguments("linux"), ["--torch-backend", "cpu"]);
  assert.deepEqual(runtimePipArguments("win32"), ["--torch-backend", "cpu"]);
});

test("Apple Silicon keeps its locked default index", () => {
  assert.deepEqual(runtimePipArguments("darwin"), []);
});

test("NVIDIA releases select the pinned CUDA 12.6 wheel index", () => {
  assert.deepEqual(runtimePipArguments("linux", "nvidia"), ["--torch-backend", "cu126"]);
  assert.deepEqual(runtimePipArguments("win32", "nvidia"), ["--torch-backend", "cu126"]);
});

test("AMD releases retain the locked ROCm index", () => {
  assert.deepEqual(runtimePipArguments("linux", "amd"), [
    "--index",
    "https://download.pytorch.org/whl/rocm7.2",
  ]);
});

test("madmom's undeclared build dependencies are bootstrapped explicitly", () => {
  assert.deepEqual(madmomBuildDependencies, [
    "setuptools==80.9.0",
    "numpy==2.3.5",
    "cython @ git+https://github.com/cython/cython.git@8a1b3c10260fa9f9a91475819d737bce59b1a3d0",
  ]);
});
