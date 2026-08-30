interface SelectionStorage {
  getItem(key: string): string | null;
  setItem(key: string, value: string): void;
}

interface StoredSelection {
  projectPath: string;
  trackId: string;
}

const STORAGE_KEY = "sonarcan.project-track-selection";
const MAX_SELECTIONS = 50;
const MAX_STORAGE_LENGTH = 64 * 1024;

function readSelections(storage: SelectionStorage): StoredSelection[] {
  try {
    const serialized = storage.getItem(STORAGE_KEY);
    if (!serialized || serialized.length > MAX_STORAGE_LENGTH) return [];
    const parsed: unknown = JSON.parse(serialized);
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((value): value is StoredSelection => {
        if (!value || typeof value !== "object") return false;
        const candidate = value as Partial<StoredSelection>;
        return typeof candidate.projectPath === "string"
          && candidate.projectPath.length > 0
          && candidate.projectPath.length <= 4_096
          && typeof candidate.trackId === "string"
          && candidate.trackId.length > 0
          && candidate.trackId.length <= 128;
      })
      .slice(0, MAX_SELECTIONS);
  } catch {
    return [];
  }
}

function writeSelections(storage: SelectionStorage, selections: StoredSelection[]): void {
  try {
    storage.setItem(STORAGE_KEY, JSON.stringify(selections.slice(0, MAX_SELECTIONS)));
  } catch {
    // Remembering the selection is a convenience and must never block project loading.
  }
}

export function rememberedTrackId(storage: SelectionStorage, projectPath: string): string | null {
  return readSelections(storage).find((selection) => selection.projectPath === projectPath)?.trackId ?? null;
}

export function rememberTrackSelection(storage: SelectionStorage, projectPath: string, trackId: string): void {
  const selections = readSelections(storage).filter((selection) => selection.projectPath !== projectPath);
  selections.unshift({ projectPath, trackId });
  writeSelections(storage, selections);
}

export function forgetTrackSelection(storage: SelectionStorage, projectPath: string): void {
  writeSelections(storage, readSelections(storage).filter((selection) => selection.projectPath !== projectPath));
}

export function preferredTrack<T extends { id: string }>(tracks: T[], rememberedId: string | null): T | null {
  if (tracks.length === 0) return null;
  return tracks.find((track) => track.id === rememberedId) ?? tracks[0];
}
