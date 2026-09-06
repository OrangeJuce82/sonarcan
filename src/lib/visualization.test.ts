import assert from "node:assert/strict";
import test from "node:test";
import { decibelsFromLevel, emptyMeterState, linePath, meterPeakHoldMilliseconds, smoothValues, spectrumRangeSlice, updateMeterState, visualizationKinds } from "./visualization.ts";

test("only the three supported visualizations are offered", () => {
  assert.deepEqual(visualizationKinds, ["spectrum", "meter", "energy"]);
});

test("visualization smoothing stays bounded and follows the selected response", () => {
  assert.deepEqual(smoothValues([0], [1], "fast"), [0.72]);
  assert.deepEqual(smoothValues([0], [1], "smooth"), [0.18]);
  assert.ok(Math.abs(smoothValues([2], [-1], "normal")[0] - 0.86) < 1e-9);
});

test("spectrum ranges expose bounded portions of the logarithmic bands", () => {
  const bands = Array.from({ length: 64 }, (_, index) => index);
  assert.deepEqual(spectrumRangeSlice(bands, "full"), bands);
  assert.equal(spectrumRangeSlice(bands, "low")[0], 0);
  assert.ok(spectrumRangeSlice(bands, "mid")[0] > 0);
  assert.equal(spectrumRangeSlice(bands, "high").at(-1), 63);
});

test("meter helpers bound silence and expose the configured hold duration", () => {
  assert.equal(decibelsFromLevel(0), -60);
  assert.equal(Math.round(decibelsFromLevel(1)), 0);
  assert.equal(meterPeakHoldMilliseconds("off"), 0);
  assert.equal(meterPeakHoldMilliseconds("oneSecond"), 1_000);
  assert.equal(meterPeakHoldMilliseconds("threeSeconds"), 3_000);
});

test("meter attacks immediately and releases according to elapsed time", () => {
  const attacked = updateMeterState(emptyMeterState(), 0.9, "normal", 1_000, 1_000);
  assert.equal(attacked.level, 0.9);
  const released = updateMeterState(attacked, 0.1, "normal", 1_000, 1_033);
  assert.ok(released.level < 0.9);
  assert.ok(released.level > 0.1);
  const irregular = updateMeterState(attacked, 0.1, "normal", 1_000, 1_099);
  const regularFirst = updateMeterState(attacked, 0.1, "normal", 1_000, 1_033);
  const regularSecond = updateMeterState(regularFirst, 0.1, "normal", 1_000, 1_066);
  const regularThird = updateMeterState(regularSecond, 0.1, "normal", 1_000, 1_099);
  assert.ok(Math.abs(irregular.level - regularThird.level) < 1e-9);
});

test("meter peak is pushed by the visible bar, holds still, then falls", () => {
  const attacked = updateMeterState(emptyMeterState(), 0.8, "normal", 1_000, 1_000);
  const held = updateMeterState(attacked, 0.2, "normal", 1_000, 1_900);
  assert.equal(held.heldPeak, 0.8);
  const falling = updateMeterState(held, 0.2, "normal", 1_000, 2_033);
  assert.ok(falling.heldPeak < 0.8);
  assert.ok(falling.heldPeak >= falling.level);
  const pushed = updateMeterState(falling, 0.95, "normal", 1_000, 2_066);
  assert.equal(pushed.level, 0.95);
  assert.equal(pushed.heldPeak, 0.95);
});

test("line paths are deterministic and tolerate empty histories", () => {
  assert.equal(linePath([]), "");
  assert.equal(linePath([0, 1]), "M0.00,100.00 L100.00,0.00");
});
