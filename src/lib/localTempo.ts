const MIN_INTERVAL_SECONDS = 0.2;
const MAX_INTERVAL_SECONDS = 2;
const WINDOW_INTERVALS = 4;
const MIN_CONSISTENT_RATIO = 0.9;
const MAX_CONSISTENT_RATIO = 1.1;

function insertionIndex(values: readonly number[], target: number): number {
  let low = 0;
  let high = values.length;
  while (low < high) {
    const middle = (low + high) >>> 1;
    if (values[middle]! < target) low = middle + 1;
    else high = middle;
  }
  return low;
}

export function localBpmAt(
  beats: readonly number[],
  seconds: number,
  playbackRate = 1,
): number | null {
  if (beats.length < 2 || !Number.isFinite(seconds) || !Number.isFinite(playbackRate)) return null;
  const center = insertionIndex(beats, seconds);
  const first = Math.max(0, center - WINDOW_INTERVALS);
  const last = Math.min(beats.length - 1, center + WINDOW_INTERVALS);
  const intervals = Array.from({ length: last - first }, (_, offset) => {
    const index = first + offset;
    return beats[index + 1]! - beats[index]!;
  }).filter((interval) => interval >= MIN_INTERVAL_SECONDS && interval <= MAX_INTERVAL_SECONDS);
  if (!intervals.length) return null;

  const ordered = [...intervals].sort((left, right) => left - right);
  const median = ordered[Math.floor(ordered.length / 2)]!;
  const minimum = median * MIN_CONSISTENT_RATIO;
  const maximum = median * MAX_CONSISTENT_RATIO;
  let bestStart = first;
  let bestEnd = first;
  let runStart = first;
  for (let index = first; index < last; index += 1) {
    const interval = beats[index + 1]! - beats[index]!;
    if (interval >= minimum && interval <= maximum) continue;
    if (index - runStart > bestEnd - bestStart) [bestStart, bestEnd] = [runStart, index];
    runStart = index + 1;
  }
  if (last - runStart > bestEnd - bestStart) [bestStart, bestEnd] = [runStart, last];
  const pointCount = bestEnd - bestStart + 1;
  if (pointCount < 2) return null;

  // Estimate the slope of the local beat timeline instead of converting one
  // frame-quantized interval. Regression keeps sub-BPM precision while the
  // timestamp error is distributed across several beats.
  const meanIndex = (bestStart + bestEnd) / 2;
  let meanSeconds = 0;
  for (let index = bestStart; index <= bestEnd; index += 1) meanSeconds += beats[index]!;
  meanSeconds /= pointCount;
  let covariance = 0;
  let variance = 0;
  for (let index = bestStart; index <= bestEnd; index += 1) {
    const indexOffset = index - meanIndex;
    covariance += indexOffset * (beats[index]! - meanSeconds);
    variance += indexOffset * indexOffset;
  }
  const secondsPerBeat = covariance / variance;
  if (!Number.isFinite(secondsPerBeat) || secondsPerBeat <= 0) return null;
  return 60 / secondsPerBeat * Math.max(0.5, Math.min(2, playbackRate));
}
