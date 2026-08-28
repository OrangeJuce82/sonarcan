export type AudioFormat = "wav" | "mp3" | "flac";
export type EndBehavior = "restart" | "advance" | "stop";

export interface TrackSummary {
  id: string;
  title: string;
  sourcePath: string;
  originalSourcePath?: string | null;
  format: AudioFormat;
  fileSizeBytes: number;
  durationSeconds: number | null;
  sampleRate: number | null;
  channels: number | null;
  practice: PracticeState;
}

export interface PracticeState {
  positionSeconds: number;
  playbackRate: number;
  pitchSemitones: number;
  volume: number;
  loopEnabled: boolean;
  loopASeconds: number | null;
  loopBSeconds: number | null;
  gridBpm: number | null;
  beatGridOffsetSeconds: number;
  metronomeEnabled: boolean;
  metronomeVolume: number;
  trainerEnabled: boolean;
  trainerRepetitions: number;
  trainerIncrement: number;
  trainerTargetRate: number;
}

export interface ProjectSummary {
  name: string;
  packagePath: string;
  formatVersion: number;
  trackCount: number;
  tracks: TrackSummary[];
}

export interface DiagnosticsSnapshot {
  appVersion: string;
  os: string;
  architecture: string;
  rustLog: string;
}

export interface WaveformPeak {
  min: number;
  max: number;
}

export interface WaveformData {
  cacheVersion: number;
  trackId: string;
  durationSeconds: number;
  peaks: WaveformPeak[];
}

export interface AudioStatus {
  loaded: boolean;
  playing: boolean;
  positionSeconds: number;
  durationSeconds: number;
  outputSampleRate: number;
  outputChannels: number;
  underruns: number;
  playbackRate: number;
  pitchSemitones: number;
  gridBpm: number | null;
  beatGridOffsetSeconds: number;
  metronomeEnabled: boolean;
  metronomeVolume: number;
  trainerEnabled: boolean;
  trainerRepetitions: number;
  trainerIncrement: number;
  trainerTargetRate: number;
  trainerLoopCount: number;
  endBehavior: EndBehavior;
  endedGeneration: number;
}

export interface TempoAnalysis {
  cacheVersion: number;
  trackId: string;
  bpm: number | null;
  confidence: number;
}

export interface SpectrumFrame {
  bands: number[];
  minimumHz: number;
  maximumHz: number;
}

export type StemState = "disabled" | "ready" | "downloading" | "separating" | "failed";
export interface StemStatus { state: StemState; progress: number; stage: string; trackId: string | null; cached: boolean; error: string | null; }
export interface StemMix { gain: number; muted: boolean; soloed: boolean; }
