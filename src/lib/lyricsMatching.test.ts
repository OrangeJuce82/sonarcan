import test from "node:test";
import assert from "node:assert/strict";
import { lyricsDurationRelevanceLevel, lyricsSearchQueries, preferredLyricsResult } from "./lyricsMatching.ts";
import type { LyricsSearchResult } from "./types.ts";

const result = (overrides: Partial<LyricsSearchResult> = {}): LyricsSearchResult => ({
  id: 1,
  trackName: "Perfect Day",
  artistName: "Lou Reed",
  albumName: "Transformer",
  durationSeconds: 180,
  instrumental: false,
  hasSyncedLyrics: true,
  hasPlainLyrics: true,
  ...overrides,
});

test("builds three progressively broader searches from a noisy title", () => {
  assert.deepEqual(lyricsSearchQueries("Lou Reed - Perfect_Day Live 2026 (Official Audio) [HD]"), [
    "Lou Reed Perfect Day Live 2026",
    "Lou Reed Perfect Day Live",
    "Lou Reed Perfect Day",
  ]);
});

test("removes bracketed text and standalone special characters", () => {
  assert.deepEqual(lyricsSearchQueries("Björk — Jóga_Live [Remaster]"), ["Björk Jóga Live", "Björk Jóga", "Björk"]);
});

test("prefers the result closest to the recording duration", () => {
  assert.equal(preferredLyricsResult([result({ id: 1, durationSeconds: 240 }), result({ id: 2, durationSeconds: 181 })], 180)?.id, 2);
});

test("prefers synchronized lyrics before a closer plain-text result", () => {
  assert.equal(preferredLyricsResult([
    result({ id: 1, durationSeconds: 181, hasSyncedLyrics: false, hasPlainLyrics: true }),
    result({ id: 2, durationSeconds: 184, hasSyncedLyrics: true }),
  ], 180)?.id, 2);
});

test("ignores instrumental and lyric-less records", () => {
  assert.equal(preferredLyricsResult([result({ instrumental: true }), result({ hasSyncedLyrics: false, hasPlainLyrics: false })], 180), null);
});

test("colors result durations by their difference from the local audio", () => {
  assert.equal(lyricsDurationRelevanceLevel(181, 180), 4);
  assert.equal(lyricsDurationRelevanceLevel(187, 180), 3);
  assert.equal(lyricsDurationRelevanceLevel(194, 180), 2);
  assert.equal(lyricsDurationRelevanceLevel(210, 180), 1);
  assert.equal(lyricsDurationRelevanceLevel(240, 180), 0);
});
