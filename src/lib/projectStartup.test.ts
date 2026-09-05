import assert from "node:assert/strict";
import test from "node:test";

import { projectStartupAction } from "./projectStartup.ts";

test("startup distinguishes restoring a recent project from creating a temporary one", () => {
  assert.equal(projectStartupAction(["/projects/recent.sac"]), "restoreRecent");
  assert.equal(projectStartupAction([]), "createTemporary");
});
