import test from "node:test";
import assert from "node:assert/strict";
import { formatStemEta, startStemEta, stemRemainingSeconds, updateStemEta } from "./stemEta.ts";

test("stem ETA switches from its prior to measured separation throughput", () => {
  const initial = startStemEta("fast", 240, 0);
  const separating = updateStemEta(initial, { stage: "separating", phaseCompleted: 0, phaseTotal: 20 }, 5_000);
  const measured = updateStemEta(separating, { stage: "separating", phaseCompleted: 4, phaseTotal: 20 }, 25_000);
  assert.equal(measured.secondsPerUnit, 5);
  assert.ok(measured.estimatedRemainingSeconds > 80);
  assert.ok((stemRemainingSeconds(measured, 0.35, 25_000) ?? 0) > 80);
});

test("stem ETA uses byte throughput while downloading a model", () => {
  const initial = startStemEta("hq", 240, 0);
  const downloading = updateStemEta(initial, { stage: "downloadingModel", phaseCompleted: 0, phaseTotal: 100 }, 1_000);
  const measured = updateStemEta(downloading, { stage: "downloadingModel", phaseCompleted: 50, phaseTotal: 100 }, 11_000);
  assert.equal(measured.secondsPerUnit, 0.2);
  assert.ok(measured.estimatedRemainingSeconds > 300);
});

test("stage transitions discard the preceding phase rate", () => {
  const initial = startStemEta("fast", 120, 0);
  const separating = updateStemEta(initial, { stage: "separating", phaseCompleted: 0, phaseTotal: 10 }, 1_000);
  const measured = updateStemEta(separating, { stage: "separating", phaseCompleted: 2, phaseTotal: 10 }, 5_000);
  const writing = updateStemEta(measured, { stage: "writingStems", phaseCompleted: 0, phaseTotal: 4 }, 6_000);
  assert.equal(writing.secondsPerUnit, null);
  assert.equal(writing.stage, "writingStems");
});

test("stem ETA formats compact durations and disappears at completion", () => {
  const state = startStemEta("hq", 240, 0);
  assert.equal(formatStemEta(80), "1m 20s");
  assert.equal(stemRemainingSeconds(state, 1, 1_000), null);
});
