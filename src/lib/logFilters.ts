import type { AppLogEntry } from "./types";

export type LogLevel = "debug" | "info" | "warn" | "error";

const levelRanks: Record<LogLevel, number> = {
  debug: 0,
  info: 1,
  warn: 2,
  error: 3,
};

function levelRank(level: string): number {
  return levelRanks[level as LogLevel] ?? levelRanks.info;
}

export function logOrigins(entries: AppLogEntry[]): string[] {
  return [...new Set(entries.map((entry) => entry.origin))].sort((left, right) => left.localeCompare(right));
}

export function filterLogs(entries: AppLogEntry[], minimumLevel: LogLevel, origin: string | null): AppLogEntry[] {
  const minimumRank = levelRanks[minimumLevel];
  return entries.filter((entry) => levelRank(entry.level) >= minimumRank && (origin === null || entry.origin === origin));
}
