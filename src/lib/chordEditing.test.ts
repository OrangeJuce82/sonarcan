import assert from "node:assert/strict";
import test from "node:test";

import { applyChordEdits, centeredChordOptionScrollTop, chordEditKey, chordEditKeyboardAction, chordEditOptions, chordEditPointerAction, chordGridKeyboardAction, chordSuggestions, shouldSeekChordFromClick, updateChordEdits, validateChordEntry } from "./chordEditing.ts";
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

test("the visible chord editor exposes every root in the selected spelling", () => {
  const flatOptions = chordEditOptions("flat");
  const sharpOptions = chordEditOptions("sharp");
  assert.ok(flatOptions.includes("Db"));
  assert.ok(flatOptions.includes("Bbm7"));
  assert.ok(sharpOptions.includes("C#"));
  assert.ok(sharpOptions.includes("A#m7"));
  assert.equal(flatOptions[0], "N");
  assert.ok(flatOptions.length > 200);
});

test("the chord editor can center its current option when it opens", () => {
  assert.equal(centeredChordOptionScrollTop(0, 100, 140, 2_000), 0);
  assert.equal(centeredChordOptionScrollTop(50, 100, 140, 2_000), 940);
  assert.equal(centeredChordOptionScrollTop(99, 100, 140, 2_000), 1_860);
  assert.equal(centeredChordOptionScrollTop(-1, 100, 140, 2_000), 0);
});

test("Enter validates the edit and Escape cancels it", () => {
  assert.equal(chordEditKeyboardAction("Enter"), "commit");
  assert.equal(chordEditKeyboardAction("Escape"), "cancel");
  assert.equal(chordEditKeyboardAction("ArrowDown"), null);
});

test("simple mode seeks on click while edit mode preserves Alt-click seeking", () => {
  assert.equal(shouldSeekChordFromClick(false, false), true);
  assert.equal(shouldSeekChordFromClick(false, true), true);
  assert.equal(shouldSeekChordFromClick(true, false), false);
  assert.equal(shouldSeekChordFromClick(true, true), true);
});

test("simple chord navigation seeks horizontally while edit mode exposes editing keys", () => {
  assert.equal(chordGridKeyboardAction(false, "ArrowLeft"), "previous");
  assert.equal(chordGridKeyboardAction(false, "ArrowRight"), "next");
  assert.equal(chordGridKeyboardAction(false, "ArrowUp"), null);
  assert.equal(chordGridKeyboardAction(false, "Enter"), null);
  assert.equal(chordGridKeyboardAction(true, "ArrowUp"), "up");
  assert.equal(chordGridKeyboardAction(true, "ArrowDown"), "down");
  assert.equal(chordGridKeyboardAction(true, "Enter"), "beginEdit");
});

test("option clicks validate edits while outside clicks cancel them", () => {
  assert.equal(chordEditPointerAction("option", 0, false), "commit");
  assert.equal(chordEditPointerAction("option", 0, true), "commitAll");
  assert.equal(chordEditPointerAction("outside", 0, false), "cancel");
  assert.equal(chordEditPointerAction("editor", 0, false), null);
  assert.equal(chordEditPointerAction("option", 1, false), null);
});

test("validated chord entries follow the selected accidental spelling", () => {
  assert.equal(validateChordEntry("g#", "flat"), "Ab");
  assert.equal(validateChordEntry("dbm7/ab", "sharp"), "C#m7/G#");
  assert.equal(validateChordEntry("-", "flat"), "N");
  assert.equal(validateChordEntry("n", "sharp"), "N");
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

test("No chord edits preserve segment timings", () => {
  const selectedKey = chordEditKey("standard", chords[1]);
  const edits = updateChordEdits(chords, [], "standard", selectedKey, "N", false);
  const result = applyChordEdits(chords, edits, "standard");
  assert.deepEqual(result.map(({ startSeconds, endSeconds }) => [startSeconds, endSeconds]), [[0, 2], [2, 4], [4, 6]]);
  assert.equal(result[1]?.label, "N");
});
