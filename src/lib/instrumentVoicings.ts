import instrumentChordCorpus from "./instrumentChordCorpus.json" with { type: "json" };
import { degreeSemitones, parseChordLabel, type ParsedChord } from "./chordNotes.ts";

export type FrettedInstrument = "guitar" | "ukulele";
export type VoicingCoverage = "exact" | "adapted";
export type VoicingSource = "corpus" | "generated";

export interface InstrumentDefinition {
  id: FrettedInstrument;
  tuning: readonly string[];
  openMidi: readonly number[];
  maximumFret: number;
}

export interface InstrumentVoicing {
  frets: number[];
  fingers: number[];
  coverage: VoicingCoverage;
  source: VoicingSource;
  omittedPitches: number[];
  bassOmitted: boolean;
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
  guitar: { id: "guitar", tuning: ["E", "A", "D", "G", "B", "E"], openMidi: [40, 45, 50, 55, 59, 64], maximumFret: 15 },
  ukulele: { id: "ukulele", tuning: ["G", "C", "E", "A"], openMidi: [67, 60, 64, 69], maximumFret: 15 },
};

const VOICING_CACHE = new Map<string, InstrumentVoicing[]>();
const SINGLE_FRET_MARKERS = new Set([3, 5, 7, 9, 15, 17, 19, 21]);

export function fretMarkerCount(fret: number): 0 | 1 | 2 {
  if (fret === 12 || fret === 24) return 2;
  return SINGLE_FRET_MARKERS.has(fret) ? 1 : 0;
}

export function fretboardStartFret(voicing: Pick<InstrumentVoicing, "baseFret"> | null): number {
  return voicing && voicing.baseFret >= 5 ? voicing.baseFret : 1;
}

function chordCorpusKey(chord: ParsedChord, pitches = chord.pitches, bass: number | "-" = chord.bassExplicit ? chord.bass : "-"): string {
  const uniquePitches = [...new Set(pitches)].sort((left, right) => left - right);
  return `${chord.root}|${uniquePitches.join(",")}|${bass}`;
}

export function instrumentPitchAt(instrument: InstrumentDefinition, string: number, fret: number): number {
  return (instrument.openMidi[string] + fret) % 12;
}

export function harmonicChordPitches(chord: ParsedChord): number[] {
  return [...new Set(chord.degrees.flatMap((degree) => {
    const interval = degreeSemitones(degree);
    return interval === null ? [] : [(chord.root + interval) % 12];
  }))];
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
  allowBassOmission = false,
): InstrumentVoicing | null {
  if (position.frets.length !== instrument.openMidi.length
    || position.fingers.length !== instrument.openMidi.length
    || position.frets.some((fret) => !Number.isInteger(fret) || fret < -1 || fret > 24)
    || position.fingers.some((finger) => !Number.isInteger(finger) || finger < 0 || finger > 4)) return null;
  const harmonicPitches = harmonicChordPitches(chord);
  const allowedPitches = allowBassOmission ? harmonicPitches : chord.pitches;
  const sounding = position.frets.flatMap((fret, string) => fret < 0 ? [] : [instrumentPitchAt(instrument, string, fret)]);
  const covered = new Set(sounding);
  const lowestPitch = lowestSoundingPitch(instrument, position.frets);
  if (covered.size < Math.min(3, harmonicPitches.length)
    || (harmonicPitches.includes(chord.root) && !covered.has(chord.root))
    || [...covered].some((pitch) => !allowedPitches.includes(pitch))
    || (chord.bassExplicit && !allowBassOmission && lowestPitch !== chord.bass)) return null;
  const omittedPitches = harmonicPitches.filter((pitch) => !covered.has(pitch));
  const bassOmitted = chord.bassExplicit && lowestPitch !== chord.bass;
  const usedFrets = position.frets.filter((fret) => fret > 0);
  const baseFret = usedFrets.length ? Math.min(...usedFrets) : 1;
  const span = usedFrets.length ? Math.max(...usedFrets) - baseFret : 0;
  return {
    frets: [...position.frets],
    fingers: [...position.fingers],
    coverage: omittedPitches.length || bassOmitted ? "adapted" : "exact",
    source: "corpus",
    omittedPitches,
    bassOmitted,
    baseFret,
    span,
    score: 10_000 - sourceOrder,
  };
}

function generatedFingers(frets: readonly number[]): number[] {
  const distinctFrets = [...new Set(frets.filter((fret) => fret > 0))].sort((left, right) => left - right);
  return frets.map((fret) => fret <= 0 ? 0 : Math.min(4, distinctFrets.indexOf(fret) + 1));
}

