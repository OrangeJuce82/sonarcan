import { invoke } from "@tauri-apps/api/core";
import type { AppLogEntry, AudioStatus, DiagnosticsSnapshot, EndBehavior, ImportCandidate, ImportJob, PracticeState, ProjectSummary, SpectrumFrame, StemStatus, TempoAnalysis, UserPreferences, WaveformData } from "./types";

const isTauri = (): boolean => "__TAURI_INTERNALS__" in window;

export async function createProject(name: string, parentDirectory: string): Promise<ProjectSummary> {
  if (!isTauri()) {
    throw new Error("Project creation requires the Tauri desktop runtime.");
  }
  return invoke<ProjectSummary>("create_project", { name, parentDirectory });
}

export async function openProject(packagePath: string): Promise<ProjectSummary> {
  if (!isTauri()) {
    throw new Error("Opening a project requires the Tauri desktop runtime.");
  }
  return invoke<ProjectSummary>("open_project", { packagePath });
}

export async function importAudio(projectPath: string, sourcePaths: string[]): Promise<ProjectSummary> {
  if (!isTauri()) {
    throw new Error("Audio import requires the Tauri desktop runtime.");
  }
  return invoke<ProjectSummary>("import_audio", { projectPath, sourcePaths });
}

export async function renameProject(packagePath: string, name: string): Promise<ProjectSummary> {
  if (!isTauri()) throw new Error("Renaming a project requires the Tauri desktop runtime.");
  return invoke<ProjectSummary>("rename_project", { packagePath, name });
}

export async function renameTrack(packagePath: string, trackId: string, name: string): Promise<ProjectSummary> {
  if (!isTauri()) throw new Error("Renaming a track requires the Tauri desktop runtime.");
  return invoke<ProjectSummary>("rename_track", { packagePath, trackId, name });
}

export async function reorderTrack(packagePath: string, trackId: string, newIndex: number): Promise<ProjectSummary> {
  if (!isTauri()) throw new Error("Reordering tracks requires the Tauri desktop runtime.");
  return invoke<ProjectSummary>("reorder_track", { packagePath, trackId, newIndex });
}

export async function exportPlaylist(packagePath: string, destination: string, format: "json" | "markdown"): Promise<void> {
  if (!isTauri()) throw new Error("Exporting a playlist requires the Tauri desktop runtime.");
  return invoke("export_playlist", { packagePath, destination, format });
}

export async function updatePracticeState(packagePath: string, trackId: string, state: PracticeState): Promise<ProjectSummary> {
  if (!isTauri()) throw new Error("Saving practice state requires the Tauri desktop runtime.");
  return invoke<ProjectSummary>("update_practice_state", { packagePath, trackId, state });
}

export async function saveProjectAs(sourcePackage: string, parentDirectory: string, name: string): Promise<ProjectSummary> {
  if (!isTauri()) throw new Error("Save As requires the Tauri desktop runtime.");
  return invoke<ProjectSummary>("save_project_as", { sourcePackage, parentDirectory, name });
}

export async function getWaveform(packagePath: string, trackId: string): Promise<WaveformData> {
  if (!isTauri()) throw new Error("Waveform generation requires the Tauri desktop runtime.");
  return invoke<WaveformData>("get_waveform", { packagePath, trackId });
}

export async function analyzeTempo(packagePath: string, trackId: string): Promise<TempoAnalysis> {
  if (!isTauri()) throw new Error("Tempo analysis requires the Tauri desktop runtime.");
  return invoke<TempoAnalysis>("analyze_tempo", { packagePath, trackId });
}

export async function listRecentProjects(): Promise<string[]> {
  if (!isTauri()) return [];
  return invoke<string[]>("list_recent_projects");
}

export async function setApplicationLanguage(language: "en" | "fr"): Promise<void> {
  if (isTauri()) await invoke("set_language", { language });
}

