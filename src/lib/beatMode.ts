import type { ChordAnalysis } from "./types";
import { homogenizeBeatTimeline, type BeatSubdivisionMode } from "./beatHomogenization.ts";

export interface BeatMode {
  beatThisDbn: boolean;
  subdivision?: BeatSubdivisionMode;
}

export interface BeatTimeline {
  beats: number[];
  downbeats: number[];
  bpm: number | null;
}

export function beatModeForTrack(trackOverride: boolean | null | undefined, userDefault: boolean): boolean {
  return trackOverride ?? userDefault;
}

export function beatTimelineFor(analysis: ChordAnalysis | null, mode: BeatMode): BeatTimeline {
  if (!analysis) return { beats: [], downbeats: [], bpm: null };
  const source = mode.beatThisDbn
    ? { beats: analysis.dbnBeats, downbeats: analysis.dbnDownbeats, bpm: analysis.dbnBpm }
    : { beats: analysis.beats, downbeats: analysis.downbeats, bpm: analysis.bpm };
  const homogenized = homogenizeBeatTimeline(source.beats, source.downbeats, mode.subdivision ?? "auto");
  return { ...homogenized, bpm: homogenized.bpm ?? source.bpm };
}

export function canToggleMetronome(enabled: boolean, beats: readonly number[]): boolean {
  return enabled || beats.length > 0;
}
