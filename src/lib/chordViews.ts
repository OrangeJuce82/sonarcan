import type { ChordAnalysis, ChordMode, TimedChord } from "./types.ts";

export type ChordColorMode = "score" | "root";
export type ChordAccidentalMode = "flat" | "sharp";

const ROOT_COLORS = Array.from({ length: 12 }, (_, pitch) => `var(--chord-tone-${pitch})`);
const SHARP_PITCHES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;
const FLAT_PITCHES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"] as const;
const NATURAL_PITCHES: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };

export function chordsForMode(analysis: ChordAnalysis | null, mode: ChordMode): TimedChord[] {
  if (!analysis) return [];
  return analysis.modes[mode];
}

export function visibleChords(chords: readonly TimedChord[], minimumStrength: number): TimedChord[] {
  return chords.filter((chord) => chord.strength >= minimumStrength);
}

function pitchClass(note: string): number | null {
  const match = /^(?<note>[A-G])(?<accidental>#|b)?$/.exec(note);
  if (!match?.groups) return null;
  const natural = NATURAL_PITCHES[match.groups.note];
  if (natural === undefined) return null;
  const accidental = match.groups.accidental === "#" ? 1 : match.groups.accidental === "b" ? -1 : 0;
  return (natural + accidental + 12) % 12;
}

export function presentChordLabel(label: string, transposition: number, accidentals: ChordAccidentalMode): string {
  if (label === "N" || label === "-") return label;
  const match = /^([A-G](?:#|b)?)([^/]*)(?:\/([A-G](?:#|b)?))?$/.exec(label);
  if (!match) return label;
  const root = pitchClass(match[1]);
  const bass = match[3] ? pitchClass(match[3]) : null;
  if (root === null || (match[3] && bass === null)) return label;
  const pitchNames = accidentals === "flat" ? FLAT_PITCHES : SHARP_PITCHES;
  const semitones = Math.round(transposition);
  const presentedRoot = pitchNames[(root + semitones + 120) % 12];
  const presentedBass = bass === null ? "" : `/${pitchNames[(bass + semitones + 120) % 12]}`;
  return `${presentedRoot}${match[2]}${presentedBass}`;
}

export function presentChordSequence(chords: readonly TimedChord[], transposition: number, accidentals: ChordAccidentalMode): TimedChord[] {
  return chords.map((chord) => ({ ...chord, label: presentChordLabel(chord.label, transposition, accidentals) }));
}

export function chordRepertoire(chords: readonly TimedChord[]): string[] {
  return [...new Set(chords.map((chord) => chord.label).filter((label) => label !== "N" && label !== "-"))]
    .sort((left, right) => left.localeCompare(right, "fr", { sensitivity: "base", numeric: true }));
}

export function chordDisplayLabel(label: string): string {
  return label === "N" ? "-" : label;
}

export function chordColor(label: string, strength: number, mode: ChordColorMode): string {
  if (label === "N" || label === "-") return "var(--muted)";
  if (mode === "score") {
    const bounded = Math.max(0, Math.min(1, strength));
    if (bounded < 0.5) {
      return `color-mix(in srgb, var(--gold) ${(bounded * 200).toFixed(1)}%, var(--danger))`;
    }
    return `color-mix(in srgb, var(--accent) ${((bounded - 0.5) * 200).toFixed(1)}%, var(--gold))`;
  }
  const match = /^(?<note>[A-G])(?<accidental>#|b)?/.exec(label);
  if (!match?.groups) return "#d7a74d";
  const natural = NATURAL_PITCHES[match.groups.note];
  if (natural === undefined) return "var(--gold)";
  const offset = match.groups.accidental === "#" ? 1 : match.groups.accidental === "b" ? -1 : 0;
  const pitch = (natural + offset + 12) % 12;
  return ROOT_COLORS[pitch] ?? "var(--gold)";
}
