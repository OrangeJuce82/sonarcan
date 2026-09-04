import assert from "node:assert/strict";
import test from "node:test";
import { adjacentBeatPosition, availableNavigationModes, effectiveNavigationMode, navigationModeAvailable, navigationPosition, snappedNavigationPosition } from "./navigation.ts";
import type { TimedChord } from "./types.ts";

const chords: TimedChord[] = [
  { startSeconds: 0, endSeconds: 4, label: "C", strength: 1 },
  { startSeconds: 4, endSeconds: 8, label: "F", strength: 1 },
];

test("analysis navigation falls back to time until its data is available", () => {
  assert.equal(effectiveNavigationMode("beat", [], chords, [2]), "time");
  assert.equal(effectiveNavigationMode("chord", [1], [], [2]), "time");
  assert.equal(effectiveNavigationMode("lyrics", [1], chords, []), "time");
  assert.equal(navigationPosition("beat", 12, 1, 10, [], chords, [2]), 22);
});

test("only validated navigation modes can be selected or cycled", () => {
  assert.deepEqual(availableNavigationModes([], [], []), ["time"]);
  assert.deepEqual(availableNavigationModes([1], chords, []), ["time", "beat", "chord"]);
  assert.equal(navigationModeAvailable("lyrics", [1], chords, []), false);
  assert.equal(navigationModeAvailable("lyrics", [], [], [2]), true);
});

test("beat navigation moves to the adjacent detected beat", () => {
  const beats = [0.5, 1, 1.5];
  assert.equal(adjacentBeatPosition(beats, 1.01, -1), 0.5);
  assert.equal(adjacentBeatPosition(beats, 1.01, 1), 1.5);
});

test("the magnet follows chord mode and otherwise uses beats", () => {
  assert.equal(snappedNavigationPosition("chord", 3.7, [3.5], chords, [3.8]), 4);
  assert.equal(snappedNavigationPosition("time", 3.7, [3.5], chords, [3.8]), 3.5);
  assert.equal(snappedNavigationPosition("beat", 3.7, [3.5], chords, [3.8]), 3.5);
  assert.equal(snappedNavigationPosition("lyrics", 3.7, [3.5], chords, [2, 3.8]), 3.8);
});

test("lyrics navigation moves between synchronized line starts", () => {
  const lyrics = [1, 3, 7];
  assert.equal(navigationPosition("lyrics", 3.01, -1, 10, [], chords, lyrics), 1);
  assert.equal(navigationPosition("lyrics", 3.01, 1, 10, [], chords, lyrics), 7);
});
