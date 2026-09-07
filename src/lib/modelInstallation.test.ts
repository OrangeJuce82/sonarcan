import assert from "node:assert/strict";
import test from "node:test";
import { formatInstallBytes, modelInstallationCopy, modelInstallationMessage, type ModelInstallProgress } from "./modelInstallation.ts";

const progress: ModelInstallProgress = {
  modelId: "scnet", modelName: "SCNet-large", stage: "downloading", progress: 0.5,
  completedBytes: 1024 * 1024, totalBytes: 2 * 1024 * 1024, modelIndex: 1, modelCount: 4,
};

test("first-run model messages name the active model", () => {
  assert.equal(modelInstallationMessage(progress, modelInstallationCopy("fr")), "Téléchargement de SCNet-large…");
  assert.equal(modelInstallationMessage(progress, modelInstallationCopy("en")), "Downloading SCNet-large…");
});

test("download byte labels are bounded and readable", () => {
  assert.equal(formatInstallBytes(-1), "0 Mo");
  assert.equal(formatInstallBytes(10 * 1024 * 1024), "10 Mo");
});
