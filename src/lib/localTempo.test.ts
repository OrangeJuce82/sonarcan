import assert from "node:assert/strict";
import test from "node:test";

import { localBpmAt } from "./localTempo.ts";

test("local BPM follows the beat intervals around the playhead", () => {
  const beats = [0, 0.5, 1, 1.5, 2, 2.6, 3.2, 3.8, 4.4, 5];
  assert.ok(Math.abs(localBpmAt(beats, 1)! - 120) < 3);
  assert.ok(Math.abs(localBpmAt(beats, 4.5)! - 100) < 3);
});

test("local BPM reflects playback speed and rejects a missing-beat interval", () => {
  assert.equal(localBpmAt([0, 0.5, 1, 2, 2.5, 3], 1.5, 0.8), 96);
  assert.equal(localBpmAt([0], 0), null);
});

test("local BPM stays fixed between two beats despite timing jitter", () => {
  const beats = [0, 0.49, 1.01, 1.5, 2.02, 2.5, 3.01];
  assert.equal(localBpmAt(beats, 1.51), localBpmAt(beats, 1.99));
});

test("frame-quantized regular beats do not alternate between two tempi", () => {
  const beats = [0];
  for (let index = 0; index < 24; index += 1) {
    beats.push(beats.at(-1)! + (index % 2 === 0 ? 0.56 : 0.58));
  }

  const displayed = [3, 4, 5, 6, 7, 8]
    .map((seconds) => localBpmAt(beats, seconds)!.toFixed(1));
  assert.deepEqual([...new Set(displayed)], ["105.3"]);
});
