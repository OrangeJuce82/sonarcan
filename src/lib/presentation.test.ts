import assert from "node:assert/strict";
import test from "node:test";

import {
  buildProjectPath,
  calculateBeatLines,
  defaultLoopBounds,
  formatPitch,
  formatProjectHeaderPath,
  formatTime,
  formatTimePrecise,
  isMetronomeBeatActive,
  moveWaveformViewport,
  panWaveformViewportFromWheel,
  resizeWaveformViewport,
  trackLoadPosition,
  visiblePeaks,
  waveformWheelAxis,
  zoomWaveformViewport,
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

test("formatProjectHeaderPath keeps the filename separate and compacts only its directory", () => {
  const formatted = formatProjectHeaderPath("/private/var/folders/5j/cache/T/sonarcan-d4345e32-578b-4910-99cb-50370002ef99.sac");
  assert.equal(formatted.fileName, "sonarcan-d4345e32-578b-4910-99cb-50370002ef99.sac");
  assert.equal(formatted.directory, "/private/var/.../T/");
  assert.deepEqual(formatted.directoryParts.map((part) => part.label), ["private", "var", "...", "T"]);
  assert.equal(formatted.directoryParts[0]?.path, "/private");
  assert.equal(formatted.directoryParts[2]?.path, "/private/var/folders/5j/cache");
  assert.equal(formatted.directoryParts[2]?.ellipsis, true);
  assert.equal(formatted.directoryParts[3]?.path, "/private/var/folders/5j/cache/T");
  assert.equal(formatted.fullPath, "/private/var/folders/5j/cache/T/sonarcan-d4345e32-578b-4910-99cb-50370002ef99.sac");
});

test("default loop bounds cover a track without enabling loop mode", () => {
  assert.deepEqual(defaultLoopBounds(null, null, 123.5), { a: 0, b: 123.5 });
  assert.deepEqual(defaultLoopBounds(null, null, 0), { a: 0, b: null });
  assert.deepEqual(defaultLoopBounds(12, null, 123.5), { a: 12, b: null });
});

test("track loads always start at zero unless an active loop is configured to start at A", () => {
  assert.equal(trackLoadPosition(false, 12.5, "loopStart"), 0);
  assert.equal(trackLoadPosition(true, 12.5, "beginning"), 0);
  assert.equal(trackLoadPosition(true, 12.5, "loopStart"), 12.5);
  assert.equal(trackLoadPosition(true, null, "loopStart"), 0);
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

test("beat grid and metronome start at A and never pulse during its lead-in", () => {
  const lines = calculateBeatLines({
    bpm: 120,
    durationSeconds: 10,
    offsetSeconds: 2,
    detailed: false,
    zoom: 1,
    start: 0,
  });
  assert.equal(lines[0]?.percent, 20);
  assert.equal(lines[0]?.accent, true);
  assert.equal(isMetronomeBeatActive(1.5, 120, 2, 1), false);
  assert.equal(isMetronomeBeatActive(2.02, 120, 2, 1), true);
  assert.equal(isMetronomeBeatActive(2.25, 120, 2, 1), false);
});

test("waveform viewport movement preserves its span and stays in bounds", () => {
  assert.deepEqual(moveWaveformViewport(0.25, 4, 0.1), { start: 0.35, zoom: 4 });
  assert.deepEqual(moveWaveformViewport(0.75, 4, 0.5), { start: 0.75, zoom: 4 });
  assert.deepEqual(moveWaveformViewport(0.25, 4, -0.5), { start: 0, zoom: 4 });
});

test("trackpad gestures pan horizontally and lock one axis per gesture", () => {
  assert.equal(waveformWheelAxis(24, 3, null), "horizontal");
  assert.equal(waveformWheelAxis(3, 24, null), "vertical");
  assert.equal(waveformWheelAxis(2, 30, "horizontal"), "horizontal");
  assert.deepEqual(panWaveformViewportFromWheel(0.25, 4, 100, 1_000), {
    start: 0.275,
    zoom: 4,
  });
});

test("waveform viewport edges resize independently down to the minimum span", () => {
  assert.deepEqual(resizeWaveformViewport(0.25, 2, "start", 0.5), { start: 0.5, zoom: 4 });
  assert.deepEqual(resizeWaveformViewport(0.25, 2, "end", 0.5), { start: 0.25, zoom: 4 });
  const minimum = resizeWaveformViewport(0.25, 2, "start", 1);
  assert.equal(minimum.zoom, 128);
  assert.equal(minimum.start, 0.75 - 1 / 128);
});

test("waveform wheel zoom keeps an in-viewport anchor stable", () => {
  assert.deepEqual(zoomWaveformViewport(0.25, 2, 2, 0.5), { start: 0.375, zoom: 4 });
  assert.deepEqual(zoomWaveformViewport(0, 1, 0.1, 0.5), { start: 0, zoom: 1 });
});
