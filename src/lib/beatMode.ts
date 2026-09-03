import type { ChordAnalysis } from "./types";

export interface BeatMode {
  beatThisDbn: boolean;
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
  if (mode.beatThisDbn) {
    return { beats: analysis.dbnBeats, downbeats: analysis.dbnDownbeats, bpm: analysis.dbnBpm };
  }
  return { beats: analysis.beats, downbeats: analysis.downbeats, bpm: analysis.bpm };
}

export function canToggleMetronome(enabled: boolean, beats: readonly number[]): boolean {
  return enabled || beats.length > 0;
}
