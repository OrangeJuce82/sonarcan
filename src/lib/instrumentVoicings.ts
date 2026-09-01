import instrumentChordCorpus from "./instrumentChordCorpus.json" with { type: "json" };
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

interface FrettedCorpusPosition { frets: number[]; fingers: number[] }
interface PianoCorpusPosition { pitches: number[] }
interface InstrumentChordCorpus {
  source: { name: string; revision: string; license: string };
  instruments: {
    guitar: Record<string, FrettedCorpusPosition[]>;
    ukulele: Record<string, FrettedCorpusPosition[]>;
    piano: Record<string, PianoCorpusPosition[]>;
  };
}

const CORPUS: InstrumentChordCorpus = instrumentChordCorpus;
export const CHORD_CORPUS_SOURCE = CORPUS.source;

export const INSTRUMENTS: Readonly<Record<FrettedInstrument, InstrumentDefinition>> = {
  guitar: { id: "guitar", tuning: ["E", "A", "D", "G", "B", "E"], openMidi: [40, 45, 50, 55, 59, 64] },
  ukulele: { id: "ukulele", tuning: ["G", "C", "E", "A"], openMidi: [67, 60, 64, 69] },
};

export function fretboardStartFret(voicing: Pick<InstrumentVoicing, "baseFret"> | null): number {
  return voicing && voicing.baseFret >= 5 ? voicing.baseFret : 1;
}

function chordCorpusKey(chord: ParsedChord): string {
  const pitches = [...new Set(chord.pitches)].sort((left, right) => left - right);
  return `${chord.root}|${pitches.join(",")}|${chord.bassExplicit ? chord.bass : "-"}`;
}

function pitchAt(instrument: InstrumentDefinition, string: number, fret: number): number {
  return (instrument.openMidi[string] + fret) % 12;
}

function lowestSoundingPitch(instrument: InstrumentDefinition, frets: readonly number[]): number | null {
  let lowestMidi = Number.POSITIVE_INFINITY;
  for (let string = 0; string < frets.length; string += 1) {
    const fret = frets[string];
    if (fret >= 0) lowestMidi = Math.min(lowestMidi, instrument.openMidi[string] + fret);
  }
  return Number.isFinite(lowestMidi) ? lowestMidi % 12 : null;
}

function corpusVoicing(
  position: FrettedCorpusPosition,
  instrument: InstrumentDefinition,
  chord: ParsedChord,
  sourceOrder: number,
): InstrumentVoicing | null {
  if (position.frets.length !== instrument.openMidi.length
    || position.fingers.length !== instrument.openMidi.length
    || position.frets.some((fret) => !Number.isInteger(fret) || fret < -1 || fret > 24)
    || position.fingers.some((finger) => !Number.isInteger(finger) || finger < 0 || finger > 4)) return null;
  const sounding = position.frets.flatMap((fret, string) => fret < 0 ? [] : [pitchAt(instrument, string, fret)]);
  const covered = new Set(sounding);
  const lowestPitch = lowestSoundingPitch(instrument, position.frets);
  if (covered.size < Math.min(3, chord.pitches.length)
    || (chord.pitches.length <= 4 && !covered.has(chord.root))
    || [...covered].some((pitch) => !chord.pitches.includes(pitch))
    || (chord.bassExplicit && lowestPitch !== chord.bass)) return null;
  const omittedPitches = chord.pitches.filter((pitch) => !covered.has(pitch));
  const usedFrets = position.frets.filter((fret) => fret > 0);
  const baseFret = usedFrets.length ? Math.min(...usedFrets) : 1;
  const span = usedFrets.length ? Math.max(...usedFrets) - baseFret : 0;
  return {
    frets: [...position.frets],
    fingers: [...position.fingers],
    coverage: omittedPitches.length ? "adapted" : "exact",
    omittedPitches,
    baseFret,
    span,
    score: 10_000 - sourceOrder,
  };
}

/** Return only published chords from the pinned chords-db corpus. */
export function instrumentVoicings(label: string, instrumentId: FrettedInstrument, maximum = 12): InstrumentVoicing[] {
  const chord = parseChordLabel(label);
  if (!chord) return [];
  const instrument = INSTRUMENTS[instrumentId];
  const positions = CORPUS.instruments[instrumentId][chordCorpusKey(chord)] ?? [];
  return positions
    .map((position, index) => corpusVoicing(position, instrument, chord, index))
    .filter((position): position is InstrumentVoicing => position !== null)
    .slice(0, Math.max(1, maximum));
}

function keyboardPositions(pitches: readonly number[]): number[] {
  let previous = -1;
  return pitches.map((pitch) => {
    let position = pitch;
    while (position <= previous) position += 12;
    previous = position;
    return position;
  });
}

/** Return only published piano positions from the same pinned corpus. */
export function pianoVoicings(label: string): number[][] {
  const chord = parseChordLabel(label);
  if (!chord) return [];
  const positions = CORPUS.instruments.piano[chordCorpusKey(chord)] ?? [];
  return positions.flatMap((position) => {
    if (!position.pitches.length
      || position.pitches.some((pitch) => !Number.isInteger(pitch) || pitch < 0 || pitch > 11 || !chord.pitches.includes(pitch))) return [];
    const covered = new Set(position.pitches);
    if (covered.size < Math.min(3, chord.pitches.length)
      || !covered.has(chord.root)
      || (chord.bassExplicit && position.pitches[0] !== chord.bass)) return [];
    return [keyboardPositions(position.pitches)];
  });
}
