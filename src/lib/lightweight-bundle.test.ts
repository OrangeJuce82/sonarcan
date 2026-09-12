import assert from "node:assert/strict";
import test from "node:test";

import { DEFAULT_MAX_BUNDLE_BYTES, forbiddenRuntimePath } from "../../scripts/verify-lightweight-bundle.mjs";
import {
  forbiddenInferenceDependency,
  validateProductionBackends,
} from "../../scripts/verify-bundled-release.mjs";

test("release bundles reject embedded Python and PyTorch runtimes", () => {
  for (const path of [
    "Resources/python-runtime/runtime/bin/python3.13",
    "Resources/chord-runtime/runtime/bin/python3.12",
    "Resources/mlx-runtime/runtime/lib/libpython3.13.dylib",
    "Resources/models/final0.ckpt",
    "Resources/models/htdemucs.safetensors",
    "usr/lib/sonarcan/site-packages/numpy/__init__.py",
    "Resources/libtorch_cpu.dylib",
    "usr/lib/libtorch_cuda.so.2",
  ]) assert.equal(forbiddenRuntimePath(path), true, path);
});

test("bundled runtime accepts only the platform GPU delegate", () => {
  assert.doesNotThrow(() => validateProductionBackends('{"MLXBackend":true}', "darwin"));
  assert.doesNotThrow(() => validateProductionBackends('{"CudaBackend":true}', "linux"));
  assert.throws(
    () => validateProductionBackends('{"MLXBackend":true,"XnnpackBackend":true}', "darwin"),
    /GPU-only MLXBackend/u,
  );
  assert.throws(
    () => validateProductionBackends('{"CudaBackend":false}', "linux"),
    /GPU-only CudaBackend/u,
  );
});

test("the selective native runtime remains eligible", () => {
  assert.equal(forbiddenRuntimePath("Resources/executorch-runtime/sonarcan-executorch-worker"), false);
  assert.equal(DEFAULT_MAX_BUNDLE_BYTES, 512 * 1024 * 1024);
});

test("native dependency inspection rejects hidden Python and PyTorch linkage", () => {
  assert.match(
    forbiddenInferenceDependency("/usr/lib/libc++.so\n/app/lib/libtorch_cpu.so") ?? "",
    /libtorch_cpu/u,
  );
  assert.match(
    forbiddenInferenceDependency("/app/site-packages/torch/lib/libomp.dylib") ?? "",
    /site-packages/u,
  );
  assert.equal(
    forbiddenInferenceDependency("/System/Library/Frameworks/CoreML.framework/CoreML\n/usr/lib/libc++.1.dylib"),
    undefined,
  );
});
