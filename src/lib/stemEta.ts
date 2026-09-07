import type { StemSeparationProfile, StemStatus } from "./types";

export interface StemEtaState {
  profile: StemSeparationProfile;
  audioDurationSeconds: number;
  stage: string;
  stageStartedAtMs: number;
  stageInitialCompleted: number;
  secondsPerUnit: number | null;
  estimatedRemainingSeconds: number;
  updatedAtMs: number;
}

interface PhaseEstimate {
  current: number;
  following: number;
}

function phaseEstimate(
  profile: StemSeparationProfile,
  duration: number,
  stage: string,
): PhaseEstimate {
  const separating = Math.max(profile === "hq" ? 75 : 28, duration * (profile === "hq" ? 1.5 : 0.35));
  const loadingModel = profile === "hq" ? 14 : 9;
  const loadingAudio = Math.max(2, duration * 0.015);
  const writing = Math.max(3, duration * 0.025);
  const validating = Math.max(3, duration * 0.02);
  const caching = Math.max(2, duration * 0.015);
  switch (stage) {
    case "checkingCache": return { current: 2, following: loadingModel + loadingAudio + separating + writing + validating + caching };
    case "downloadingModel": return { current: 45, following: loadingModel + loadingAudio + separating + writing + validating + caching };
    case "loadingModel": return { current: loadingModel, following: loadingAudio + separating + writing + validating + caching };
    case "loadingAudio": return { current: loadingAudio, following: separating + writing + validating + caching };
    case "separating": return { current: separating, following: writing + validating + caching };
    case "writingStems": return { current: writing, following: validating + caching };
    case "validatingStems": return { current: validating, following: caching };
    case "cachingStems": return { current: caching, following: 0 };
    default: return { current: separating, following: writing + validating + caching };
  }
}

export function startStemEta(
  profile: StemSeparationProfile,
  audioDurationSeconds: number,
  nowMs: number,
): StemEtaState {
  const duration = Math.max(1, audioDurationSeconds);
  const estimate = phaseEstimate(profile, duration, "checkingCache");
  return {
    profile,
    audioDurationSeconds: duration,
    stage: "checkingCache",
    stageStartedAtMs: nowMs,
    stageInitialCompleted: 0,
    secondsPerUnit: null,
    estimatedRemainingSeconds: estimate.current + estimate.following,
    updatedAtMs: nowMs,
  };
}

export function updateStemEta(
  state: StemEtaState,
  status: Pick<StemStatus, "stage" | "phaseCompleted" | "phaseTotal">,
  nowMs: number,
): StemEtaState {
  const estimate = phaseEstimate(state.profile, state.audioDurationSeconds, status.stage);
  const completed = status.phaseCompleted ?? null;
  const total = status.phaseTotal ?? null;
  if (status.stage !== state.stage) {
    return {
      ...state,
      stage: status.stage,
      stageStartedAtMs: nowMs,
      stageInitialCompleted: completed ?? 0,
      secondsPerUnit: null,
      estimatedRemainingSeconds: estimate.current + estimate.following,
      updatedAtMs: nowMs,
    };
  }

  const elapsed = Math.max(0, (nowMs - state.stageStartedAtMs) / 1_000);
  const measuredUnits = completed === null ? 0 : completed - state.stageInitialCompleted;
  let secondsPerUnit = state.secondsPerUnit;
  if (total !== null && completed !== null && measuredUnits > 0 && elapsed >= 0.5) {
    const observed = elapsed / measuredUnits;
    secondsPerUnit = secondsPerUnit === null ? observed : secondsPerUnit * 0.65 + observed * 0.35;
  }

  const measuredRemaining = secondsPerUnit !== null && total !== null && completed !== null
    ? secondsPerUnit * Math.max(0, total - completed)
    : Math.max(0, estimate.current - elapsed);
  const rawRemaining = measuredRemaining + estimate.following;
  const previousAtNow = Math.max(0, state.estimatedRemainingSeconds - (nowMs - state.updatedAtMs) / 1_000);
  const smoothedRemaining = secondsPerUnit === null
    ? rawRemaining
    : previousAtNow * 0.35 + rawRemaining * 0.65;
  return {
    ...state,
    secondsPerUnit,
    estimatedRemainingSeconds: Math.min(7_200, Math.max(0, smoothedRemaining)),
    updatedAtMs: nowMs,
  };
}

export function stemRemainingSeconds(
  state: StemEtaState | null,
  progress: number,
  nowMs: number,
): number | null {
  if (!state || progress >= 0.999) return null;
  const sinceUpdate = Math.max(0, (nowMs - state.updatedAtMs) / 1_000);
  return Math.max(1, Math.round(state.estimatedRemainingSeconds - sinceUpdate));
}

export function formatStemEta(seconds: number): string {
  if (seconds < 60) return `${Math.max(1, Math.round(seconds))}s`;
  const minutes = Math.floor(seconds / 60);
  const remainder = Math.round(seconds % 60);
  return remainder === 0 ? `${minutes}m` : `${minutes}m ${remainder}s`;
}
