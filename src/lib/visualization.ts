import type { SpectrumRange, VisualizationResponse } from "./types";

export interface EnergyPoint {
  timestampMs: number;
  left: number;
  right: number;
}

export interface MeterState {
  level: number;
  heldPeak: number;
  heldAtMs: number;
  updatedAtMs: number;
}

export const visualizationKinds = ["spectrum", "meter", "energy"] as const;

export function responseAlpha(response: VisualizationResponse): number {
  if (response === "fast") return 0.72;
  if (response === "smooth") return 0.18;
  return 0.38;
}

export function smoothValues(previous: number[], next: number[], response: VisualizationResponse): number[] {
  const alpha = responseAlpha(response);
  return next.map((value, index) => {
    const prior = previous[index] ?? value;
    return Math.max(0, Math.min(1, prior + (value - prior) * alpha));
  });
}

export function emptyMeterState(): MeterState {
  return { level: 0, heldPeak: 0, heldAtMs: 0, updatedAtMs: 0 };
}

export function meterPeakHoldMilliseconds(peakHold: "off" | "oneSecond" | "threeSeconds"): number {
  if (peakHold === "oneSecond") return 1_000;
  if (peakHold === "threeSeconds") return 3_000;
  return 0;
}

export function updateMeterState(
  previous: MeterState,
  inputLevel: number,
  response: VisualizationResponse,
  holdMilliseconds: number,
  nowMs: number,
): MeterState {
  const input = Math.max(0, Math.min(1, inputLevel));
  const elapsedMs = previous.updatedAtMs > 0
    ? Math.max(0, Math.min(250, nowMs - previous.updatedAtMs))
    : 0;
  const releaseMilliseconds = response === "fast" ? 55 : response === "smooth" ? 190 : 105;
  const releaseAlpha = elapsedMs > 0 ? 1 - Math.exp(-elapsedMs / releaseMilliseconds) : 1;
  const level = input >= previous.level
    ? input
    : previous.level + (input - previous.level) * releaseAlpha;

  if (holdMilliseconds <= 0) {
    return { level, heldPeak: 0, heldAtMs: nowMs, updatedAtMs: nowMs };
  }
  if (level >= previous.heldPeak) {
    return { level, heldPeak: level, heldAtMs: nowMs, updatedAtMs: nowMs };
  }
  if (nowMs - previous.heldAtMs <= holdMilliseconds) {
    return { level, heldPeak: previous.heldPeak, heldAtMs: previous.heldAtMs, updatedAtMs: nowMs };
  }

  const peakReleasePerMillisecond = 0.0008;
  const heldPeak = Math.max(level, previous.heldPeak - elapsedMs * peakReleasePerMillisecond);
  return {
    level,
    heldPeak,
    heldAtMs: heldPeak === level ? nowMs : previous.heldAtMs,
    updatedAtMs: nowMs,
  };
}

export function spectrumRangeSlice(values: number[], range: SpectrumRange): number[] {
  if (range === "full") return values;
  const boundaries: Record<Exclude<SpectrumRange, "full">, [number, number]> = {
    low: [0, 0.43],
    mid: [0.31, 0.74],
    high: [0.63, 1],
  };
  const [startRatio, endRatio] = boundaries[range];
  const start = Math.floor(values.length * startRatio);
  const end = Math.max(start + 1, Math.ceil(values.length * endRatio));
  return values.slice(start, end);
}

export function decibelsFromLevel(level: number): number {
  return Math.max(-60, 20 * Math.log10(Math.max(level, 0.001)));
}

export function linePath(values: number[], width = 100, height = 100): string {
  if (values.length === 0) return "";
  return values.map((value, index) => {
    const x = values.length === 1 ? 0 : index / (values.length - 1) * width;
    const y = height - Math.max(0, Math.min(1, value)) * height;
    return `${index === 0 ? "M" : "L"}${x.toFixed(2)},${y.toFixed(2)}`;
  }).join(" ");
}
