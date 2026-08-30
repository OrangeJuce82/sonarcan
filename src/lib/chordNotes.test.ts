import assert from "node:assert/strict";
import test from "node:test";
import { parseChordLabel } from "./chordNotes.ts";

test("the keyboard spells the supported triads and extensions", () => {
  assert.deepEqual(parseChordLabel("Dm")?.pitchNames, ["D", "F", "A"]);
  assert.deepEqual(parseChordLabel("Cmaj7")?.pitchNames, ["C", "E", "G", "B"]);
  assert.deepEqual(parseChordLabel("F#7b5")?.pitchNames, ["F#", "A#", "C", "E"]);
  assert.deepEqual(parseChordLabel("Cm9")?.pitchNames, ["C", "D", "D#", "G", "A#"]);
  assert.deepEqual(parseChordLabel("Gsus4(b7)")?.pitchNames, ["G", "C", "D", "F"]);
});

test("the keyboard keeps a slash bass distinct from the chord root", () => {
  const chord = parseChordLabel("D7/F#");
  assert.equal(chord?.root, 2);
  assert.equal(chord?.bass, 6);
  assert.deepEqual(chord?.pitchNames, ["D", "F#", "A", "C"]);
});

test("the keyboard accepts flats and rejects unknown or absent chords", () => {
  assert.deepEqual(parseChordLabel("Bbmaj7")?.pitchNames, ["A#", "D", "F", "A"]);
  assert.equal(parseChordLabel("N"), null);
  assert.equal(parseChordLabel("Cadd13"), null);
});
