import type { LoopLoadPosition, WaveformPeak } from "./types";

export interface ProjectPathPart {
  label: string;
  path: string;
}

export interface ProjectHeaderPathPart extends ProjectPathPart {
  ellipsis?: boolean;
}

export interface ProjectHeaderPath {
  directory: string;
  directoryPath: string;
  directoryParts: ProjectHeaderPathPart[];
  absolute: boolean;
  fileName: string;
  fileStem: string;
  fileExtension: string;
  fullPath: string;
}

export interface BeatLine {
  percent: number;
  accent: boolean;
}

export interface WaveformViewport {
  start: number;
  zoom: number;
}

export type WaveformViewportEdge = "start" | "end";
export type WaveformWheelAxis = "horizontal" | "vertical";

export function shouldApplyAudioStatus(
  audioLoading: boolean,
  requestGeneration: number,
  currentGeneration: number,
  requestedTrackId: string,
  currentTrackId: string | undefined,
): boolean {
  return !audioLoading
    && requestGeneration === currentGeneration
    && requestedTrackId === currentTrackId;
}

export function shouldApplyAudioStatusPosition(
  requestSeekGeneration: number,
  currentSeekGeneration: number,
  seekPendingAtRequest: boolean,
  seekPendingNow: boolean,
): boolean {
  return requestSeekGeneration === currentSeekGeneration
    && !seekPendingAtRequest
    && !seekPendingNow;
}

export const WAVEFORM_MAX_ZOOM = 128;
export const DEFAULT_WAVEFORM_WINDOW_SECONDS = 30;
export const WAVEFORM_CHORD_WINDOW_SECONDS = 60;
const WAVEFORM_DOWNBEAT_ZOOM = 1.5;

export function waveformShowsDetail(
  durationSeconds: number,
  zoom: number,
  maximumVisibleSeconds = DEFAULT_WAVEFORM_WINDOW_SECONDS,
): boolean {
  return Number.isFinite(durationSeconds)
    && durationSeconds > 0
    && Number.isFinite(zoom)
    && zoom > 0
    && durationSeconds / zoom <= maximumVisibleSeconds + 0.000_001;
}

export function waveformShowsChords(durationSeconds: number, zoom: number): boolean {
  return waveformShowsDetail(durationSeconds, zoom, WAVEFORM_CHORD_WINDOW_SECONDS);
}

export function defaultLoopBounds(
  savedA: number | null,
  savedB: number | null,
  durationSeconds: number,
): { a: number | null; b: number | null } {
  if (savedA !== null || savedB !== null) return { a: savedA, b: savedB };
  return { a: 0, b: Number.isFinite(durationSeconds) && durationSeconds > 0 ? durationSeconds : null };
}

export function trackLoadPosition(
  loopEnabled: boolean,
  loopASeconds: number | null,
  preference: LoopLoadPosition,
): number {
  return loopEnabled
    && preference === "loopStart"
    && loopASeconds !== null
    && Number.isFinite(loopASeconds)
    ? Math.max(0, loopASeconds)
    : 0;
}

function clamp(value: number, minimum: number, maximum: number): number {
  return Math.max(minimum, Math.min(maximum, value));
}

function normalizedViewport(start: number, zoom: number): WaveformViewport {
  const safeZoom = clamp(Number.isFinite(zoom) ? zoom : 1, 1, WAVEFORM_MAX_ZOOM);
  const span = 1 / safeZoom;
  return {
    start: clamp(Number.isFinite(start) ? start : 0, 0, 1 - span),
    zoom: safeZoom,
  };
}

export function waveformViewportForWindow(
  durationSeconds: number,
  windowSeconds = DEFAULT_WAVEFORM_WINDOW_SECONDS,
  centerSeconds = windowSeconds / 2,
): WaveformViewport {
  if (!Number.isFinite(durationSeconds) || durationSeconds <= 0) return { start: 0, zoom: 1 };
  const safeWindow = Number.isFinite(windowSeconds) && windowSeconds > 0
    ? Math.min(durationSeconds, windowSeconds)
    : durationSeconds;
  const zoom = Math.min(WAVEFORM_MAX_ZOOM, Math.max(1, durationSeconds / safeWindow));
  const span = 1 / zoom;
  const center = clamp(Number.isFinite(centerSeconds) ? centerSeconds / durationSeconds : span / 2, 0, 1);
  return {
    start: clamp(center - span / 2, 0, 1 - span),
    zoom,
  };
}

