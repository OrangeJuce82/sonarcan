import test from "node:test";
import assert from "node:assert/strict";
import { chordSegmentsForJams, jamsChordLabel } from "./chordExport.ts";

test("converts displayed chord symbols to JAMS chord values", () => {
  assert.equal(jamsChordLabel("Cm7"), "C:min7");
  assert.equal(jamsChordLabel("Dbmaj7/Ab"), "Db:maj7/5");
  assert.equal(jamsChordLabel("N"), "N");
  assert.equal(jamsChordLabel("F#:sus4"), "F#:sus4");
});

test("exports bounded observation fields", () => {
  assert.deepEqual(chordSegmentsForJams([{ label: "Am", startSeconds: 2, endSeconds: 5, strength: 1.2 }]), [
    { time: 2, duration: 3, value: "A:min", confidence: 1 },
  ]);
});
