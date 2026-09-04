import type { NavigationMode, TimedChord } from "./types.ts";
import { adjacentChordTransportPosition } from "./chordViews.ts";
import { nearestDetectedBeat } from "./presentation.ts";

export function availableNavigationModes(
  beats: readonly number[],
  chords: readonly TimedChord[],
  lyrics: readonly number[],
): NavigationMode[] {
  const modes: NavigationMode[] = ["time"];
  if (beats.length) modes.push("beat");
  if (chords.length) modes.push("chord");
  if (lyrics.length) modes.push("lyrics");
  return modes;
}

export function navigationModeAvailable(
  mode: NavigationMode,
  beats: readonly number[],
  chords: readonly TimedChord[],
  lyrics: readonly number[],
): boolean {
  return availableNavigationModes(beats, chords, lyrics).includes(mode);
}

export function effectiveNavigationMode(
  preferred: NavigationMode,
  beats: readonly number[],
  chords: readonly TimedChord[],
  lyrics: readonly number[],
): NavigationMode {
  if (preferred === "beat" && !beats.length) return "time";
  if (preferred === "chord" && !chords.length) return "time";
  if (preferred === "lyrics" && !lyrics.length) return "time";
  return preferred;
}

export function adjacentBeatPosition(
  beats: readonly number[],
  positionSeconds: number,
  direction: -1 | 1,
): number {
  if (!beats.length || !Number.isFinite(positionSeconds)) return positionSeconds;
  const tolerance = 0.01;
  if (direction < 0) {
    for (let index = beats.length - 1; index >= 0; index -= 1) {
      const beat = beats[index];
      if (beat !== undefined && beat < positionSeconds - tolerance) return beat;
    }
    return beats[0] ?? positionSeconds;
  }
  for (const beat of beats) {
    if (beat > positionSeconds + tolerance) return beat;
  }
  return beats[beats.length - 1] ?? positionSeconds;
}

export function navigationPosition(
  preferred: NavigationMode,
  positionSeconds: number,
  direction: -1 | 1,
  timeSeconds: number,
  beats: readonly number[],
  chords: readonly TimedChord[],
  lyrics: readonly number[],
): number {
  const effective = effectiveNavigationMode(preferred, beats, chords, lyrics);
  if (effective === "beat") return adjacentBeatPosition(beats, positionSeconds, direction);
  if (effective === "chord") return adjacentChordTransportPosition(chords, positionSeconds, direction);
  if (effective === "lyrics") return adjacentBeatPosition(lyrics, positionSeconds, direction);
  return positionSeconds + direction * timeSeconds;
}

export function snappedNavigationPosition(
  preferred: NavigationMode,
  positionSeconds: number,
  beats: readonly number[],
  chords: readonly TimedChord[],
  lyrics: readonly number[],
): number {
  if (preferred === "chord" && chords.length) {
    const starts = chords.map((chord) => chord.startSeconds);
    let low = 0;
    let high = starts.length;
    while (low < high) {
      const middle = (low + high) >>> 1;
      if ((starts[middle] ?? 0) < positionSeconds) low = middle + 1;
      else high = middle;
    }
    const after = starts[low];
    const before = starts[low - 1];
    if (before === undefined) return after ?? positionSeconds;
    if (after === undefined) return before;
    return positionSeconds - before <= after - positionSeconds ? before : after;
  }
  if (preferred === "lyrics" && lyrics.length) return nearestPosition(positionSeconds, lyrics);
  return nearestDetectedBeat(positionSeconds, beats);
}

function nearestPosition(positionSeconds: number, positions: readonly number[]): number {
  let nearest = positions[0] ?? positionSeconds;
  let distance = Math.abs(nearest - positionSeconds);
  for (const position of positions.slice(1)) {
    const candidateDistance = Math.abs(position - positionSeconds);
    if (candidateDistance < distance) {
      nearest = position;
      distance = candidateDistance;
    }
  }
  return nearest;
}