export function moveWaveformViewport(start: number, zoom: number, delta: number): WaveformViewport {
  const current = normalizedViewport(start, zoom);
  const span = 1 / current.zoom;
  return {
    start: clamp(current.start + (Number.isFinite(delta) ? delta : 0), 0, 1 - span),
    zoom: current.zoom,
  };
}

export function waveformWheelAxis(
  deltaX: number,
  deltaY: number,
  lockedAxis: WaveformWheelAxis | null,
): WaveformWheelAxis {
  if (lockedAxis) return lockedAxis;
  return Math.abs(deltaX) > Math.abs(deltaY) ? "horizontal" : "vertical";
}

export function panWaveformViewportFromWheel(
  start: number,
  zoom: number,
  deltaX: number,
  width: number,
): WaveformViewport {
  if (!Number.isFinite(width) || width <= 0) return moveWaveformViewport(start, zoom, 0);
  return moveWaveformViewport(start, zoom, deltaX / width / zoom);
}

export function resizeWaveformViewport(
  start: number,
  zoom: number,
  edge: WaveformViewportEdge,
  position: number,
): WaveformViewport {
  const current = normalizedViewport(start, zoom);
  const minimumSpan = 1 / WAVEFORM_MAX_ZOOM;
  const end = current.start + 1 / current.zoom;
  const safePosition = Number.isFinite(position) ? position : edge === "start" ? current.start : end;
  const nextStart = edge === "start"
    ? clamp(safePosition, 0, end - minimumSpan)
    : current.start;
  const nextEnd = edge === "end"
    ? clamp(safePosition, current.start + minimumSpan, 1)
    : end;
  return { start: nextStart, zoom: 1 / (nextEnd - nextStart) };
}