function omissionPenalty(chord: ParsedChord, pitch: number): number {
  const interval = (pitch - chord.root + 12) % 12;
  const degree = chord.degrees.find((candidate) => {
    const semitones = degreeSemitones(candidate);
    return semitones !== null && (semitones + 120) % 12 === interval;
  }) ?? "";
  if (interval === 0) return 1_000;
  if (["3", "b3", "7", "b7"].includes(degree)) return 180;
  if (["b5", "#5"].includes(degree)) return 130;
  if (degree === "5") return 20;
  return 90;
}

function generatedVoicing(
  frets: readonly number[],
  instrument: InstrumentDefinition,
  chord: ParsedChord,
): InstrumentVoicing | null {
  const soundingMidi = frets.flatMap((fret, string) => fret < 0 ? [] : [instrument.openMidi[string] + fret]);
  if (soundingMidi.length < Math.min(instrument.id === "guitar" ? 3 : 2, Math.max(2, harmonicChordPitches(chord).length))) return null;
  const covered = new Set(soundingMidi.map((midi) => midi % 12));
  const harmonicPitches = harmonicChordPitches(chord);
  if ((harmonicPitches.includes(chord.root) && !covered.has(chord.root))
    || [...covered].some((pitch) => !chord.pitches.includes(pitch))) return null;
  const omittedPitches = harmonicPitches.filter((pitch) => !covered.has(pitch));
  const bassOmitted = chord.bassExplicit && Math.min(...soundingMidi) % 12 !== chord.bass;
  const usedFrets = frets.filter((fret) => fret > 0);
  const baseFret = usedFrets.length ? Math.min(...usedFrets) : 1;
  const span = usedFrets.length ? Math.max(...usedFrets) - baseFret : 0;
  const mutedStrings = frets.filter((fret) => fret < 0).length;
  const openStrings = frets.filter((fret) => fret === 0).length;
  const score = 5_000
    - omittedPitches.reduce((sum, pitch) => sum + omissionPenalty(chord, pitch), 0)
    - (bassOmitted ? 500 : 0)
    - span * 35
    - Math.max(0, baseFret - 1) * 4
    - mutedStrings * 12
    + openStrings * 3;
  return {
    frets: [...frets],
    fingers: generatedFingers(frets),
    coverage: omittedPitches.length || bassOmitted ? "adapted" : "exact",
    source: "generated",
    omittedPitches,
    bassOmitted,
    baseFret,
    span,
    score,
  };
}

function generatedVoicings(chord: ParsedChord, instrument: InstrumentDefinition): InstrumentVoicing[] {
  const results = new Map<string, InstrumentVoicing>();
  const generationPitches = [...new Set(chord.pitches)]
    .sort((left, right) => {
      const leftPriority = left === chord.bass && chord.bassExplicit ? 2_000 : omissionPenalty(chord, left);
      const rightPriority = right === chord.bass && chord.bassExplicit ? 2_000 : omissionPenalty(chord, right);
      return rightPriority - leftPriority;
    })
    .slice(0, instrument.openMidi.length);
  const allowed = new Set(generationPitches);
  for (let baseFret = 1; baseFret <= instrument.maximumFret - 3; baseFret += 1) {
    const choices = instrument.openMidi.map((_, string) => {
      const values = [-1];
      if (allowed.has(instrumentPitchAt(instrument, string, 0))) values.push(0);
      for (let fret = baseFret; fret <= baseFret + 3; fret += 1) {
        if (allowed.has(instrumentPitchAt(instrument, string, fret))) values.push(fret);
      }
      return [...new Set(values)];
    });
    const frets = Array<number>(instrument.openMidi.length).fill(-1);
    const visit = (string: number): void => {
      if (string === choices.length) {
        const voicing = generatedVoicing(frets, instrument, chord);
        if (voicing) results.set(voicing.frets.join(","), voicing);
        return;
      }
      for (const fret of choices[string]) {
        frets[string] = fret;
        visit(string + 1);
      }
    };
    visit(0);
  }
  return [...results.values()];
}

function compareVoicings(left: InstrumentVoicing, right: InstrumentVoicing): number {
  if (left.coverage !== right.coverage) return left.coverage === "exact" ? -1 : 1;
  if (left.bassOmitted !== right.bassOmitted) return left.bassOmitted ? 1 : -1;
  if (left.omittedPitches.length !== right.omittedPitches.length) return left.omittedPitches.length - right.omittedPitches.length;
  if (left.source !== right.source) return left.source === "corpus" ? -1 : 1;
  return right.score - left.score || left.baseFret - right.baseFret || left.span - right.span;
}

