import type { TimedChord } from "./types.ts";

export interface JamsChordSegment {
  time: number;
  duration: number;
  value: string;
  confidence: number;
}

const qualityMap: Record<string, string> = {
  "": "maj", m: "min", "7": "7", maj7: "maj7", m7: "min7", dim: "dim",
  dim7: "dim7", m7b5: "hdim7", aug: "aug", sus2: "sus2", sus4: "sus4",
  "6": "6", m6: "min6", "9": "9", maj9: "maj9", m9: "min9", "11": "11",
  maj11: "maj11", m11: "min11", "13": "13", maj13: "maj13", m13: "min13", add9: "add9",
};
const pitchClasses: Record<string, number> = {
  C: 0, "C#": 1, Db: 1, D: 2, "D#": 3, Eb: 3, E: 4, F: 5,
  "F#": 6, Gb: 6, G: 7, "G#": 8, Ab: 8, A: 9, "A#": 10, Bb: 10, B: 11,
};
const bassIntervals = ["1", "b2", "2", "b3", "3", "4", "b5", "5", "b6", "6", "b7", "7"];

export function jamsChordLabel(label: string): string {
  if (label === "N") return "N";
  if (label.includes(":")) return label;
  const match = /^([A-G](?:#|b)?)([^/]*)?(?:\/([A-G](?:#|b)?))?$/.exec(label);
  if (!match) return label;
  const [, root, rawQuality = "", bass] = match;
  const quality = qualityMap[rawQuality] ?? rawQuality;
  let value = `${root}:${quality}`;
  if (bass && pitchClasses[root] !== undefined && pitchClasses[bass] !== undefined) {
    value += `/${bassIntervals[(pitchClasses[bass] - pitchClasses[root] + 12) % 12]}`;
  }
  return value;
}

export function chordSegmentsForJams(chords: readonly TimedChord[]): JamsChordSegment[] {
  return chords.map((chord) => ({
    time: chord.startSeconds,
    duration: Math.max(0, chord.endSeconds - chord.startSeconds),
    value: jamsChordLabel(chord.sourceLabel ?? chord.label),
    confidence: Math.max(0, Math.min(1, chord.strength)),
  }));
}
