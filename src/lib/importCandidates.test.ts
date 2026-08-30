import assert from "node:assert/strict";
import test from "node:test";

import { deduplicateImportCandidates, defaultImportSelection, normalizeImportQuery, reconcileImportSelection } from "./importCandidates.ts";
import type { ImportCandidateGroup } from "./importCandidates.ts";
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

test("selects a single search result but never preselects several results", () => {
  const one = { input: "https://youtu.be/one", title: "One", detail: "Channel", kind: "video" } as const;
  const two = { input: "https://youtu.be/two", title: "Two", detail: "Channel", kind: "video" } as const;
  const three = { input: "https://youtu.be/three", title: "Three", detail: "Channel", kind: "video" } as const;
  const local = { input: "file:///music/local.wav", title: "Local", detail: "Local file", kind: "local" } as const;
  const groups: ImportCandidateGroup[] = [
    { id: "direct", query: null, searchIndex: null, candidates: [local] },
    { id: "search:1", query: "unique song", searchIndex: 1, candidates: [one] },
    { id: "search:2", query: "ambiguous song", searchIndex: 2, candidates: [two, three] },
  ];

  assert.deepEqual([...defaultImportSelection(groups)], [local.input, one.input]);
});

test("preserves explicit choices when unchanged searches move or new searches are added", () => {
  const chosen = { input: "https://youtu.be/chosen", title: "Chosen", detail: "Channel", kind: "video" } as const;
  const ignored = { input: "https://youtu.be/ignored", title: "Ignored", detail: "Channel", kind: "video" } as const;
  const newOnly = { input: "https://youtu.be/new", title: "New", detail: "Channel", kind: "video" } as const;
  const previous: ImportCandidateGroup[] = [
    { id: "search:beatles imagine", query: "beatles imagine", searchIndex: 1, candidates: [chosen, ignored] },
  ];
  const next: ImportCandidateGroup[] = [
    { id: "search:new line", query: "new line", searchIndex: 1, candidates: [newOnly] },
    { id: "search:beatles imagine", query: "beatles imagine", searchIndex: 2, candidates: [chosen, ignored] },
  ];

  assert.deepEqual([...reconcileImportSelection(new Set([chosen.input]), previous, next)], [chosen.input, newOnly.input]);
  assert.equal(normalizeImportQuery("  Beatles   Imagine "), "beatles imagine");
});
