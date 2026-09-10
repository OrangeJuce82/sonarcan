export interface TimelineLaneItem {
  id: string;
  label: string;
  startSeconds: number;
  endSeconds: number;
  color?: string;
  active?: boolean;
  muted?: boolean;
}

export function timelineBoundaryPoints(
  ...lanes: ReadonlyArray<ReadonlyArray<Pick<TimelineLaneItem, "startSeconds" | "endSeconds">>>
): number[] {
  return [...new Set(lanes.flatMap((items) => items.flatMap((item) => [item.startSeconds, item.endSeconds])))].filter(Number.isFinite).sort((left, right) => left - right);
}

export function nearestTimelineSnapPosition(
  positionSeconds: number,
  snapPoints: readonly number[],
  toleranceSeconds: number,
): number {
  if (!Number.isFinite(positionSeconds) || !(toleranceSeconds >= 0)) return positionSeconds;
  let nearest = positionSeconds;
  let nearestDistance = toleranceSeconds;
  for (const point of snapPoints) {
    if (!Number.isFinite(point)) continue;
    const distance = Math.abs(point - positionSeconds);
    if (distance <= nearestDistance) {
      nearest = point;
      nearestDistance = distance;
    }
  }
  return nearest;
}

export function snappedTimelineRange(
  items: readonly TimelineLaneItem[],
  itemId: string,
  edge: "start" | "end",
  durationSeconds: number,
  minimumSeconds = 0.05,
): { startSeconds: number; endSeconds: number } | null {
  const ordered = [...items].sort((left, right) => left.startSeconds - right.startSeconds || left.id.localeCompare(right.id));
  const index = ordered.findIndex((item) => item.id === itemId);
  const item = ordered[index];
  if (!item) return null;
  if (edge === "start") {
    const startSeconds = ordered[index - 1]?.endSeconds ?? 0;
    return startSeconds <= item.endSeconds - minimumSeconds
      ? { startSeconds, endSeconds: item.endSeconds }
      : null;
  }
  const endSeconds = ordered[index + 1]?.startSeconds ?? durationSeconds;
  return endSeconds >= item.startSeconds + minimumSeconds
    ? { startSeconds: item.startSeconds, endSeconds }
    : null;
}
