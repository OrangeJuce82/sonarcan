import assert from "node:assert/strict";
import test from "node:test";
import { INSTRUMENTS, instrumentVoicings } from "./instrumentVoicings.ts";
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
  const chord = parseChordLabel("D:7/3");
  const voicing = instrumentVoicings("D:7/3", "guitar")[0];
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
