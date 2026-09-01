import type { ChordAnalysis, ChordMode, TimedChord } from "./types.ts";

export type ChordColorMode = "score" | "root";
export type ChordAccidentalMode = "flat" | "sharp";

export interface ChordGridItem {
  index: number;
  left: number;
  top: number;
  width: number;
  height: number;
}

const ROOT_COLORS = Array.from({ length: 12 }, (_, pitch) => `var(--chord-tone-${pitch})`);
const SHARP_PITCHES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;
const FLAT_PITCHES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"] as const;
const NATURAL_PITCHES: Record<string, number> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };

export function chordsForMode(analysis: ChordAnalysis | null, mode: ChordMode): TimedChord[] {
  if (!analysis) return [];
  return analysis.modes[mode];
}

export function visibleChords(chords: readonly TimedChord[], minimumStrength: number): TimedChord[] {
  return chords.filter((chord) => chord.edited || chord.strength >= minimumStrength);
}

export function chordTimeline(chords: readonly TimedChord[]): TimedChord[] {
  return [...chords];
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

export function activeChordIndexAt(chords: readonly TimedChord[], positionSeconds: number, visualLeadSeconds = 0.01): number {
  const displayPosition = positionSeconds + Math.max(0, visualLeadSeconds);
  return chords.findIndex((chord) => displayPosition >= chord.startSeconds && displayPosition < chord.endSeconds);
}

export function adjacentChordPosition(
  chords: readonly TimedChord[],
  positionSeconds: number,
  direction: -1 | 1,
): number {
  if (!chords.length || !Number.isFinite(positionSeconds)) return positionSeconds;
  const navigationToleranceSeconds = 0.01;
  const starts = chords.map((chord) => chord.startSeconds);
  let low = 0;
  let high = starts.length;
  if (direction < 0) {
    while (low < high) {
      const middle = (low + high) >>> 1;
      if ((starts[middle] ?? 0) < positionSeconds - navigationToleranceSeconds) low = middle + 1;
      else high = middle;
    }
    return starts[low - 1] ?? positionSeconds;
  }
  while (low < high) {
    const middle = (low + high) >>> 1;
    if ((starts[middle] ?? 0) <= positionSeconds + navigationToleranceSeconds) low = middle + 1;
    else high = middle;
  }
  return starts[low] ?? positionSeconds;
}

export function adjacentChordTransportPosition(
  chords: readonly TimedChord[],
  positionSeconds: number,
  direction: -1 | 1,
): number {
  if (!chords.length || !Number.isFinite(positionSeconds)) return positionSeconds;
  const activeIndex = chords.findIndex((chord) => (
    positionSeconds >= chord.startSeconds - 0.01
      && positionSeconds < chord.endSeconds
  ));
  if (activeIndex >= 0) {
    const targetIndex = Math.max(0, Math.min(chords.length - 1, activeIndex + direction));
    return chords[targetIndex]?.startSeconds ?? positionSeconds;
  }
  return adjacentChordPosition(chords, positionSeconds, direction);
}

export function adjacentChordGridIndex(
  items: readonly ChordGridItem[],
  currentIndex: number,
  direction: -1 | 1,
): number {
  const current = items.find((item) => item.index === currentIndex);
  if (!current) return currentIndex;
  const currentX = current.left + current.width / 2;
  const currentY = current.top + current.height / 2;
  const candidates = items.flatMap((item) => {
    if (item.index === currentIndex) return [];
    const centerY = item.top + item.height / 2;
    const verticalDistance = direction < 0 ? currentY - centerY : centerY - currentY;
    return verticalDistance > 1 ? [{ item, verticalDistance }] : [];
  });
  if (!candidates.length) return currentIndex;
  const nearestRowDistance = Math.min(...candidates.map(({ verticalDistance }) => verticalDistance));
  return candidates
    .filter(({ verticalDistance }) => Math.abs(verticalDistance - nearestRowDistance) <= 2)
    .sort((left, right) => {
      const leftDistance = Math.abs(left.item.left + left.item.width / 2 - currentX);
      const rightDistance = Math.abs(right.item.left + right.item.width / 2 - currentX);
      return leftDistance - rightDistance || left.item.index - right.item.index;
    })[0]?.item.index ?? currentIndex;
}

export function nearestChordPosition(
  chords: readonly TimedChord[],
  positionSeconds: number,
): number {
  if (!chords.length || !Number.isFinite(positionSeconds)) return positionSeconds;
  let low = 0;
  let high = chords.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if ((chords[middle]?.startSeconds ?? 0) <= positionSeconds) low = middle + 1;
    else high = middle;
  }
  const before = chords[low - 1];
  const after = chords[low];
  if (before && positionSeconds >= before.startSeconds && positionSeconds < before.endSeconds) {
    return before.startSeconds;
  }
  if (!before) return after?.startSeconds ?? positionSeconds;
  if (!after) return before.startSeconds;
  const distanceFromBefore = Math.max(0, positionSeconds - before.endSeconds);
  const distanceFromAfter = Math.max(0, after.startSeconds - positionSeconds);
  return distanceFromBefore <= distanceFromAfter ? before.startSeconds : after.startSeconds;
}

export interface ChordViewportBlock {
  chord: TimedChord;
  index: number;
  leftPercent: number;
  widthPercent: number;
}

export function chordViewportBlocks(
  chords: readonly TimedChord[],
  durationSeconds: number,
  zoom: number,
  start: number,
): ChordViewportBlock[] {
  if (durationSeconds <= 0 || zoom < 1 || !Number.isFinite(start)) return [];
  const viewportStart = start * durationSeconds;
  const viewportEnd = (start + 1 / zoom) * durationSeconds;
  return chords.flatMap((chord, index) => {
    const visibleStart = Math.max(viewportStart, chord.startSeconds);
    const visibleEnd = Math.min(viewportEnd, chord.endSeconds);
    if (visibleEnd <= visibleStart) return [];
    return [{
      chord,
      index,
      leftPercent: (visibleStart / durationSeconds - start) * zoom * 100,
      widthPercent: (visibleEnd - visibleStart) / durationSeconds * zoom * 100,
    }];
  });
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
