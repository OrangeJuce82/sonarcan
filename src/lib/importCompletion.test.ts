import assert from "node:assert/strict";
import test from "node:test";

import { completedImportBatch } from "./importCompletion.ts";

test("an import batch is summarized only after every job settles", () => {
  assert.equal(completedImportBatch(2, ["completed", "importing"]), null);
  assert.equal(completedImportBatch(2, ["completed"]), null);
  assert.deepEqual(completedImportBatch(2, ["completed", "failed"]), {
    completed: 1,
    failed: 1,
  });
});

test("a successful import batch reports one aggregate count", () => {
  assert.deepEqual(completedImportBatch(5, Array(5).fill("completed")), {
    completed: 5,
    failed: 0,
  });
});
