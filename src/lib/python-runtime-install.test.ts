import assert from "node:assert/strict";
import test from "node:test";

import {
  madmomBuildDependencies,
} from "../../scripts/python-runtime-install.mjs";

test("madmom's undeclared build dependencies are bootstrapped explicitly", () => {
  assert.deepEqual(madmomBuildDependencies, [
    "setuptools==80.9.0",
    "numpy==2.3.5",
    "cython @ git+https://github.com/cython/cython.git@8a1b3c10260fa9f9a91475819d737bce59b1a3d0",
  ]);
});
