export const PLAYBACK_UI_INTERVAL_MS = 100;

export function shouldPublishPlaybackUiPosition(
  lastPublishedAtMs: number,
  nowMs: number,
  playing: boolean,
): boolean {
  return !playing
    || !Number.isFinite(lastPublishedAtMs)
    || nowMs - lastPublishedAtMs >= PLAYBACK_UI_INTERVAL_MS;
}
