import type { SystemMetrics } from "./types";

export type ResourcePressure = "normal" | "warm" | "hot";

export function resourceLoad(metrics: SystemMetrics): number {
  return Math.max(
    0,
    ...[metrics.cpuPercent, metrics.gpuPercent, metrics.memoryPercent]
      .filter((value): value is number => value !== null)
      .map((value) => Math.min(100, Math.max(0, value))),
  );
}

export function resourcePressure(load: number): ResourcePressure {
  if (load >= 85) return "hot";
  if (load >= 65) return "warm";
  return "normal";
}

export function activeResourceLeds(value: number | null, count = 14): number {
  if (value === null) return 0;
  return Math.min(count, Math.max(0, Math.ceil(value / 100 * count)));
}

export function formatResourceMemory(megabytes: number | null): string {
  if (megabytes === null) return "—";
  if (megabytes < 1_024) return `${megabytes} MB`;
  return `${(megabytes / 1_024).toFixed(megabytes >= 10_240 ? 1 : 2)} GB`;
}
