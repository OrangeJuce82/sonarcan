import assert from "node:assert/strict";
import test from "node:test";

import { chordColor, chordDisplayLabel, chordRepertoire, visibleChords } from "./chordViews.ts";

const chord = (label: string, strength: number) => ({ label, strength, startSeconds: 0, endSeconds: 1 });

test("the repertoire is unique, alphabetical, and excludes no-chord", () => {
  assert.deepEqual(chordRepertoire([chord("G", 1), chord("A", 1), chord("B", 1), chord("B", 1), chord("Am", 1), chord("N", 1)]), ["A", "Am", "B", "G"]);
});

test("the score filter is dynamic and does not relabel results", () => {
  assert.deepEqual(visibleChords([chord("C", 0.49), chord("Dm", 0.5)], 0.5).map(({ label }) => label), ["Dm"]);
});

test("no-chord is displayed as a neutral dash", () => {
  assert.equal(chordDisplayLabel("N"), "-");
  assert.equal(chordColor("N", 1, "score"), "#7b898f");
});

test("root colors ignore chord quality", () => {
  assert.equal(chordColor("C", 1, "root"), chordColor("Cm", 0.2, "root"));
  assert.notEqual(chordColor("C", 1, "root"), chordColor("D", 1, "root"));
});
