import type { LyricsSearchResult } from "./types";

export function lyricsSearchQueries(title: string): string[] {
  const cleaned = title.normalize("NFKC")
    .replace(/\([^)]*\)|\[[^\]]*\]/g, " ")
    .replace(/[^\p{L}\p{N}]+/gu, " ")
    .trim()
    .replace(/\s+/g, " ");
  const words = cleaned.split(" ").filter(Boolean);
  const queries: string[] = [];
  for (let pass = 0; pass < 3 && words.length - pass > 0; pass += 1) {
    const query = words.slice(0, words.length - pass).join(" ");
    if (query && queries.at(-1) !== query) queries.push(query);
  }
  return queries;
}

export function preferredLyricsResult(
  results: LyricsSearchResult[],
  durationSeconds: number | null,
): LyricsSearchResult | null {
  const usable = results.filter((result) => !result.instrumental && (result.hasSyncedLyrics || result.hasPlainLyrics));
  if (!usable.length) return null;
  const synchronized = usable.filter((result) => result.hasSyncedLyrics);
  const candidates = synchronized.length ? synchronized : usable;
  if (durationSeconds === null) return candidates[0];
  return candidates.reduce((best, result) => (
    Math.abs(result.durationSeconds - durationSeconds) < Math.abs(best.durationSeconds - durationSeconds) ? result : best
  ));
}

export function lyricsDurationRelevanceLevel(candidateSeconds: number, audioSeconds: number): 0 | 1 | 2 | 3 | 4 {
  if (!Number.isFinite(candidateSeconds) || !Number.isFinite(audioSeconds) || candidateSeconds <= 0 || audioSeconds <= 0) return 0;
  const difference = Math.abs(candidateSeconds - audioSeconds);
  const ratio = difference / audioSeconds;
  if (difference <= 2 || ratio <= 0.015) return 4;
  if (difference <= 8 || ratio <= 0.05) return 3;
  if (difference <= 15 || ratio <= 0.1) return 2;
  if (ratio <= 0.2) return 1;
  return 0;
}
