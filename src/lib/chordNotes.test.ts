import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import { chordKeyboardPositions, parseChordLabel } from "./chordNotes.ts";

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

test("the two-octave keyboard displays every chord tone only once", () => {
  const c = parseChordLabel("C");
  const bDiminished = parseChordLabel("Bdim");
  assert.deepEqual(c && chordKeyboardPositions(c), [0, 4, 7]);
  assert.deepEqual(bDiminished && chordKeyboardPositions(bDiminished), [11, 14, 17]);
});

test("every chord template shipped with LV-Chordia is parseable", () => {
  const directory = resolve("src-tauri/resources/chord-runtime/runtime/lib/python3.12/site-packages/lv_chordia/data");
  for (const dictionary of ["ismir2017", "submission", "full"]) {
    const labels = readFileSync(resolve(directory, `${dictionary}_chord_list.txt`), "utf8").trim().split(/\r?\n/);
    for (const label of labels) {
      if (["N", "X"].includes(label)) continue;
      assert.ok(parseChordLabel(label), `${dictionary} label is unsupported: ${label}`);
    }
  }
});

test("explicit LV-Chordia omissions are not silently restored", () => {
  const rootless = parseChordLabel("C:maj(*1)");
  assert.ok(rootless);
  assert.equal(rootless.pitches.includes(rootless.root), false);
  assert.equal(rootless.bassExplicit, false);
});
