import type { ImportCandidate } from "./types";

export interface ImportCandidateGroup {
  id: string;
  query: string | null;
  searchIndex: number | null;
  candidates: ImportCandidate[];
  kind?: "direct" | "search" | "playlist";
}

export type ImportRelevanceLevel = 0 | 1 | 2 | 3 | 4;

export function importRelevancePercent(score: number): number {
  if (!Number.isFinite(score)) return 0;
  return Math.round(Math.max(0, Math.min(1, score)) * 100);
}

export function importRelevanceLevel(score: number): ImportRelevanceLevel {
  const percent = importRelevancePercent(score);
  if (percent >= 100) return 4;
  if (percent >= 75) return 3;
  if (percent >= 50) return 2;
  if (percent >= 25) return 1;
  return 0;
}

export function deduplicateImportCandidates(candidates: ImportCandidate[]): ImportCandidate[] {
  const seen = new Set<string>();
  return candidates.filter((candidate) => {
    const key = importCandidateKey(candidate);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
}

export function orderedImportCandidateGroups(
  candidates: ImportCandidate[],
  searchProvider: "youtube" | "soundcloud",
): ImportCandidateGroup[] {
  const groups: ImportCandidateGroup[] = [];
  let searchIndex = 0;

  for (const candidate of candidates) {
    if (candidate.kind === "search") {
      searchIndex += 1;
      groups.push({
        id: `search:${searchProvider}:${normalizeImportQuery(candidate.input)}`,
        kind: "search",
        query: candidate.input,
        searchIndex,
        candidates: [],
      });
      continue;
    }

    if (candidate.kind === "playlist") {
      groups.push({
        id: `playlist:${candidate.input}`,
        kind: "playlist",
        query: candidate.input,
        searchIndex: null,
        candidates: [],
      });
      continue;
    }

    if (candidate.kind === "video" && candidate.sourceUrl) {
      groups.push({
        id: `source:${candidate.input}`,
        kind: "direct",
        query: candidate.input,
        searchIndex: null,
        candidates: [candidate],
      });
      continue;
    }

    groups.push({
      id: `direct:${candidate.input}`,
      kind: "direct",
      query: null,
      searchIndex: null,
      candidates: [candidate],
    });
  }

  return groups;
}

export function defaultImportSelection(groups: ImportCandidateGroup[], autoSelectBestMatch = false): Set<string> {
  return new Set(groups.flatMap((group) => {
    const importable = group.candidates.filter((candidate) => !candidate.blocked);
    if (group.query === null || group.kind === "playlist") return importable.map((candidate) => candidate.input);
    return importable.length === 1 || autoSelectBestMatch ? importable.slice(0, 1).map((candidate) => candidate.input) : [];
  }));
}

export function reconcileImportSelection(
  previousSelection: ReadonlySet<string>,
  previousGroups: ImportCandidateGroup[],
  nextGroups: ImportCandidateGroup[],
  autoSelectBestMatch = false,
): Set<string> {
  const previousCandidates = new Set(previousGroups.flatMap((group) => group.candidates.map((candidate) => candidate.input)));
  const nextCandidates = new Set(nextGroups.flatMap((group) => group.candidates.filter((candidate) => !candidate.blocked).map((candidate) => candidate.input)));
  const selection = new Set([...previousSelection].filter((input) => nextCandidates.has(input)));

  for (const group of nextGroups) {
    const selectAll = group.query === null || group.kind === "playlist";
    const shouldSelectNewCandidate = selectAll || group.candidates.length === 1 || autoSelectBestMatch;
    if (!shouldSelectNewCandidate) continue;
    const importable = group.candidates.filter((candidate) => !candidate.blocked);
    const candidates = !selectAll && autoSelectBestMatch ? importable.slice(0, 1) : importable;
    for (const candidate of candidates) {
      if (!previousCandidates.has(candidate.input)) selection.add(candidate.input);
    }
  }
  return selection;
}

export function normalizeImportQuery(query: string): string {
  return query.trim().replace(/\s+/g, " ").toLocaleLowerCase();
}

function importCandidateKey(candidate: ImportCandidate): string {
  if (candidate.kind === "local") {
    const path = candidate.input.replace(/^file:\/\/(?:localhost)?/i, "");
    const filename = path.split(/[\\/]/).at(-1) ?? path;
    return `file:${safeDecode(filename).toLocaleLowerCase()}`;
  }
  if (candidate.kind === "search") {
    return `search:${candidate.input.trim().replace(/\s+/g, " ").toLocaleLowerCase()}`;
  }
  try {
    const url = new URL(candidate.input);
    const host = url.hostname.toLocaleLowerCase();
    if (host === "youtu.be") {
      return `youtube-video:${url.pathname.split("/").filter(Boolean)[0] ?? ""}`;
    }
    if (["youtube.com", "www.youtube.com", "m.youtube.com"].includes(host)) {
      const playlistId = url.searchParams.get("list");
      if (playlistId) return `youtube-playlist:${playlistId}`;
      const videoId = url.searchParams.get("v");
      if (videoId) return `youtube-video:${videoId}`;
    }
    url.hash = "";
    url.pathname = url.pathname.replace(/\/$/, "");
    return `url:${url.toString()}`;
  } catch {
    return `url:${candidate.input.trim()}`;
  }
}

function safeDecode(value: string): string {
  try {
    return decodeURIComponent(value);
  } catch {
    return value;
  }
}
