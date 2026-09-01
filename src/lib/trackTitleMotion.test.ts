import assert from "node:assert/strict";
import test from "node:test";

import { trackTitleBounceMetrics } from "./trackTitleMotion.ts";

test("playlist titles bounce only when their text overflows", () => {
  assert.equal(trackTitleBounceMetrics(160, 150), null);
  assert.equal(trackTitleBounceMetrics(160, 161), null);
  assert.deepEqual(trackTitleBounceMetrics(160, 260), {
    overflowPixels: 100,
    durationSeconds: 100 / 72,
  });
});

test("playlist title bounce remains readable for short and very long overflows", () => {
  assert.equal(trackTitleBounceMetrics(160, 170)?.durationSeconds, 1.35);
  assert.equal(trackTitleBounceMetrics(100, 900)?.durationSeconds, 5.5);
  assert.equal(trackTitleBounceMetrics(0, 900), null);
});
