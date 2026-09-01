export interface TrackTitleBounceMetrics {
  durationSeconds: number;
  overflowPixels: number;
}

export function trackTitleBounceMetrics(
  availableWidth: number,
  contentWidth: number,
): TrackTitleBounceMetrics | null {
  if (!Number.isFinite(availableWidth) || !Number.isFinite(contentWidth) || availableWidth <= 0) return null;
  const overflowPixels = Math.ceil(contentWidth - availableWidth);
  if (overflowPixels <= 1) return null;
  return {
    overflowPixels,
    durationSeconds: Math.min(5.5, Math.max(1.35, overflowPixels / 72)),
  };
}
