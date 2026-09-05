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

export type LyricsProvider = "local" | "lrclib" | "musixmatch" | "lyricfind";
export type LyricsSyncLevel = "none" | "line" | "word";

export interface LyricsWord {
  text: string;
  startMs: number;
  endMs: number | null;
}

export interface LyricsLine {
  text: string;
  startMs: number | null;
  endMs: number | null;
  words: LyricsWord[];
}

export interface LyricsDocument {
  version: 1;
  provider: LyricsProvider;
  providerTrackId: string | null;
  language: string;
  syncLevel: LyricsSyncLevel;
  attribution: string | null;
  copyright: string | null;
  offsetMs: number;
  lines: LyricsLine[];
}

export interface LyricsSearchResult {
  id: number;
  trackName: string;
  artistName: string;
  albumName: string;
  durationSeconds: number;
  instrumental: boolean;
  hasSyncedLyrics: boolean;
  hasPlainLyrics: boolean;
}

export interface RemoteLyricsRecord extends LyricsSearchResult {
  syncedLyrics: string | null;
  plainLyrics: string | null;
}

export interface PracticeState {
  positionSeconds: number;
  playbackRate: number;
  pitchSemitones: number;
  volume: number;
  loopEnabled: boolean;
  loopASeconds: number | null;
  loopBSeconds: number | null;
  metronomeEnabled: boolean;
  metronomeVolume: number;
  beatThisDbn?: boolean | null;
  trainerEnabled: boolean;
  trainerStartRate: number;
  trainerRepetitions: number;
  trainerIncrement: number;
  trainerTargetRate: number;
  stemsEnabled: boolean;
  stemMix: StemMix[];
  stemNames: string[];
  chordEdits: ChordEdit[];
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

export interface AnalysisCapabilities {
  accelerated: boolean;
  backend: string | null;
  edition: "full" | "light";
  reason: "editionLight" | "acceleratorUnavailable" | null;
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
  normalizationGain: number;
  integratedLufs: number | null;
  limiterReduction: number;
  stemsEnabled: boolean;
  stemPeaks: number[];
  playbackRate: number;
  pitchSemitones: number;
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

export interface TimedChord {
  label: string;
  sourceLabel?: string;
  startSeconds: number;
  endSeconds: number;
  bass?: string;
  strength: number;
  edited?: boolean;
}

export type ChordMode = "essential" | "standard" | "complete";
export interface ChordEdit {
  mode: ChordMode;
  startSeconds: number;
  endSeconds: number;
  label: string;
}
export type MetronomeSound = "electronic" | "woodblock" | "metallic";

export interface ChordAnalysis {
  cacheVersion: number;
  trackId: string;
  modelVersion: string;
  downbeatModelVersion: string;
  bpm: number | null;
  beats: number[];
  downbeats: number[];
  dbnBpm: number | null;
  dbnBeats: number[];
  dbnDownbeats: number[];
  modes: Record<ChordMode, TimedChord[]>;
  warnings: string[];
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
export type TimeDisplay = "simple" | "precise";
export type NavigationMode = "time" | "beat" | "chord" | "lyrics";
export interface UserPreferences { theme: Theme; language: import("./i18n").Language; timeDisplay: TimeDisplay; toastDurationSeconds: number; concurrentDownloads: number; youtubeAutoSelectBestMatch: boolean; conversionFormat: ConversionFormat; sampleRate: SampleRatePreference; channels: ChannelPreference; mp3Quality: Mp3Quality; masterVolume: number; musicVolume: number; loudnessNormalization: boolean; metronomeVolume: number; metronomeSound: MetronomeSound; beatThisDbn: boolean; defaultPlaybackRate: number; defaultPitchSemitones: number; loopLoadPosition: LoopLoadPosition; loopSnapEnabled: boolean; navigationMode: NavigationMode; navigationTimeSeconds: number; degradedAnalysisNoticeSeen: boolean; lightEditionNoticeSeen: boolean; defaultTrainerStartRate: number; defaultTrainerRepetitions: number; defaultTrainerIncrement: number; defaultTrainerTargetRate: number; }
export type ImportJobState = "queued" | "downloading" | "converting" | "importing" | "completed" | "failed";
export interface ImportJob { id: string; label: string; state: ImportJobState; progress: number; error: string | null; suggestion: string | null; diagnostic: string | null; }
export interface ImportCandidate { input: string; title: string; detail: string; kind: "local" | "video" | "playlist" | "search"; matchScore?: number; thumbnailUrl?: string; videoId?: string; }
export interface AppLogEntry { timestampMs: number; origin: string; level: string; message: string; }
