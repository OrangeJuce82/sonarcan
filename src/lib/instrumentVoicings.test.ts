import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import test from "node:test";
import { chordEditOptions } from "./chordEditing.ts";
import { CHORD_CORPUS_SOURCE, fretboardStartFret, fretMarkerCount, INSTRUMENTS, instrumentVoicings, pianoVoicings } from "./instrumentVoicings.ts";
import { parseChordLabel } from "./chordNotes.ts";

function soundingPitches(instrument: keyof typeof INSTRUMENTS, frets: readonly number[]): number[] {
  return frets.flatMap((fret, string) => fret < 0 ? [] : [(INSTRUMENTS[instrument].openMidi[string] + fret) % 12]);
}

test("guitar and ukulele voicings contain only LV-Chordia chord tones", () => {
  for (const instrument of ["guitar", "ukulele"] as const) {
    const chord = parseChordLabel("C:maj7");
    const voicings = instrumentVoicings("C:maj7", instrument);
    assert.ok(chord && voicings.length > 0);
    for (const voicing of voicings) {
      assert.ok(soundingPitches(instrument, voicing.frets).every((pitch) => chord.pitches.includes(pitch)));
    }
  }
});

test("slash-chord voicings honor LV-Chordia's requested bass", () => {
  const chord = parseChordLabel("C7/G");
  const voicing = instrumentVoicings("C7/G", "guitar")[0];
  assert.ok(chord && voicing);
  const soundingMidi = voicing.frets.flatMap((fret, string) => fret < 0 ? [] : [INSTRUMENTS.guitar.openMidi[string] + fret]);
  assert.equal(Math.min(...soundingMidi) % 12, chord.bass);
});

test("rich voicings disclose unavoidable omissions", () => {
  const voicing = instrumentVoicings("C:13", "ukulele")[0];
  assert.ok(voicing);
  assert.equal(voicing.coverage, "adapted");
  assert.ok(voicing.omittedPitches.length > 0);
});

test("the fretboard stays on the nut until a shifted position is useful", () => {
  assert.equal(fretboardStartFret(null), 1);
  assert.equal(fretboardStartFret({ baseFret: 2 }), 1);
  assert.equal(fretboardStartFret({ baseFret: 4 }), 1);
  assert.equal(fretboardStartFret({ baseFret: 5 }), 5);
  assert.equal(fretboardStartFret({ baseFret: 9 }), 9);
});

test("guitar and ukulele fret markers follow the requested single and double-dot convention", () => {
  for (const fret of [3, 5, 7, 9, 15, 17, 19, 21]) assert.equal(fretMarkerCount(fret), 1);
  for (const fret of [12, 24]) assert.equal(fretMarkerCount(fret), 2);
  for (const fret of [1, 2, 4, 6, 8, 10, 11, 13, 14, 16, 18, 20, 22, 23]) assert.equal(fretMarkerCount(fret), 0);
});

test("instrument positions come from the pinned published corpus", () => {
  assert.deepEqual(CHORD_CORPUS_SOURCE, {
    name: "@tombatossals/chords-db",
    revision: "df06fa7b425cf5fd29485ff6591236b3557e3fac",
    license: "MIT",
  });
  assert.deepEqual(instrumentVoicings("C", "guitar")[0]?.frets, [-1, 3, 2, 0, 1, 0]);
  assert.equal(instrumentVoicings("C", "guitar")[0]?.source, "corpus");
  assert.deepEqual(instrumentVoicings("C", "ukulele")[0]?.frets, [0, 0, 0, 3]);
  assert.equal(instrumentVoicings("C", "ukulele")[0]?.source, "corpus");
  assert.deepEqual(pianoVoicings("Cmaj7")[0], [0, 4, 7, 11]);
});

test("labels absent from the published corpus receive generated, validated positions", () => {
  const unpublished = "C:(1,b2,2,b3,3,4,#4,5,b6,6,b7,7)";
  for (const instrument of ["guitar", "ukulele"] as const) {
    const chord = parseChordLabel(unpublished);
    const voicing = instrumentVoicings(unpublished, instrument)[0];
    assert.ok(chord && voicing);
    assert.equal(voicing.source, "generated");
    assert.ok(soundingPitches(instrument, voicing.frets).every((pitch) => chord.pitches.includes(pitch)));
  }
  assert.deepEqual(pianoVoicings(unpublished)[0], [0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11]);
});

test("piano slash chords always place the requested bass below the complete harmony", () => {
  const chord = parseChordLabel("C7/Bb");
  const generated = pianoVoicings("C7/Bb").at(-1);
  assert.ok(chord && generated);
  assert.equal(generated[0] % 12, chord.bass);
  assert.ok(generated.slice(1).every((position) => position > generated[0]));
  assert.deepEqual(new Set(generated.map((position) => position % 12)), new Set(chord.pitches));
});

test("piano slash chords reuse every validated variant of the base chord", () => {
  const chord = parseChordLabel("C/E");
  const positions = pianoVoicings("C/E");
  assert.ok(chord && positions.length > 1);
  for (const position of positions) {
    assert.equal(position[0] % 12, chord.bass);
    assert.ok(position.slice(1).every((pitch) => pitch > position[0]));
    assert.deepEqual(new Set(position.map((pitch) => pitch % 12)), new Set(chord.pitches));
  }
});

test("known invalid corpus entries are replaced by generated positions", () => {
  assert.equal(instrumentVoicings("C#11", "guitar")[0]?.source, "generated");
  assert.equal(instrumentVoicings("F11", "ukulele")[0]?.source, "generated");
});

test("every chord offered by the editor has a guitar and ukulele position", () => {
  for (const label of chordEditOptions("sharp")) {
    if (label === "N") continue;
    assert.ok(instrumentVoicings(label, "guitar", 1).length, `missing guitar position for ${label}`);
    assert.ok(instrumentVoicings(label, "ukulele", 1).length, `missing ukulele position for ${label}`);
    assert.ok(pianoVoicings(label).length, `missing piano position for ${label}`);
  }
});

test("no-chord never produces an instrument position", () => {
  assert.deepEqual(instrumentVoicings("N", "guitar"), []);
  assert.deepEqual(instrumentVoicings("N", "ukulele"), []);
  assert.deepEqual(pianoVoicings("N"), []);
});

test("every LV-Chordia Full template resolves on every instrument", () => {
  const path = resolve("src/lib/test-fixtures/lv-chordia/full_chord_list.txt");
  const labels = readFileSync(path, "utf8").trim().split(/\r?\n/).filter((label) => !["N", "X"].includes(label));
  for (const label of labels) {
    assert.ok(instrumentVoicings(label, "guitar", 1).length, `missing guitar position for ${label}`);
    assert.ok(instrumentVoicings(label, "ukulele", 1).length, `missing ukulele position for ${label}`);
    assert.ok(pianoVoicings(label).length, `missing piano position for ${label}`);
  }
});
