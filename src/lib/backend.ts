import { invoke } from "@tauri-apps/api/core";
import type { DiagnosticsSnapshot, ProjectSummary, WaveformData } from "./types";

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

export async function saveProjectAs(sourcePackage: string, parentDirectory: string, name: string): Promise<ProjectSummary> {
  if (!isTauri()) throw new Error("Save As requires the Tauri desktop runtime.");
  return invoke<ProjectSummary>("save_project_as", { sourcePackage, parentDirectory, name });
}

export async function getWaveform(packagePath: string, trackId: string): Promise<WaveformData> {
  if (!isTauri()) throw new Error("Waveform generation requires the Tauri desktop runtime.");
  return invoke<WaveformData>("get_waveform", { packagePath, trackId });
}

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
