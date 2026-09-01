import assert from "node:assert/strict";
import test from "node:test";
import { adjacentBeatPosition, effectiveNavigationMode, navigationPosition, snappedNavigationPosition } from "./navigation.ts";
import type { TimedChord } from "./types.ts";

const chords: TimedChord[] = [
  { startSeconds: 0, endSeconds: 4, label: "C", strength: 1 },
  { startSeconds: 4, endSeconds: 8, label: "F", strength: 1 },
];

test("analysis navigation falls back to time until its data is available", () => {
  assert.equal(effectiveNavigationMode("beat", [], chords), "time");
  assert.equal(effectiveNavigationMode("chord", [1], []), "time");
  assert.equal(navigationPosition("beat", 12, 1, 10, [], chords), 22);
});

test("beat navigation moves to the adjacent detected beat", () => {
  const beats = [0.5, 1, 1.5];
  assert.equal(adjacentBeatPosition(beats, 1.01, -1), 0.5);
  assert.equal(adjacentBeatPosition(beats, 1.01, 1), 1.5);
});

test("the magnet follows chord mode and otherwise uses beats", () => {
  assert.equal(snappedNavigationPosition("chord", 3.7, [3.5], chords), 4);
  assert.equal(snappedNavigationPosition("time", 3.7, [3.5], chords), 3.5);
  assert.equal(snappedNavigationPosition("beat", 3.7, [3.5], chords), 3.5);
});
