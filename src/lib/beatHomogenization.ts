export type BeatSubdivisionMode = "off" | "auto" | "eighth" | "sixteenth";

export interface HomogenizedBeatTimeline {
  beats: number[];
  downbeats: number[];
  bpm: number | null;
}

const BEAT_SUBDIVISION_MODES: readonly BeatSubdivisionMode[] = ["off", "auto", "eighth", "sixteenth"];

export function nextBeatSubdivisionMode(mode: BeatSubdivisionMode): BeatSubdivisionMode {
  const index = BEAT_SUBDIVISION_MODES.indexOf(mode);
  return BEAT_SUBDIVISION_MODES[(index + 1) % BEAT_SUBDIVISION_MODES.length] ?? "auto";
}

const OCTAVE_RATIO_TOLERANCE = 0.12;
const MIN_INTERVAL_SECONDS = 0.12;
const MAX_INTERVAL_SECONDS = 2;
const MIN_ANOMALY_INTERVALS = 3;
const MIN_ANOMALY_TARGET_BEATS = 4;
const TIME_EQUALITY_TOLERANCE_SECONDS = 0.001;

function median(values: readonly number[]): number | null {
  if (!values.length) return null;
  const ordered = [...values].sort((left, right) => left - right);
  const middle = Math.floor(ordered.length / 2);
  return ordered.length % 2
    ? ordered[middle] ?? null
    : ((ordered[middle - 1] ?? 0) + (ordered[middle] ?? 0)) / 2;
}

function nearRatio(value: number, reference: number, ratio: number): boolean {
  if (!(value > 0) || !(reference > 0)) return false;
  const expected = reference * ratio;
  return value >= expected * (1 - OCTAVE_RATIO_TOLERANCE)
    && value <= expected * (1 + OCTAVE_RATIO_TOLERANCE);
}

function clusterAround(intervals: readonly number[], candidate: number): number[] {
  return intervals.filter((interval) => nearRatio(interval, candidate, 1));
}

function dominantInterval(intervals: readonly number[]): number | null {
  let best: number[] = [];
  let bestCandidate = Number.POSITIVE_INFINITY;
  for (const candidate of intervals) {
    const cluster = clusterAround(intervals, candidate);
    if (cluster.length > best.length || (cluster.length === best.length && candidate < bestCandidate)) {
      best = cluster;
      bestCandidate = candidate;
    }
  }
  return median(best);
}

function targetInterval(intervals: readonly number[], mode: BeatSubdivisionMode): number | null {
  const dominant = dominantInterval(intervals);
  if (dominant === null) return null;
  if (mode === "auto") return dominant;
  const slowerCluster = intervals.filter((interval) => nearRatio(interval, dominant, 2));
  const coarse = median(slowerCluster) ?? dominant;
  return mode === "eighth" ? coarse : coarse / 2;
}

function sustainedAnomalies(
  intervals: readonly number[],
  target: number,
  ratio: 0.5 | 2,
): boolean[] {
  const corrections = intervals.map(() => false);
  let runStart = 0;
  while (runStart < intervals.length) {
    if (!nearRatio(intervals[runStart] ?? 0, target, ratio)) {
      runStart += 1;
      continue;
    }
    let runEnd = runStart + 1;
    let duration = intervals[runStart] ?? 0;
    while (runEnd < intervals.length && nearRatio(intervals[runEnd] ?? 0, target, ratio)) {
      duration += intervals[runEnd] ?? 0;
      runEnd += 1;
    }
    if (runEnd - runStart >= MIN_ANOMALY_INTERVALS && duration >= target * MIN_ANOMALY_TARGET_BEATS) {
      for (let index = runStart; index < runEnd; index += 1) corrections[index] = true;
    }
    runStart = runEnd;
  }
  return corrections;
}

function containsTime(values: readonly number[], time: number): boolean {
  return values.some((value) => Math.abs(value - time) <= TIME_EQUALITY_TOLERANCE_SECONDS);
}

export function homogenizeBeatTimeline(
  beats: readonly number[],
  downbeats: readonly number[],
  mode: BeatSubdivisionMode,
): HomogenizedBeatTimeline {
  if (mode === "off") {
    return { beats: [...beats], downbeats: [...downbeats], bpm: null };
  }
  const sourceBeats = [...new Set(beats.filter((beat) => Number.isFinite(beat) && beat >= 0))]
    .sort((left, right) => left - right);
  const sourceDownbeats = [...new Set(downbeats.filter((beat) => Number.isFinite(beat) && beat >= 0))]
    .sort((left, right) => left - right);
  const intervals = sourceBeats.slice(1)
    .map((beat, index) => beat - (sourceBeats[index] ?? beat))
    .filter((interval) => interval >= MIN_INTERVAL_SECONDS && interval <= MAX_INTERVAL_SECONDS);
  const target = targetInterval(intervals, mode);
  if (sourceBeats.length < 2 || target === null) {
    return { beats: sourceBeats, downbeats: sourceDownbeats, bpm: null };
  }

  const sourceIntervals = sourceBeats.slice(1).map((beat, index) => beat - (sourceBeats[index] ?? beat));
  const denseIntervals = sustainedAnomalies(sourceIntervals, target, 0.5);
  const sparseIntervals = sustainedAnomalies(sourceIntervals, target, 2);
  const output = [sourceBeats[0]!];
  for (let intervalIndex = 0; intervalIndex < sourceIntervals.length; intervalIndex += 1) {
    const beat = sourceBeats[intervalIndex + 1]!;
    if (denseIntervals[intervalIndex]) {
      const last = output[output.length - 1]!;
      const previous = output[output.length - 2];
      const canPreferDownbeat = containsTime(sourceDownbeats, beat)
        && !containsTime(sourceDownbeats, last)
        && (previous === undefined || nearRatio(beat - previous, target, 1));
      if (canPreferDownbeat) output[output.length - 1] = beat;
      else if (beat - last >= target * (1 - OCTAVE_RATIO_TOLERANCE)) output.push(beat);
      continue;
    }
    if (sparseIntervals[intervalIndex]) {
      const last = output[output.length - 1]!;
      output.push(last + (beat - last) / 2);
    }
    output.push(beat);
  }

  const outputDownbeats = sourceDownbeats.filter((downbeat) => containsTime(output, downbeat));
  const outputIntervals = output.slice(1).map((beat, index) => beat - (output[index] ?? beat));
  const outputMedian = median(outputIntervals.filter((interval) => interval > 0));
  return {
    beats: output,
    downbeats: outputDownbeats,
    bpm: outputMedian === null ? null : 60 / outputMedian,
  };
}
