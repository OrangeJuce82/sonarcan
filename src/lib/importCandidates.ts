import type { ImportCandidate } from "./types";

export interface ImportCandidateGroup {
  id: string;
  query: string | null;
  searchIndex: number | null;
  candidates: ImportCandidate[];
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

export function defaultImportSelection(groups: ImportCandidateGroup[]): Set<string> {
  return new Set(groups.flatMap((group) => {
    if (group.query === null) return group.candidates.map((candidate) => candidate.input);
    return group.candidates.length === 1 ? [group.candidates[0].input] : [];
  }));
}

export function reconcileImportSelection(
  previousSelection: ReadonlySet<string>,
  previousGroups: ImportCandidateGroup[],
  nextGroups: ImportCandidateGroup[],
): Set<string> {
  const previousCandidates = new Set(previousGroups.flatMap((group) => group.candidates.map((candidate) => candidate.input)));
  const nextCandidates = new Set(nextGroups.flatMap((group) => group.candidates.map((candidate) => candidate.input)));
  const selection = new Set([...previousSelection].filter((input) => nextCandidates.has(input)));

  for (const group of nextGroups) {
    const shouldSelectNewCandidate = group.query === null || group.candidates.length === 1;
    if (!shouldSelectNewCandidate) continue;
    for (const candidate of group.candidates) {
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
