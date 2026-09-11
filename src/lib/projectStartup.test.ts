import assert from "node:assert/strict";
import test from "node:test";

import { projectStartupAction, shouldProcessProjectOpenRequest } from "./projectStartup.ts";

test("startup distinguishes restoring a recent project from creating a temporary one", () => {
  assert.equal(projectStartupAction(["/projects/recent.sac"]), "restoreRecent");
  assert.equal(projectStartupAction([]), "createTemporary");
});

test("native project opening waits for startup capability qualification", () => {
  assert.equal(shouldProcessProjectOpenRequest(false), false);
  assert.equal(shouldProcessProjectOpenRequest(true), true);
});
