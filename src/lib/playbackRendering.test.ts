import assert from "node:assert/strict";
import test from "node:test";
import { PLAYBACK_UI_INTERVAL_MS, shouldPublishPlaybackUiPosition } from "./playbackRendering.ts";

test("publishes playback UI state at no more than ten hertz while playing", () => {
  let lastPublishedAtMs = Number.NEGATIVE_INFINITY;
  let publications = 0;

  for (let nowMs = 0; nowMs < 1_000; nowMs += 50) {
    if (!shouldPublishPlaybackUiPosition(lastPublishedAtMs, nowMs, true)) continue;
    publications += 1;
    lastPublishedAtMs = nowMs;
  }

  assert.equal(PLAYBACK_UI_INTERVAL_MS, 100);
  assert.equal(publications, 10);
});

test("publishes every stopped position so seeks and loop resets stay exact", () => {
  assert.equal(shouldPublishPlaybackUiPosition(950, 975, false), true);
});
