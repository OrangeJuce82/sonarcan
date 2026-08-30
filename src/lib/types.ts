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
  trainerStartRate: number;
  trainerRepetitions: number;
  trainerIncrement: number;
  trainerTargetRate: number;
  stemsEnabled: boolean;
  stemMix: StemMix[];
  stemNames: string[];
}

export interface ProjectSummary {
  name: string;
  packagePath: string;
  temporary: boolean;
  formatVersion: number;
  trackCount: number;
  tracks: TrackSummary[];
}

export interface StartupProject {
  project: ProjectSummary;
  unavailableProjectPath: string | null;
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
  outputPeak: number;
  outputPeakLeft: number;
  outputPeakRight: number;
  stemsEnabled: boolean;
  stemPeaks: number[];
  playbackRate: number;
  pitchSemitones: number;
  gridBpm: number | null;
  beatGridOffsetSeconds: number;
  metronomeEnabled: boolean;
  metronomeVolume: number;
  trainerEnabled: boolean;
  trainerStartRate: number;
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

export interface TimedChord {
  label: string;
  startSeconds: number;
  endSeconds: number;
  bass?: string;
  strength: number;
}

export type ChordMode = "essential" | "standard" | "complete";

export interface ChordAnalysis {
  cacheVersion: number;
  trackId: string;
  modelVersion: string;
  modes: Record<ChordMode, TimedChord[]>;
}

export interface SpectrumFrame {
  bands: number[];
  minimumHz: number;
  maximumHz: number;
}

export interface SystemMetrics {
  cpuPercent: number | null;
  memoryMegabytes: number | null;
}

export type StemState = "disabled" | "ready" | "separating" | "failed";
export interface StemStatus { state: StemState; enabled: boolean; progress: number; stage: string; trackId: string | null; cached: boolean; error: string | null; computeBackend: "MLX" | null; }
export interface StemMix { gain: number; pan: number; muted: boolean; soloed: boolean; }

export type Theme = "system" | "dark" | "light";
export type ConversionFormat = "keep" | "mp3" | "wav" | "flac";
export type SampleRatePreference = "preserve" | "hz44100" | "hz48000";
export type ChannelPreference = "preserve" | "stereo" | "mono";
export type Mp3Quality = "vbrHigh" | "kbps320" | "kbps256" | "kbps192";
export type LoopLoadPosition = "beginning" | "loopStart";
export interface UserPreferences { theme: Theme; language: "en" | "fr"; toastDurationSeconds: number; concurrentDownloads: number; conversionFormat: ConversionFormat; sampleRate: SampleRatePreference; channels: ChannelPreference; mp3Quality: Mp3Quality; masterVolume: number; metronomeVolume: number; defaultPlaybackRate: number; defaultPitchSemitones: number; loopLoadPosition: LoopLoadPosition; defaultTrainerStartRate: number; defaultTrainerRepetitions: number; defaultTrainerIncrement: number; defaultTrainerTargetRate: number; }
export type ImportJobState = "queued" | "downloading" | "converting" | "importing" | "completed" | "failed";
export interface ImportJob { id: string; label: string; state: ImportJobState; progress: number; error: string | null; suggestion: string | null; diagnostic: string | null; }
export interface ImportCandidate { input: string; title: string; detail: string; kind: "local" | "video" | "playlist" | "search"; }
export interface AppLogEntry { timestampMs: number; origin: string; level: string; message: string; }
