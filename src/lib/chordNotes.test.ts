import assert from "node:assert/strict";
import test from "node:test";
import { parseChordLabel, presentChordSequence, simplifyChordLabel } from "./chordNotes.ts";

test("the keyboard spells the supported triads and extensions", () => {
  assert.deepEqual(parseChordLabel("Dm")?.pitchNames, ["D", "F", "A"]);
  assert.deepEqual(parseChordLabel("Cmaj7")?.pitchNames, ["C", "E", "G", "B"]);
  assert.deepEqual(parseChordLabel("F#7b5")?.pitchNames, ["F#", "A#", "C", "E"]);
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

test("simple mode keeps five chord families without bass-driven inversions", () => {
  assert.equal(simplifyChordLabel("Cmaj7"), "C");
  assert.equal(simplifyChordLabel("Dm9/A"), "Dm");
  assert.equal(simplifyChordLabel("F#dim"), "F#dim");
  assert.equal(simplifyChordLabel("Bm7b5"), "Bm7b5");
  assert.equal(simplifyChordLabel("G7#5/B"), "Gaug");
  assert.equal(simplifyChordLabel("Dsus2"), "D");
  assert.equal(simplifyChordLabel("N"), "N");
});

test("simple mode renames and merges the complete timed sequence", () => {
  const complete = [
    { label: "Cmaj7", startSeconds: 0, endSeconds: 1, strength: 0.8 },
    { label: "C/E", startSeconds: 1, endSeconds: 2, bass: "E", strength: 0.6 },
    { label: "Dm7", startSeconds: 2, endSeconds: 3, strength: 0.7 },
  ];
  assert.deepEqual(presentChordSequence(complete, true), [
    { label: "C", startSeconds: 0, endSeconds: 2, strength: 0.7 },
    { label: "Dm", startSeconds: 2, endSeconds: 3, strength: 0.7 },
  ]);
  assert.deepEqual(presentChordSequence(complete, false), complete);
});
