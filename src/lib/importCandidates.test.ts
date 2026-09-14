import assert from "node:assert/strict";
import test from "node:test";

import { deduplicateImportCandidates, defaultImportSelection, importRelevanceLevel, importRelevancePercent, normalizeImportQuery, orderedImportCandidateGroups, reconcileImportSelection } from "./importCandidates.ts";
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
  assert.deepEqual([...defaultImportSelection(groups, true)], [local.input, one.input, two.input]);
});

test("auto-selection chooses only the first newly ranked search result", () => {
  const best = { input: "https://youtu.be/best", title: "Best", detail: "Artist", kind: "video" } as const;
  const other = { input: "https://youtu.be/other", title: "Other", detail: "Channel", kind: "video" } as const;
  const next: ImportCandidateGroup[] = [
    { id: "search:song", query: "artist song", searchIndex: 1, candidates: [best, other] },
  ];

  assert.deepEqual([...reconcileImportSelection(new Set(), [], next, true)], [best.input]);
  assert.deepEqual([...reconcileImportSelection(new Set(), [], next, false)], []);
});

test("preselects every expanded playlist track", () => {
  const one = { input: "https://soundcloud.com/artist/one", title: "One", detail: "Artist", kind: "video" } as const;
  const two = { input: "https://soundcloud.com/artist/two", title: "Two", detail: "Artist", kind: "video" } as const;
  const groups: ImportCandidateGroup[] = [
    { id: "playlist:set", kind: "playlist", query: "https://soundcloud.com/artist/sets/set", searchIndex: null, candidates: [one, two] },
  ];

  assert.deepEqual([...defaultImportSelection(groups)], [one.input, two.input]);
  assert.deepEqual([...reconcileImportSelection(new Set(), [], groups)], [one.input, two.input]);
});

test("keeps blocked provider results visible but never selectable", () => {
  const blocked = { input: "https://soundcloud.com/artist/drm", title: "Known title", detail: "Artist", kind: "video", blocked: true } as const;
  const available = { input: "https://soundcloud.com/artist/public", title: "Public title", detail: "Artist", kind: "video" } as const;
  const groups: ImportCandidateGroup[] = [
    { id: "playlist:set", kind: "playlist", query: "https://soundcloud.com/artist/sets/set", searchIndex: null, candidates: [blocked, available] },
  ];
  const unresolvedGroups: ImportCandidateGroup[] = [
    { id: "playlist:set", kind: "playlist", query: "https://soundcloud.com/artist/sets/set", searchIndex: null, candidates: [blocked] },
  ];

  assert.deepEqual([...defaultImportSelection(groups)], [available.input]);
  assert.deepEqual(
    [...reconcileImportSelection(new Set([blocked.input]), unresolvedGroups, groups)],
    [available.input],
  );
  assert.equal(groups[0].candidates[0].title, "Known title");
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

test("keeps result groups in input-line order and exposes direct links in their header", () => {
  const firstSearch = { input: "new first line", title: "new first line", detail: "YouTube search", kind: "search" } as const;
  const directLink = { input: "https://youtu.be/direct", title: "Direct", detail: "YouTube URL", kind: "video", sourceUrl: "https://youtu.be/direct" } as const;
  const playlist = { input: "https://youtube.com/playlist?list=set", title: "Set", detail: "YouTube playlist", kind: "playlist" } as const;
  const lastSearch = { input: "existing last line", title: "existing last line", detail: "YouTube search", kind: "search" } as const;

  const groups = orderedImportCandidateGroups([firstSearch, directLink, playlist, lastSearch], "youtube");

  assert.deepEqual(groups.map((group) => group.query), [
    firstSearch.input,
    directLink.input,
    playlist.input,
    lastSearch.input,
  ]);
  assert.deepEqual(groups.map((group) => group.searchIndex), [1, null, null, 2]);
  assert.deepEqual(groups.map((group) => group.kind), ["search", "direct", "playlist", "search"]);
});

test("keeps direct input lines in separate result blocks", () => {
  const first = { input: "file:///music/first.wav", title: "first.wav", detail: "Local file", kind: "local" } as const;
  const second = { input: "file:///music/second.wav", title: "second.wav", detail: "Local file", kind: "local" } as const;

  const groups = orderedImportCandidateGroups([first, second], "youtube");

  assert.deepEqual(groups.map((group) => group.candidates), [[first], [second]]);
});

test("moves an existing search group when its text line moves to the top", () => {
  const other = { input: "Peter Tosh Legalize It", title: "Peter Tosh Legalize It", detail: "YouTube search", kind: "search" } as const;
  const bobMarley = { input: "Bob marley one love", title: "Bob marley one love", detail: "YouTube search", kind: "search" } as const;
  const initial = orderedImportCandidateGroups([other, bobMarley], "youtube");
  const reordered = orderedImportCandidateGroups([bobMarley, other], "youtube");

  assert.deepEqual(initial.map((group) => group.id), [
    "search:youtube:peter tosh legalize it",
    "search:youtube:bob marley one love",
  ]);
  assert.deepEqual(reordered.map((group) => group.id), [
    "search:youtube:bob marley one love",
    "search:youtube:peter tosh legalize it",
  ]);
  assert.deepEqual(reordered.map((group) => group.searchIndex), [1, 2]);
});

test("maps import relevance to the requested color thresholds", () => {
  assert.deepEqual(
    [0, 0.244, 0.25, 0.494, 0.5, 0.744, 0.75, 0.994, 1].map(importRelevanceLevel),
    [0, 0, 1, 1, 2, 2, 3, 3, 4],
  );
  assert.equal(importRelevancePercent(-1), 0);
  assert.equal(importRelevancePercent(2), 100);
  assert.equal(importRelevancePercent(Number.NaN), 0);
});
