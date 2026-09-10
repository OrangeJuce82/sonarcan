import type { TrackMarker } from "./types";

export interface MarkerViewportBlock {
  marker: TrackMarker;
  index: number;
  startSeconds: number;
  endSeconds: number;
  leftPercent: number;
  widthPercent: number;
}

export const sortedMarkers = (markers: readonly TrackMarker[]): TrackMarker[] =>
  [...markers].sort((left, right) => left.startSeconds - right.startSeconds || left.id.localeCompare(right.id));

export function markerEndSeconds(markers: readonly TrackMarker[], index: number, durationSeconds: number): number {
  const marker = markers[index];
  if (!marker) return 0;
  const nextStart = markers[index + 1]?.startSeconds ?? durationSeconds;
  const requestedEnd = marker.endSeconds ?? nextStart;
  return Math.max(marker.startSeconds, Math.min(durationSeconds, nextStart, requestedEnd));
}

export function activeMarkerIndexAt(markers: readonly TrackMarker[], seconds: number, durationSeconds: number): number {
  const ordered = sortedMarkers(markers);
  for (let index = ordered.length - 1; index >= 0; index -= 1) {
    const marker = ordered[index];
    if (seconds >= marker.startSeconds && seconds < markerEndSeconds(ordered, index, durationSeconds)) return index;
  }
  return -1;
}

export function markerViewportBlocks(
  markers: readonly TrackMarker[],
  durationSeconds: number,
  viewportStart: number,
  viewportZoom: number,
): MarkerViewportBlock[] {
  if (!(durationSeconds > 0) || !(viewportZoom >= 1)) return [];
  const ordered = sortedMarkers(markers);
  const viewportEnd = viewportStart + 1 / viewportZoom;
  return ordered.flatMap((marker, index) => {
    const startRatio = marker.startSeconds / durationSeconds;
    const endSeconds = markerEndSeconds(ordered, index, durationSeconds);
    const endRatio = endSeconds / durationSeconds;
    if (endRatio <= viewportStart || startRatio >= viewportEnd) return [];
    const clippedStart = Math.max(viewportStart, startRatio);
    const clippedEnd = Math.min(viewportEnd, endRatio);
    return [{
      marker,
      index,
      startSeconds: marker.startSeconds,
      endSeconds,
      leftPercent: (clippedStart - viewportStart) * viewportZoom * 100,
      widthPercent: Math.max(0, (clippedEnd - clippedStart) * viewportZoom * 100),
    }];
  });
}

export function replaceMarker(markers: readonly TrackMarker[], changed: TrackMarker): TrackMarker[] {
  return sortedMarkers(markers.map((marker) => marker.id === changed.id ? changed : marker));
}

export function insertMarkerAt(markers: readonly TrackMarker[], inserted: TrackMarker, durationSeconds: number): TrackMarker[] {
  const ordered = sortedMarkers(markers);
  const shortened = ordered.map((marker, index) => (
    marker.startSeconds < inserted.startSeconds
      && markerEndSeconds(ordered, index, durationSeconds) > inserted.startSeconds
      ? { ...marker, endSeconds: inserted.startSeconds }
      : marker
  ));
  return sortedMarkers([...shortened, inserted]);
}
