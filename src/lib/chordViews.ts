import type { ChordAnalysis, ChordMode, TimedChord } from "./types.ts";

export type ChordColorMode = "score" | "root";

const ROOT_COLORS = ["#e76f51", "#f08c46", "#e9c46a", "#a8c957", "#52b788", "#2a9d8f", "#3a86ff", "#6574cd", "#8338ec", "#b85dd3", "#d45087", "#e05d5d"] as const;

export function chordsForMode(analysis: ChordAnalysis | null, mode: ChordMode): TimedChord[] {
  if (!analysis) return [];
  return analysis.modes[mode];
}

export function visibleChords(chords: readonly TimedChord[], minimumStrength: number): TimedChord[] {
  return chords.filter((chord) => chord.strength >= minimumStrength);
}

export function chordRepertoire(chords: readonly TimedChord[]): string[] {
  return [...new Set(chords.map((chord) => chord.label).filter((label) => label !== "N" && label !== "-"))]
    .sort((left, right) => left.localeCompare(right, "fr", { sensitivity: "base", numeric: true }));
}

export function chordDisplayLabel(label: string): string {
  return label === "N" ? "-" : label;
}

export function chordColor(label: string, strength: number, mode: ChordColorMode): string {
  if (label === "N" || label === "-") return "#7b898f";
  if (mode === "score") {
    const bounded = Math.max(0, Math.min(1, strength));
    return `hsl(${(8 + bounded * 122).toFixed(1)} 68% 58%)`;
  }
  const match = /^(?<note>[A-G])(?<accidental>#|b)?/.exec(label);
  if (!match?.groups) return "#d7a74d";
  const naturalPitch: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
  const offset = match.groups.accidental === "#" ? 1 : match.groups.accidental === "b" ? -1 : 0;
  const pitch = (naturalPitch[match.groups.note] + offset + 12) % 12;
  return ROOT_COLORS[pitch];
}
