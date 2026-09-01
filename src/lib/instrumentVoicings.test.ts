import assert from "node:assert/strict";
import test from "node:test";
import { CHORD_CORPUS_SOURCE, fretboardStartFret, INSTRUMENTS, instrumentVoicings, pianoVoicings } from "./instrumentVoicings.ts";
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

test("instrument positions come from the pinned published corpus", () => {
  assert.deepEqual(CHORD_CORPUS_SOURCE, {
    name: "@tombatossals/chords-db",
    revision: "df06fa7b425cf5fd29485ff6591236b3557e3fac",
    license: "MIT",
  });
  assert.deepEqual(instrumentVoicings("C", "guitar")[0]?.frets, [-1, 3, 2, 0, 1, 0]);
  assert.deepEqual(instrumentVoicings("C", "ukulele")[0]?.frets, [0, 0, 0, 3]);
  assert.deepEqual(pianoVoicings("Cmaj7")[0], [0, 4, 7, 11]);
});

test("labels absent from the published corpus never receive generated positions", () => {
  const unpublished = "C:(1,b2,2,b3,3,4,#4,5,b6,6,b7,7)";
  assert.deepEqual(instrumentVoicings(unpublished, "guitar"), []);
  assert.deepEqual(instrumentVoicings(unpublished, "ukulele"), []);
  assert.deepEqual(pianoVoicings(unpublished), []);
});
