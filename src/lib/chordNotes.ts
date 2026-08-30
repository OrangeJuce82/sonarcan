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
  mmaj7: [0, 3, 7, 11],
  dim: [0, 3, 6],
  dim7: [0, 3, 6, 9],
  m7b5: [0, 3, 6, 10],
  aug: [0, 4, 8],
  sus2: [0, 2, 7],
  sus4: [0, 5, 7],
  "6": [0, 4, 7, 9],
  m6: [0, 3, 7, 9],
  "9": [0, 2, 4, 7, 10],
  maj9: [0, 2, 4, 7, 11],
  m9: [0, 2, 3, 7, 10],
  "11": [0, 2, 4, 5, 7, 10],
  "13": [0, 2, 4, 5, 7, 9, 10],
  "sus4(b7)": [0, 5, 7, 10],
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

export const keyboardPitchNames = PITCH_NAMES;
