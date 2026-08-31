import assert from "node:assert/strict";
import test from "node:test";

import { applyChordEdits, chordEditKey, chordEditKeyboardAction, chordSuggestions, updateChordEdits, validateChordEntry } from "./chordEditing.ts";
import type { TimedChord } from "./types.ts";

const chords: TimedChord[] = [
  { label: "Am", sourceLabel: "A:min", startSeconds: 0, endSeconds: 2, strength: 0.8 },
  { label: "G", sourceLabel: "G:maj", startSeconds: 2, endSeconds: 4, strength: 0.7 },
  { label: "Am", sourceLabel: "A:min", startSeconds: 4, endSeconds: 6, strength: 0.9 },
];

test("chord suggestions wait for a valid root and use the common Standard corpus", () => {
  assert.deepEqual(chordSuggestions(""), []);
  assert.deepEqual(chordSuggestions("H"), []);
  assert.ok(chordSuggestions("G#").includes("G#m7"));
  assert.ok(chordSuggestions("Bbmaj").includes("Bbmaj7"));
});

test("Enter accepts a keyboard suggestion before it validates the chord", () => {
  assert.equal(chordEditKeyboardAction("Enter", true), "acceptSuggestion");
  assert.equal(chordEditKeyboardAction("Enter", false), "commit");
  assert.equal(chordEditKeyboardAction("Escape", false), "cancel");
  assert.equal(chordEditKeyboardAction("ArrowDown", false), null);
});

test("validated chord entries follow the selected accidental spelling", () => {
  assert.equal(validateChordEntry("g#", "flat"), "Ab");
  assert.equal(validateChordEntry("dbm7/ab", "sharp"), "C#m7/G#");
  assert.equal(validateChordEntry("H7", "flat"), null);
});

test("one edit changes one segment while Shift validation changes every matching chord", () => {
  const selectedKey = chordEditKey("standard", chords[0]);
  const single = updateChordEdits(chords, [], "standard", selectedKey, "D", false);
  assert.deepEqual(applyChordEdits(chords, single, "standard").map(({ label }) => label), ["D", "G", "Am"]);

  const all = updateChordEdits(chords, [], "standard", selectedKey, "D", true);
  assert.deepEqual(applyChordEdits(chords, all, "standard").map(({ label }) => label), ["D", "G", "D"]);
  assert.equal(all.length, 2);
});
