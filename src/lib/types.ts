export type AudioFormat = "wav" | "mp3" | "flac";

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
