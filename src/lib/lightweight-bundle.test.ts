import assert from "node:assert/strict";
import test from "node:test";

import { DEFAULT_MAX_BUNDLE_BYTES, forbiddenRuntimePath } from "../../scripts/verify-lightweight-bundle.mjs";

test("release bundles reject embedded Python and PyTorch runtimes", () => {
  for (const path of [
    "Resources/python-runtime/runtime/bin/python3.13",
    "usr/lib/sonarcan/site-packages/numpy/__init__.py",
    "Resources/libtorch_cpu.dylib",
    "usr/lib/libtorch_cuda.so.2",
  ]) assert.equal(forbiddenRuntimePath(path), true, path);
});

test("the selective native runtime remains eligible", () => {
  assert.equal(forbiddenRuntimePath("Resources/executorch-runtime/sonarcan-executorch-worker"), false);
  assert.equal(DEFAULT_MAX_BUNDLE_BYTES, 256 * 1024 * 1024);
});
