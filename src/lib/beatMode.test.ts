import assert from "node:assert/strict";
import test from "node:test";

import { beatModeForTrack, beatTimelineFor, canToggleMetronome } from "./beatMode.ts";
import type { ChordAnalysis } from "./types.ts";

const analysis = {
  bpm: 100, beats: [1], downbeats: [1],
  dbnBpm: 102, dbnBeats: [3], dbnDownbeats: [3],
} as ChordAnalysis;

test("selects the raw or DBN Beat This! timeline", () => {
  assert.deepEqual(beatTimelineFor(analysis, { beatThisDbn: false }).beats, [1]);
  assert.deepEqual(beatTimelineFor(analysis, { beatThisDbn: true }).beats, [3]);
});

test("a track override takes precedence over the user default", () => {
  assert.equal(beatModeForTrack(undefined, true), true);
  assert.equal(beatModeForTrack(null, true), true);
  assert.equal(beatModeForTrack(false, true), false);
  assert.equal(beatModeForTrack(true, false), true);
});

test("an enabled metronome can always be switched off", () => {
  assert.equal(canToggleMetronome(true, []), true);
  assert.equal(canToggleMetronome(false, []), false);
  assert.equal(canToggleMetronome(false, [0.5]), true);
});
