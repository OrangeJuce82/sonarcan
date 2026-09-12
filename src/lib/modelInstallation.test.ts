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

test("unsupported hardware receives a blocking full-feature error", () => {
  const french = modelInstallationCopy("fr", true);
  assert.equal(french.title, "Matériel incompatible");
  assert.match(french.failure, /n’a pas de mode CPU/);
  assert.match(french.failure, /bloquée avant l’ouverture/);
  const english = modelInstallationCopy("en", true);
  assert.equal(english.title, "Incompatible hardware");
  assert.match(english.failure, /has no CPU mode/);
  assert.match(english.failure, /blocked before the workspace opens/);
});

test("download byte labels are bounded and readable", () => {
  assert.equal(formatInstallBytes(-1), "0 Mo");
  assert.equal(formatInstallBytes(10 * 1024 * 1024), "10 Mo");
});
