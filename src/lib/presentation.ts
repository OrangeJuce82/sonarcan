import type { WaveformPeak } from "./types";

export interface ProjectPathPart {
  label: string;
  path: string;
}

export interface BeatLine {
  percent: number;
  accent: boolean;
}

export interface BeatLineOptions {
  bpm: number | null;
  durationSeconds: number;
  offsetSeconds: number;
  detailed: boolean;
  zoom: number;
  start: number;
}

export function buildProjectPath(packagePath: string): ProjectPathPart[] {
  const normalized = packagePath.replaceAll("\\", "/");
  const absolute = normalized.startsWith("/");
  const segments = normalized.split("/").filter(Boolean);
  let current = absolute ? "/" : "";
  return segments.map((label) => {
    current = current === "/" ? `/${label}` : current ? `${current}/${label}` : label;
    return { label, path: current };
  });
}

export function formatTime(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "00:00";
  const minutes = Math.floor(value / 60);
  const seconds = Math.floor(value % 60);
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

export function formatTimePrecise(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "00:00.000";
  const minutes = Math.floor(value / 60);
  const seconds = value % 60;
  return `${String(minutes).padStart(2, "0")}:${seconds.toFixed(3).padStart(6, "0")}`;
}

export function formatPitch(value: number): string {
  const cents = Math.round(value * 100);
  return Math.abs(value) < 1
    ? `${cents > 0 ? "+" : ""}${cents} ct`
    : `${value > 0 ? "+" : ""}${value.toFixed(2)} st`;
}

export function visiblePeaks(
  source: WaveformPeak[],
  zoom: number,
  start: number,
  maximum: number,
): WaveformPeak[] {
  if (source.length === 0) return [];
  const first = Math.floor(start * source.length);
  const count = Math.max(1, Math.ceil(source.length / zoom));
  const selection = source.slice(first, Math.min(source.length, first + count));
  const groupSize = Math.max(1, Math.ceil(selection.length / maximum));
  const result: WaveformPeak[] = [];
  for (let index = 0; index < selection.length; index += groupSize) {
    const group = selection.slice(index, index + groupSize);
    result.push({
      min: Math.min(...group.map((peak) => peak.min)),
      max: Math.max(...group.map((peak) => peak.max)),
    });
  }
  return result;
}

export function calculateBeatLines(options: BeatLineOptions): BeatLine[] {
  const { bpm, durationSeconds, offsetSeconds, detailed, zoom, start } = options;
  if (bpm === null || durationSeconds <= 0) return [];
  const period = 60 / bpm;
  const visibleStart = detailed ? start * durationSeconds : 0;
  const visibleEnd = detailed ? (start + 1 / zoom) * durationSeconds : durationSeconds;
  const firstBeat = Math.ceil((visibleStart - offsetSeconds) / period);
  const lastBeat = Math.floor((visibleEnd - offsetSeconds) / period);
  const count = Math.min(500, Math.max(0, lastBeat - firstBeat + 1));
  return Array.from({ length: count }, (_, index) => {
    const beat = firstBeat + index;
    const seconds = offsetSeconds + beat * period;
    return {
      percent: detailed
        ? (seconds / durationSeconds - start) * zoom * 100
        : seconds / durationSeconds * 100,
      accent: ((beat % 4) + 4) % 4 === 0,
    };
  });
}