/** Prefer validated chords-db shapes, then fill missing coverage with generated, validated voicings. */
export function instrumentVoicings(label: string, instrumentId: FrettedInstrument, maximum = 12): InstrumentVoicing[] {
  const chord = parseChordLabel(label);
  if (!chord) return [];
  const boundedMaximum = Math.max(1, maximum);
  const cacheKey = `${instrumentId}:${label}`;
  let cached = VOICING_CACHE.get(cacheKey);
  if (!cached) {
    const instrument = INSTRUMENTS[instrumentId];
    const positions = CORPUS.instruments[instrumentId][chordCorpusKey(chord)] ?? [];
    const corpusPositions = positions
      .map((position, index) => corpusVoicing(position, instrument, chord, index))
      .filter((position): position is InstrumentVoicing => position !== null);
    const harmonicPitches = harmonicChordPitches(chord);
    const baseCorpusPositions = chord.bassExplicit
      ? (CORPUS.instruments[instrumentId][chordCorpusKey(chord, harmonicPitches, "-")] ?? [])
        .map((position, index) => corpusVoicing(position, instrument, chord, positions.length + index, true))
        .filter((position): position is InstrumentVoicing => position !== null)
      : [];
    const generatedPositions = generatedVoicings(chord, instrument);
    const unique = new Map<string, InstrumentVoicing>();
    for (const voicing of [...corpusPositions, ...baseCorpusPositions, ...generatedPositions].sort(compareVoicings)) {
      if (!unique.has(voicing.frets.join(","))) unique.set(voicing.frets.join(","), voicing);
    }
    cached = [...unique.values()].sort(compareVoicings).slice(0, 48);
    VOICING_CACHE.set(cacheKey, cached);
  }
  return cached.slice(0, boundedMaximum);
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

function generatedPianoPosition(chord: ParsedChord): number[] {
  const harmonicPitches = harmonicChordPitches(chord);
  if (chord.bassExplicit) {
    return [chord.bass, ...harmonicPitches.map((pitch) => pitch + 12)];
  }
  return keyboardPositions(harmonicPitches
    .map((pitch) => (pitch - chord.root + 12) % 12)
    .sort((left, right) => left - right)
    .map((interval) => (chord.root + interval) % 12));
}

function validatedPianoCorpusPositions(
  positions: readonly PianoCorpusPosition[],
  requiredPitches: readonly number[],
  requiredBass: number | null,
): number[][] {
  return positions.flatMap((position) => {
    if (!position.pitches.length
      || position.pitches.some((pitch) => !Number.isInteger(pitch) || pitch < 0 || pitch > 11 || !requiredPitches.includes(pitch))) return [];
    const covered = new Set(position.pitches);
    if (requiredPitches.some((pitch) => !covered.has(pitch))
      || (requiredBass !== null && position.pitches[0] !== requiredBass)) return [];
    return [keyboardPositions(position.pitches)];
  });
}

function addPianoBass(position: readonly number[], bass: number): number[] {
  const octaveShift = Math.min(...position) <= bass ? 12 : 0;
  return [bass, ...position.map((pitch) => pitch + octaveShift)];
}

function pianoInversions(position: readonly number[]): number[][] {
  const unique = position.filter((pitch, index) => position.findIndex((candidate) => candidate % 12 === pitch % 12) === index);
  return unique.map((_, inversion) => unique.map((pitch, index) => (
    unique[(index + inversion) % unique.length] + (index + inversion >= unique.length ? 12 : 0)
  )));
}

/** Prefer validated chords-db piano shapes and synthesize a complete fallback. */
export function pianoVoicings(label: string): number[][] {
  const chord = parseChordLabel(label);
  if (!chord) return [];
  const harmonicPitches = harmonicChordPitches(chord);
  const positions = CORPUS.instruments.piano[chordCorpusKey(chord)] ?? [];
  const validated = validatedPianoCorpusPositions(positions, chord.pitches, chord.bassExplicit ? chord.bass : null);
  const baseVariants = chord.bassExplicit
    ? validatedPianoCorpusPositions(
      CORPUS.instruments.piano[chordCorpusKey(chord, harmonicPitches, "-")] ?? [],
      harmonicPitches,
      null,
    ).flatMap((position) => pianoInversions(position).map((variant) => addPianoBass(variant, chord.bass)))
    : [];
  const generated = generatedPianoPosition(chord);
  const unique = new Map([...validated, ...baseVariants].map((position) => [position.join(","), position]));
  if (!unique.has(generated.join(","))) unique.set(generated.join(","), generated);
  return [...unique.values()];
}
