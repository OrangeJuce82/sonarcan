import assert from "node:assert/strict";
import test from "node:test";

import { prepareAnalysisForStartup, projectStartupAction, shouldProcessProjectOpenRequest } from "./projectStartup.ts";

test("startup distinguishes restoring a recent project from creating a temporary one", () => {
  assert.equal(projectStartupAction(["/projects/recent.sac"]), "restoreRecent");
  assert.equal(projectStartupAction([]), "createTemporary");
});

test("native project opening waits for startup capability qualification", () => {
  assert.equal(shouldProcessProjectOpenRequest(false), false);
  assert.equal(shouldProcessProjectOpenRequest(true), true);
});

test("startup always verifies and repairs the model cache before probing the accelerator", async () => {
  const calls: string[] = [];
  let modelsReady = false;
  const prepareModels = async () => {
    calls.push("prepareModels");
    modelsReady = true;
  };
  const getAnalysisCapabilities = async () => {
    calls.push("getAnalysisCapabilities");
    return { accelerated: modelsReady };
  };

  const firstStartupAccelerated = await prepareAnalysisForStartup(
    prepareModels,
    getAnalysisCapabilities,
  );
  const secondStartupAccelerated = await prepareAnalysisForStartup(
    prepareModels,
    getAnalysisCapabilities,
  );

  assert.equal(firstStartupAccelerated, true);
  assert.equal(secondStartupAccelerated, true);
  assert.deepEqual(calls, [
    "prepareModels",
    "getAnalysisCapabilities",
    "prepareModels",
    "getAnalysisCapabilities",
  ]);
});

test("startup does not probe the accelerator when model verification fails", async () => {
  let probed = false;

  await assert.rejects(() => prepareAnalysisForStartup(
    async () => {
      throw new Error("invalid model cache");
    },
    async () => {
      probed = true;
      return { accelerated: true };
    },
  ));

  assert.equal(probed, false);
});
