import assert from "node:assert/strict";
import test from "node:test";

import { activeChordIndexAt, chordColor, chordDisplayLabel, chordOccurrencesAtDownbeats, chordRepertoire, presentChordLabel, presentChordSequence, visibleChords } from "./chordViews.ts";

const chord = (label: string, strength: number) => ({ label, strength, startSeconds: 0, endSeconds: 1 });

test("the repertoire is unique, alphabetical, and excludes no-chord", () => {
  assert.deepEqual(chordRepertoire([chord("G", 1), chord("A", 1), chord("B", 1), chord("B", 1), chord("Am", 1), chord("N", 1)]), ["A", "Am", "B", "G"]);
});

test("the score filter is dynamic and does not relabel results", () => {
  assert.deepEqual(visibleChords([chord("C", 0.49), chord("Dm", 0.5)], 0.5).map(({ label }) => label), ["Dm"]);
});

test("no-chord is displayed as a neutral dash", () => {
  assert.equal(chordDisplayLabel("N"), "-");
  assert.equal(chordColor("N", 1, "score"), "var(--muted)");
});

test("root colors ignore chord quality", () => {
  assert.equal(chordColor("C", 1, "root"), chordColor("Cm", 0.2, "root"));
  assert.notEqual(chordColor("C", 1, "root"), chordColor("D", 1, "root"));
});

test("all model modes share the selected accidental spelling", () => {
  assert.equal(presentChordLabel("C#min7/G#", 0, "flat"), "Dbmin7/Ab");
  assert.equal(presentChordLabel("Dbmin7/Ab", 0, "sharp"), "C#min7/G#");
  assert.equal(presentChordLabel("N", 4, "flat"), "N");
});

test("presented chords follow the playback pitch without changing timing or confidence", () => {
  assert.deepEqual(presentChordSequence([chord("Bb", 0.8)], 2, "sharp"), [chord("C", 0.8)]);
  assert.equal(presentChordLabel("D7/F#", -2, "flat"), "C7/E");
});

test("the active chord is shown shortly before its exact boundary", () => {
  const chords = [
    { ...chord("C", 0.8), startSeconds: 10, endSeconds: 12.345 },
    { ...chord("Dm", 0.8), startSeconds: 12.345, endSeconds: 14 },
  ];

  assert.equal(activeChordIndexAt(chords, 12.334), 0);
  assert.equal(activeChordIndexAt(chords, 12.335), 1);
  assert.equal(activeChordIndexAt(chords, 12.345), 1);
});

test("downbeats preserve repeated measures without changing the chord decision", () => {
  const chords = [
    { label: "A", strength: 0.9, startSeconds: 0, endSeconds: 2 },
    { label: "G", strength: 0.8, startSeconds: 2, endSeconds: 4 },
    { label: "D", strength: 0.9, startSeconds: 4, endSeconds: 6 },
    { label: "A", strength: 0.85, startSeconds: 6, endSeconds: 10 },
    { label: "G", strength: 0.8, startSeconds: 10, endSeconds: 12 },
    { label: "D", strength: 0.9, startSeconds: 12, endSeconds: 14 },
    { label: "A", strength: 0.9, startSeconds: 14, endSeconds: 16 },
  ];

  const occurrences = chordOccurrencesAtDownbeats(chords, [0, 2, 4, 6, 8, 10, 12, 14]);
  assert.deepEqual(occurrences.map(({ label }) => label), ["A", "G", "D", "A", "A", "G", "D", "A"]);
  assert.deepEqual(occurrences.filter(({ label, startSeconds }) => label === "A" && startSeconds >= 6 && startSeconds < 10).map(({ startSeconds }) => startSeconds), [6, 8]);
  assert.equal(occurrences[4].strength, 0.85);
});

test("downbeat projection leaves raw segments untouched and works without downbeats", () => {
  const chords = [{ label: "Am", strength: 0.7, startSeconds: 1, endSeconds: 5 }];
  assert.deepEqual(chordOccurrencesAtDownbeats(chords, []), chords);
  assert.deepEqual(chordOccurrencesAtDownbeats(chords, [0, 1, 5]), chords);
  assert.deepEqual(chords, [{ label: "Am", strength: 0.7, startSeconds: 1, endSeconds: 5 }]);
});