export function zoomWaveformViewport(
  start: number,
  zoom: number,
  factor: number,
  anchorPosition: number,
): WaveformViewport {
  const current = normalizedViewport(start, zoom);
  const currentSpan = 1 / current.zoom;
  const anchor = clamp(Number.isFinite(anchorPosition) ? anchorPosition : current.start + currentSpan / 2, 0, 1);
  const anchorWithinViewport = anchor >= current.start && anchor <= current.start + currentSpan
    ? (anchor - current.start) / currentSpan
    : 0.5;
  const safeFactor = Number.isFinite(factor) && factor > 0 ? factor : 1;
  const nextZoom = clamp(current.zoom * safeFactor, 1, WAVEFORM_MAX_ZOOM);
  const nextSpan = 1 / nextZoom;
  return {
    start: clamp(anchor - anchorWithinViewport * nextSpan, 0, 1 - nextSpan),
    zoom: nextZoom,
  };
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

/** Split a project package path and compact only the directory portion. */
export function formatProjectHeaderPath(packagePath: string, targetDirectoryLength = 20): ProjectHeaderPath {
  const fullPath = packagePath.replaceAll("\\", "/");
  const separator = fullPath.lastIndexOf("/");
  const fileName = separator >= 0 ? fullPath.slice(separator + 1) : fullPath;
  const extensionStart = fileName.lastIndexOf(".");
  const fileExtension = extensionStart > 0 ? fileName.slice(extensionStart) : "";
  const fileStem = fileExtension ? fileName.slice(0, extensionStart) : fileName;
  const directory = separator >= 0 ? fullPath.slice(0, separator + 1) : "";
  const absolute = directory.startsWith("/");
  const segments = directory.split("/").filter(Boolean);
  const fullDirectoryParts = buildProjectPath(directory.replace(/\/$/, ""));
  if (!directory || directory.length <= targetDirectoryLength || segments.length < 3) {
    return { directory, directoryPath: directory, directoryParts: fullDirectoryParts, absolute, fileName, fileStem, fileExtension, fullPath };
  }
  const suffix = `/${segments.at(-1)}/`;
  const budget = Math.max(1, targetDirectoryLength - suffix.length - 5);
  let prefix = absolute ? `/${segments[0]}` : segments[0];
  let prefixCount = 1;
  for (const segment of segments.slice(1, -1)) {
    const candidate = `${prefix}/${segment}`;
    if (candidate.length > budget) break;
    prefix = candidate;
    prefixCount += 1;
  }
  const hiddenParent = fullDirectoryParts.at(-2)?.path ?? directory;
  const directoryParts: ProjectHeaderPathPart[] = [
    ...fullDirectoryParts.slice(0, prefixCount),
    { label: "...", path: hiddenParent, ellipsis: true },
    fullDirectoryParts.at(-1)!,
  ];
  return { directory: `${prefix}/...${suffix}`, directoryPath: directory, directoryParts, absolute, fileName, fileStem, fileExtension, fullPath };
}

export function formatTime(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "00:00";
  const minutes = Math.floor(value / 60);
  const seconds = Math.floor(value % 60);
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}`;
}

export function formatTimePrecise(value: number): string {
  if (!Number.isFinite(value) || value < 0) return "00:00.000";
  const totalMilliseconds = Math.round(value * 1_000);
  const minutes = Math.floor(totalMilliseconds / 60_000);
  const seconds = Math.floor(totalMilliseconds % 60_000 / 1_000);
  const milliseconds = totalMilliseconds % 1_000;
  return `${String(minutes).padStart(2, "0")}:${String(seconds).padStart(2, "0")}.${String(milliseconds).padStart(3, "0")}`;
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

export function calculateDetectedBeatLines(
  beats: readonly number[],
  downbeats: readonly number[],
  durationSeconds: number,
  detailed: boolean,
  zoom: number,
  start: number,
): BeatLine[] {
  // Keep long full-track views clean, introduce downbeats at medium zoom, then
  // reveal every beat once the viewport contains at most thirty seconds.
  const showEveryBeat = waveformShowsDetail(durationSeconds, zoom);
  if (durationSeconds <= 0
    || !beats.length
    || !detailed
    || (!showEveryBeat && zoom < WAVEFORM_DOWNBEAT_ZOOM)) return [];
  const visibleStart = detailed ? start * durationSeconds : 0;
  const visibleEnd = detailed ? (start + 1 / zoom) * durationSeconds : durationSeconds;
  const candidates = showEveryBeat ? beats : downbeats;
  const visible = candidates.filter((seconds) => seconds >= visibleStart && seconds <= visibleEnd);
  const stride = Math.max(1, Math.ceil(visible.length / 500));
  return visible.filter((_, index) => index % stride === 0).map((seconds) => ({
    percent: detailed
      ? (seconds / durationSeconds - start) * zoom * 100
      : seconds / durationSeconds * 100,
    accent: downbeats.includes(seconds),
  }));
}

export function nearestDetectedBeat(
  positionSeconds: number,
  beats: readonly number[],
): number {
  if (!beats.length || !Number.isFinite(positionSeconds)) return positionSeconds;
  let low = 0;
  let high = beats.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if ((beats[middle] ?? 0) < positionSeconds) low = middle + 1;
    else high = middle;
  }
  const after = beats[low];
  const before = beats[low - 1];
  if (before === undefined) return after ?? positionSeconds;
  if (after === undefined) return before;
  return positionSeconds - before <= after - positionSeconds ? before : after;
}

export function isDetectedBeatActive(
  positionSeconds: number,
  beats: readonly number[],
  playbackRate: number,
  pulseSeconds = 0.08,
): boolean {
  if (!beats.length || playbackRate <= 0) return false;
  let low = 0;
  let high = beats.length;
  while (low < high) {
    const middle = Math.floor((low + high) / 2);
    if (beats[middle] <= positionSeconds) low = middle + 1;
    else high = middle;
  }
  if (low === 0) return false;
  return (positionSeconds - beats[low - 1]) / playbackRate < pulseSeconds;
}
