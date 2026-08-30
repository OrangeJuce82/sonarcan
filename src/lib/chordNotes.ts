import type { TimedChord } from "./types";

const PITCH_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;

const FLAT_PITCHES: Record<string, number> = {
  Cb: 11, Db: 1, Eb: 3, Fb: 4, Gb: 6, Ab: 8, Bb: 10,
};

const QUALITY_INTERVALS: Record<string, readonly number[]> = {
  "": [0, 4, 7],
  m: [0, 3, 7],
  "7": [0, 4, 7, 10],
  maj7: [0, 4, 7, 11],
  m7: [0, 3, 7, 10],
  dim: [0, 3, 6],
  m7b5: [0, 3, 6, 10],
  aug: [0, 4, 8],
  sus2: [0, 2, 7],
  sus4: [0, 5, 7],
  "6": [0, 4, 7, 9],
  m6: [0, 3, 7, 9],
  "9": [0, 2, 4, 7, 10],
  "7b5": [0, 4, 6, 10],
  "7#5": [0, 4, 8, 10],
};

export type ParsedChord = {
  root: number;
  bass: number;
  pitches: number[];
  pitchNames: string[];
};

function pitchIndex(note: string): number | null {
  if (note in FLAT_PITCHES) return FLAT_PITCHES[note];
  const index = PITCH_NAMES.indexOf(note as (typeof PITCH_NAMES)[number]);
  return index >= 0 ? index : null;
}

export function parseChordLabel(label: string): ParsedChord | null {
  if (label === "N") return null;
  const match = /^([A-G](?:#|b)?)([^/]*)(?:\/([A-G](?:#|b)?))?$/.exec(label);
  if (!match) return null;
  const root = pitchIndex(match[1]);
  const intervals = QUALITY_INTERVALS[match[2]];
  const explicitBass = match[3] ? pitchIndex(match[3]) : null;
  if (root === null || !intervals || (match[3] && explicitBass === null)) return null;
  const pitches = intervals.map((interval) => (root + interval) % 12);
  return {
    root,
    bass: explicitBass ?? root,
    pitches,
    pitchNames: pitches.map((pitch) => PITCH_NAMES[pitch]),
  };
}

export function simplifyChordLabel(label: string): string {
  if (label === "N") return label;
  const match = /^([A-G](?:#|b)?)([^/]*)(?:\/([A-G](?:#|b)?))?$/.exec(label);
  if (!match) return label;
  const [, root, quality] = match;
  let simpleQuality = "";
  if (quality === "m7b5") simpleQuality = "m7b5";
  else if (quality === "dim") simpleQuality = "dim";
  else if (quality === "aug" || quality === "7#5") simpleQuality = "aug";
  else if (quality.startsWith("m") && !quality.startsWith("maj")) simpleQuality = "m";
  return `${root}${simpleQuality}`;
}

export function presentChordSequence(chords: readonly TimedChord[], simplified: boolean): TimedChord[] {
  const result: TimedChord[] = [];
  for (const chord of chords) {
    const presented: TimedChord = {
      ...chord,
      label: simplified ? simplifyChordLabel(chord.label) : chord.label,
    };
    if (simplified) delete presented.bass;
    const previous = result.at(-1);
    if (previous?.label === presented.label && Math.abs(previous.endSeconds - presented.startSeconds) < 0.001) {
      const previousDuration = previous.endSeconds - previous.startSeconds;
      const presentedDuration = presented.endSeconds - presented.startSeconds;
      previous.endSeconds = presented.endSeconds;
      previous.strength = (previous.strength * previousDuration + presented.strength * presentedDuration)
        / Math.max(previousDuration + presentedDuration, Number.EPSILON);
    } else {
      result.push(presented);
    }
  }
  return result;
}

export const keyboardPitchNames = PITCH_NAMES;
