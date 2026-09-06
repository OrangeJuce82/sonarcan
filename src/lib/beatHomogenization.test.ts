import assert from "node:assert/strict";
import test from "node:test";

import { homogenizeBeatTimeline, nextBeatSubdivisionMode } from "./beatHomogenization.ts";

test("the beat subdivision button cycles through its three states", () => {
  assert.equal(nextBeatSubdivisionMode("auto"), "eighth");
  assert.equal(nextBeatSubdivisionMode("eighth"), "sixteenth");
  assert.equal(nextBeatSubdivisionMode("sixteenth"), "auto");
});

test("auto removes a sustained double-density region and preserves its downbeat phase", () => {
  const regular = [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5];
  const doubled = [5.25, 5.5, 5.75, 6, 6.25, 6.5, 6.75, 7];
  const result = homogenizeBeatTimeline([...regular, ...doubled], [0, 2, 4, 6, 7], "auto");

  assert.deepEqual(result.beats, [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4, 4.5, 5, 5.5, 6, 6.5, 7]);
  assert.deepEqual(result.downbeats, [0, 2, 4, 6, 7]);
  assert.equal(result.bpm, 120);
});

test("auto fills a sustained half-density region without inventing downbeats", () => {
  const dense = [0, 0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2];
  const sparse = [2.5, 3, 3.5, 4];
  const result = homogenizeBeatTimeline([...dense, ...sparse], [0, 1, 2, 4], "auto");

  assert.deepEqual(result.beats, [0, 0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.25, 2.5, 2.75, 3, 3.25, 3.5, 3.75, 4]);
  assert.deepEqual(result.downbeats, [0, 1, 2, 4]);
  assert.equal(result.bpm, 240);
});

test("eighth and sixteenth select the coarse and dense octave-related grids", () => {
  const mixed = [0, 0.5, 1, 1.5, 2, 2.25, 2.5, 2.75, 3, 3.25, 3.5, 3.75, 4];

  assert.deepEqual(homogenizeBeatTimeline(mixed, [0, 2, 4], "eighth").beats, [0, 0.5, 1, 1.5, 2, 2.5, 3, 3.5, 4]);
  assert.deepEqual(homogenizeBeatTimeline(mixed, [0, 2, 4], "sixteenth").beats, [0, 0.25, 0.5, 0.75, 1, 1.25, 1.5, 1.75, 2, 2.25, 2.5, 2.75, 3, 3.25, 3.5, 3.75, 4]);
});

test("isolated octave-sized intervals are not corrected", () => {
  const source = [0, 0.5, 1, 2, 2.5, 3];
  assert.deepEqual(homogenizeBeatTimeline(source, [0], "auto").beats, source);
});
