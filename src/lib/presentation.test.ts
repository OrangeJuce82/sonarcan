import assert from "node:assert/strict";
import test from "node:test";

import {
  buildProjectPath,
  calculateBeatLines,
  formatPitch,
  formatTime,
  formatTimePrecise,
  visiblePeaks,
} from "./presentation.ts";

test("buildProjectPath preserves absolute and Windows path navigation", () => {
  assert.deepEqual(buildProjectPath("/Music/Set.sac"), [
    { label: "Music", path: "/Music" },
    { label: "Set.sac", path: "/Music/Set.sac" },
  ]);
  assert.deepEqual(buildProjectPath("C:\\Music\\Set.sac").at(-1), {
    label: "Set.sac",
    path: "C:/Music/Set.sac",
  });
});

test("time and pitch formatters handle boundaries", () => {
  assert.equal(formatTime(65.9), "01:05");
  assert.equal(formatTime(Number.NaN), "00:00");
  assert.equal(formatTimePrecise(65.125), "01:05.125");
  assert.equal(formatPitch(0.25), "+25 ct");
  assert.equal(formatPitch(-2), "-2.00 st");
});

test("visiblePeaks keeps extrema while reducing the sample count", () => {
  const peaks = [
    { min: -0.2, max: 0.3 },
    { min: -0.8, max: 0.4 },
    { min: -0.1, max: 0.9 },
    { min: -0.4, max: 0.2 },
  ];
  assert.deepEqual(visiblePeaks(peaks, 1, 0, 2), [
    { min: -0.8, max: 0.4 },
    { min: -0.4, max: 0.9 },
  ]);
});

test("calculateBeatLines bounds density and accents every fourth beat", () => {
  const lines = calculateBeatLines({
    bpm: 120,
    durationSeconds: 2,
    offsetSeconds: 0,
    detailed: false,
    zoom: 1,
    start: 0,
  });
  assert.deepEqual(lines, [
    { percent: 0, accent: true },
    { percent: 25, accent: false },
    { percent: 50, accent: false },
    { percent: 75, accent: false },
    { percent: 100, accent: true },
  ]);
  assert.equal(calculateBeatLines({
    bpm: 300,
    durationSeconds: 1_000,
    offsetSeconds: 0,
    detailed: false,
    zoom: 1,
    start: 0,
  }).length, 500);
});