export const audioLoad = (packagePath: string, trackId: string): Promise<AudioStatus> => invoke("audio_load", { packagePath, trackId });
export const audioPreload = (packagePath: string, trackId: string): Promise<void> => invoke("audio_preload", { packagePath, trackId });
export const audioPlay = (): Promise<void> => invoke("audio_play");
export const audioPause = (): Promise<void> => invoke("audio_pause");
export const audioSeek = (seconds: number): Promise<void> => invoke("audio_seek", { seconds });
export const audioSetLoop = (aSeconds: number | null, bSeconds: number | null): Promise<void> => invoke("audio_set_loop", { aSeconds, bSeconds });
export const audioSetVolume = (volume: number): Promise<void> => invoke("audio_set_volume", { volume });
export const audioSetPlaybackRate = (rate: number): Promise<void> => invoke("audio_set_playback_rate", { rate });
export const audioSetPitch = (semitones: number): Promise<void> => invoke("audio_set_pitch", { semitones });
export const audioSetBeatGrid = (bpm: number | null, offsetSeconds: number): Promise<void> => invoke("audio_set_beat_grid", { bpm, offsetSeconds });
export const audioSetMetronome = (enabled: boolean, volume: number): Promise<void> => invoke("audio_set_metronome", { enabled, volume });
export const audioSetLoopTrainer = (enabled: boolean, repetitions: number, increment: number, targetRate: number): Promise<void> => invoke("audio_set_loop_trainer", { enabled, repetitions, increment, targetRate });
export const audioSetEndBehavior = (behavior: EndBehavior): Promise<void> => invoke("audio_set_end_behavior", { behavior });
export const audioSpectrum = (): Promise<SpectrumFrame> => invoke("audio_spectrum");
export const audioStatus = (): Promise<AudioStatus> => invoke("audio_status");
export const stemStart = (packagePath: string, trackId: string): Promise<void> => invoke("stem_start", { packagePath, trackId });
export const stemStatus = (): Promise<StemStatus> => invoke("stem_status");
export const stemDisable = (): Promise<void> => invoke("stem_disable");
export const stemSetMix = (index: number, gain: number, muted: boolean, soloed: boolean): Promise<void> => invoke("stem_set_mix", { index, gain, muted, soloed });
export const getPreferences = (): Promise<UserPreferences> => invoke("get_preferences");
export const savePreferences = (value: UserPreferences): Promise<UserPreferences> => invoke("save_preferences", { value });
export const analyzeImportText = (text: string): Promise<ImportCandidate[]> => invoke("analyze_import_text", { text });
export const resolveYoutubeSearch = (query: string): Promise<ImportCandidate[]> => invoke("resolve_youtube_search", { query });
export const readImportTextFiles = (paths: string[]): Promise<string> => invoke("read_import_text_files", { paths });
export const enqueueImports = (packagePath: string, inputs: string[]): Promise<ImportJob[]> => invoke("enqueue_imports", { request: { packagePath, inputs } });
export const importJobs = (): Promise<ImportJob[]> => invoke("import_jobs");
export const readClipboardText = (): Promise<string> => invoke("read_clipboard_text");
export const logsSnapshot = (): Promise<AppLogEntry[]> => invoke("logs_snapshot");
export const pushFrontendLog = (level: string, message: string): Promise<void> => invoke("push_frontend_log", { level, message });
export const revealProject = (packagePath: string): Promise<void> => invoke("reveal_project", { packagePath });
export const openExternalLink = (target: "github" | "donate"): Promise<void> => invoke("open_external_link", { target });

export async function diagnostics(): Promise<DiagnosticsSnapshot> {
  if (!isTauri()) {
    return {
      appVersion: "browser-preview",
      os: navigator.platform,
      architecture: "webview",
      rustLog: "Frontend preview mode",
    };
  }
  return invoke<DiagnosticsSnapshot>("diagnostics_snapshot");
}
