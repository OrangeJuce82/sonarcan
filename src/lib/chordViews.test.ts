import assert from "node:assert/strict";
import test from "node:test";

import { activeChordIndexAt, adjacentChordGridIndex, adjacentChordPosition, adjacentChordTransportPosition, chordColor, chordDisplayLabel, chordRepertoire, chordTimeline, chordViewportBlocks, isNoChordLabel, presentChordLabel, presentChordSequence, visibleChords } from "./chordViews.ts";

const chord = (label: string, strength: number) => ({ label, strength, startSeconds: 0, endSeconds: 1 });

test("the repertoire is unique, alphabetical, and excludes no-chord", () => {
  assert.deepEqual(chordRepertoire([chord("G", 1), chord("A", 1), chord("B", 1), chord("B", 1), chord("Am", 1), chord("N", 1)]), ["A", "Am", "B", "G"]);
});

test("the score filter is dynamic and does not relabel results", () => {
  assert.deepEqual(visibleChords([chord("C", 0.49), chord("Dm", 0.5)], 0.5).map(({ label }) => label), ["Dm"]);
  assert.deepEqual(visibleChords([{ ...chord("C", 0.1), edited: true }], 0.85).map(({ label }) => label), ["C"]);
});

test("no-chord is displayed as a neutral dash", () => {
  assert.equal(chordDisplayLabel("N"), "-");
  assert.equal(chordDisplayLabel("-"), "-");
  assert.equal(isNoChordLabel("N"), true);
  assert.equal(isNoChordLabel("-"), true);
  assert.equal(isNoChordLabel("C"), false);
  assert.equal(chordColor("N", 1, "score"), "var(--muted)");
});

test("root colors ignore chord quality", () => {
  assert.equal(chordColor("C", 1, "root"), chordColor("Cm", 0.2, "root"));
  assert.notEqual(chordColor("C", 1, "root"), chordColor("D", 1, "root"));
});

test("score colors use ten bounded semantic bands", () => {
  assert.equal(chordColor("C", -1, "score"), "var(--chord-score-0)");
  assert.equal(chordColor("C", 0.099, "score"), "var(--chord-score-0)");
  assert.equal(chordColor("C", 0.1, "score"), "var(--chord-score-1)");
  assert.equal(chordColor("C", 0.899, "score"), "var(--chord-score-8)");
  assert.equal(chordColor("C", 0.9, "score"), "var(--chord-score-9)");
  assert.equal(chordColor("C", 2, "score"), "var(--chord-score-9)");
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

test("chord navigation follows the previous and next displayed segment starts", () => {
  const chords = [
    { ...chord("C", 0.8), startSeconds: 0, endSeconds: 4 },
    { ...chord("Dm", 0.8), startSeconds: 4, endSeconds: 8 },
    { ...chord("G", 0.8), startSeconds: 8, endSeconds: 12 },
  ];
  assert.equal(adjacentChordPosition(chords, 5, -1), 4);
  assert.equal(adjacentChordPosition(chords, 4, -1), 0);
  assert.equal(adjacentChordPosition(chords, 5, 1), 8);
  assert.equal(adjacentChordPosition(chords, 4, 1), 8);
  assert.equal(adjacentChordPosition(chords, 4 - 1 / 48_000, 1), 8);
  assert.equal(adjacentChordPosition(chords, 4 + 1 / 48_000, -1), 0);
  assert.equal(adjacentChordPosition(chords, 12, 1), 12);
  assert.equal(adjacentChordPosition([], 5, -1), 5);
});

test("vertical chord navigation follows the actual responsive grid rows", () => {
  const grid = (columns: number) => Array.from({ length: 8 }, (_, index) => ({
    index,
    left: (index % columns) * 100,
    top: Math.floor(index / columns) * 50,
    width: 80,
    height: 32,
  }));
  assert.equal(adjacentChordGridIndex(grid(3), 4, -1), 1);
  assert.equal(adjacentChordGridIndex(grid(3), 4, 1), 7);
  assert.equal(adjacentChordGridIndex(grid(2), 4, -1), 2);
  assert.equal(adjacentChordGridIndex(grid(2), 4, 1), 6);
  assert.equal(adjacentChordGridIndex(grid(3), 7, 1), 7);
});

test("chord transport moves strictly between segments while playback advances", () => {
  const chords = [
    { ...chord("C", 0.8), startSeconds: 0, endSeconds: 4 },
    { ...chord("Dm", 0.8), startSeconds: 4, endSeconds: 8 },
    { ...chord("G", 0.8), startSeconds: 8, endSeconds: 12 },
  ];
  assert.equal(adjacentChordTransportPosition(chords, 4.25, -1), 0);
  assert.equal(adjacentChordTransportPosition(chords, 4.25, 1), 8);
  assert.equal(adjacentChordTransportPosition(chords, 8.03, -1), 4);
  assert.equal(adjacentChordTransportPosition(chords, 8.03, 1), 8);
});

test("chord transport does not stall on slightly overlapping model regions", () => {
  const chords = [
    { ...chord("C", 0.8), startSeconds: 0, endSeconds: 4.0008 },
    { ...chord("Dm", 0.8), startSeconds: 4, endSeconds: 8.0006 },
    { ...chord("G", 0.8), startSeconds: 8, endSeconds: 12 },
  ];

  assert.equal(adjacentChordTransportPosition(chords, 4, 1), 8);
  assert.equal(adjacentChordTransportPosition(chords, 8, -1), 4);
});

test("waveform chord blocks share its zoomed viewport and clip edge segments", () => {
  const chords = [
    { ...chord("C", 0.8), startSeconds: 0, endSeconds: 4 },
    { ...chord("Dm", 0.8), startSeconds: 4, endSeconds: 8 },
    { ...chord("G", 0.8), startSeconds: 8, endSeconds: 12 },
  ];
  assert.deepEqual(chordViewportBlocks(chords, 12, 2, 0.25).map((block) => ({
    label: block.chord.label,
    index: block.index,
    left: Math.round(block.leftPercent),
    width: Math.round(block.widthPercent),
  })), [
    { label: "C", index: 0, left: 0, width: 17 },
    { label: "Dm", index: 1, left: 17, width: 67 },
    { label: "G", index: 2, left: 83, width: 17 },
  ]);
  assert.deepEqual(chordViewportBlocks(chords, 0, 2, 0.25), []);
});

test("the chord timeline preserves the model regions without rhythmic splitting", () => {
  const chords = [
    { label: "A", strength: 0.85, startSeconds: 0, endSeconds: 8 },
    { label: "G", strength: 0.8, startSeconds: 8, endSeconds: 12 },
  ];

  assert.deepEqual(chordTimeline(chords), chords);
  assert.notEqual(chordTimeline(chords), chords);
});
