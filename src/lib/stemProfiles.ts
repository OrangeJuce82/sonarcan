import type { StemSeparationProfile } from "./types";

export function preferredStemProfile(
  available: readonly StemSeparationProfile[],
  lastUsed: StemSeparationProfile | null | undefined,
): StemSeparationProfile | null {
  if (lastUsed && available.includes(lastUsed)) return lastUsed;
  if (available.includes("hq")) return "hq";
  if (available.includes("fast")) return "fast";
  return null;
}
