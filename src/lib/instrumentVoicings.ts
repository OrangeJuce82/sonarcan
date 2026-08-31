import { parseChordLabel, type ParsedChord } from "./chordNotes.ts";

export type FrettedInstrument = "guitar" | "ukulele";
export type VoicingCoverage = "exact" | "adapted";

export interface InstrumentDefinition {
  id: FrettedInstrument;
  tuning: readonly string[];
  openMidi: readonly number[];
}

export interface InstrumentVoicing {
  frets: number[];
  fingers: number[];
  coverage: VoicingCoverage;
  omittedPitches: number[];
  baseFret: number;
  span: number;
  score: number;
}

export const INSTRUMENTS: Readonly<Record<FrettedInstrument, InstrumentDefinition>> = {
  guitar: { id: "guitar", tuning: ["E", "A", "D", "G", "B", "E"], openMidi: [40, 45, 50, 55, 59, 64] },
  ukulele: { id: "ukulele", tuning: ["G", "C", "E", "A"], openMidi: [67, 60, 64, 69] },
};

function pitchAt(instrument: InstrumentDefinition, string: number, fret: number): number {
  return (instrument.openMidi[string] + fret) % 12;
}

function fingering(frets: readonly number[]): number[] {
  const used = [...new Set(frets.filter((fret) => fret > 0))].sort((left, right) => left - right);
  return frets.map((fret) => fret <= 0 ? 0 : Math.min(4, used.indexOf(fret) + 1));
}

function voicingKey(frets: readonly number[]): string {
  return frets.join(",");
}

function lowestSoundingPitch(instrument: InstrumentDefinition, frets: readonly number[]): number | null {
  let lowestMidi = Number.POSITIVE_INFINITY;
  for (let string = 0; string < frets.length; string += 1) {
    const fret = frets[string];
    if (fret >= 0) lowestMidi = Math.min(lowestMidi, instrument.openMidi[string] + fret);
  }
  return Number.isFinite(lowestMidi) ? lowestMidi % 12 : null;
}

function collectVoicing(instrument: InstrumentDefinition, chord: ParsedChord, frets: number[]): InstrumentVoicing | null {
  const sounding = frets.flatMap((fret, string) => fret < 0 ? [] : [pitchAt(instrument, string, fret)]);
  const lowestPitch = lowestSoundingPitch(instrument, frets);
  if (sounding.length < Math.min(3, chord.pitches.length) || (chord.bassExplicit && lowestPitch !== chord.bass)) return null;
  const covered = new Set(sounding);
  const omittedPitches = chord.pitches.filter((pitch) => !covered.has(pitch));
  const requiredCoverage = Math.min(chord.pitches.length, instrument.openMidi.length);
  if (covered.size < Math.min(3, requiredCoverage)) return null;
  const usedFrets = frets.filter((fret) => fret > 0);
  if (new Set(usedFrets).size > 4) return null;
  const baseFret = usedFrets.length ? Math.min(...usedFrets) : 1;
  const span = usedFrets.length ? Math.max(...usedFrets) - baseFret : 0;
  const coverage: VoicingCoverage = omittedPitches.length === 0 ? "exact" : "adapted";
  const score = (coverage === "exact" ? 1_000 : 300)
    + (lowestPitch === chord.root ? 35 : 0)
    + covered.size * 20
    - omittedPitches.length * 45
    - span * 12
    - baseFret * 2
    - frets.filter((fret) => fret < 0).length * 3;
  return { frets: [...frets], fingers: fingering(frets), coverage, omittedPitches, baseFret, span, score };
}

/**
 * Generate a bounded, validated voicing corpus from LV-Chordia pitch sets.
 * Every returned fret is playable, uses only chord tones, and honors the
 * detected slash bass. Rich chords are marked adapted when strings are scarce.
 */
export function instrumentVoicings(label: string, instrumentId: FrettedInstrument, maximum = 12): InstrumentVoicing[] {
  const chord = parseChordLabel(label);
  if (!chord) return [];
  const instrument = INSTRUMENTS[instrumentId];
  const allowed = new Set(chord.pitches);
  const candidates = new Map<string, InstrumentVoicing>();
  const fretWindows = [0, 1, 3, 5, 7, 9];
  for (const windowStart of fretWindows) {
    const windowEnd = windowStart === 0 ? 4 : windowStart + 4;
    const options = instrument.openMidi.map((_, string) => {
      const values = [-1];
      if (allowed.has(pitchAt(instrument, string, 0))) values.push(0);
      for (let fret = Math.max(1, windowStart); fret <= Math.min(12, windowEnd); fret += 1) {
        if (allowed.has(pitchAt(instrument, string, fret))) values.push(fret);
      }
      return values;
    });
    const frets = Array<number>(instrument.openMidi.length).fill(-1);
    const visit = (string: number): void => {
      if (string === frets.length) {
        const voicing = collectVoicing(instrument, chord, frets);
        if (voicing) candidates.set(voicingKey(voicing.frets), voicing);
        return;
      }
      for (const fret of options[string]) {
        frets[string] = fret;
        visit(string + 1);
      }
    };
    visit(0);
  }
  return [...candidates.values()]
    .sort((left, right) => right.score - left.score || left.baseFret - right.baseFret)
    .slice(0, Math.max(1, maximum));
}
