import assert from "node:assert/strict";
import test from "node:test";

import { deduplicateImportCandidates } from "./importCandidates.ts";
import type { ImportCandidate } from "./types.ts";

test("deduplicates local candidates by filename", () => {
  const candidates: ImportCandidate[] = [
    { input: "file:///music/Track%20One.mp3", title: "Track One", detail: "Local file", kind: "local" },
    { input: "file:///backup/track one.MP3", title: "track one", detail: "Local file", kind: "local" },
  ];

  assert.deepEqual(deduplicateImportCandidates(candidates), [candidates[0]]);
});

test("deduplicates equivalent YouTube URLs after search resolution", () => {
  const candidates: ImportCandidate[] = [
    { input: "https://youtu.be/AbC123", title: "First", detail: "YouTube", kind: "video" },
    { input: "https://www.youtube.com/watch?v=AbC123&feature=share", title: "Second", detail: "YouTube", kind: "video" },
  ];

  assert.deepEqual(deduplicateImportCandidates(candidates), [candidates[0]]);
});
