import test from "node:test";
import assert from "node:assert/strict";
import { activeLyricsLineIndex, activeLyricsWordIndex, estimatedLyricsLineIndex, lrclibDocument, lyricsEditorContent, lyricsNavigationPositions, LyricsParseError, parseLyrics } from "./lyrics.ts";

test("parses and follows line-synchronized LRC", () => {
  const document = parseLyrics("[00:01.00]Première\n[00:03.50]Deuxième", "fr", 5_000);
  assert.equal(document.syncLevel, "line");
  assert.equal(document.lines[0].endMs, 3_500);
  assert.equal(activeLyricsLineIndex(document, 3_600), 1);
});

test("parses enhanced LRC word timings", () => {
  const document = parseLyrics("[00:01.00]<00:01.00>Bon <00:01.50>jour", "fr", 3_000);
  assert.equal(document.syncLevel, "word");
  assert.equal(document.lines[0].text, "Bon jour");
  assert.equal(activeLyricsWordIndex(document.lines[0], 1_600, 0), 1);
});

test("parses Apple-style TTML paragraphs and spans", () => {
  const document = parseLyrics(`<tt><body><div><p begin="00:00:02.000" end="00:00:04.000"><span begin="2s" end="3s">Hello </span><span begin="3s">world</span></p></div></body></tt>`, "en");
  assert.equal(document.syncLevel, "word");
  assert.equal(document.lines[0].text, "Hello world");
  assert.equal(document.lines[0].startMs, 2_000);
});

test("keeps plain text as unsynchronized lyrics", () => {
  const document = parseLyrics("Verse one\nVerse two", "en");
  assert.equal(document.syncLevel, "none");
  assert.equal(document.lines.length, 2);
});

test("estimates unsynchronized lyric progress without making it navigable", () => {
  const document = parseLyrics("One\nTwo\nThree\nFour", "en");
  assert.equal(estimatedLyricsLineIndex(document, 0, 40_000), 0);
  assert.equal(estimatedLyricsLineIndex(document, 21_000, 40_000), 2);
  assert.equal(estimatedLyricsLineIndex(document, 40_000, 40_000), 3);
  assert.deepEqual(lyricsNavigationPositions(document, 40), []);
});

test("rejects malformed LRC timestamps instead of treating them as plain text", () => {
  const malformed = "[01:dsdqsdqsdqsdqsdqsd58.91]Oh, it's such a perfect day\n[01:10qsdqsdqsd.66]You just keep me hanging on";
  assert.throws(() => parseLyrics(malformed, "en"), (error) => error instanceof LyricsParseError && error.code === "invalidTimestamp");
  assert.throws(() => parseLyrics("[01:60.00]Invalid seconds", "en"), LyricsParseError);
  assert.throws(() => parseLyrics("[01:10.00]<01:word>Broken word sync", "en"), LyricsParseError);
});

test("rejects line and word timestamps outside the audio duration", () => {
  assert.throws(
    () => parseLyrics("[00:11.00]Past the end", "en", 10_000),
    (error) => error instanceof LyricsParseError && error.code === "timestampOutOfRange",
  );
  assert.throws(
    () => parseLyrics("[00:01.00]<00:12.00>Past the end", "en", 10_000),
    (error) => error instanceof LyricsParseError && error.code === "timestampOutOfRange",
  );
});

test("serializes lyrics and selects the active line for inline editing", () => {
  const document = parseLyrics("[00:01.00]First\n[00:03.50]Second", "en");
  const editor = lyricsEditorContent(document, 1);
  assert.equal(editor.text.slice(editor.selectionStart, editor.selectionEnd), "[00:03.50]Second");
});

test("converts an LRCLIB response into a locally persistable provider document", () => {
  const document = lrclibDocument({
    id: 42, trackName: "Song", artistName: "Artist", albumName: "Album", durationSeconds: 3,
    instrumental: false, hasSyncedLyrics: true, hasPlainLyrics: true,
    syncedLyrics: "[00:01.00]Line", plainLyrics: "Line",
  }, "en");
  assert.equal(document.provider, "lrclib");
  assert.equal(document.providerTrackId, "42");
  assert.equal(document.syncLevel, "line");
});

test("builds bounded lyric navigation points with the display offset", () => {
  const document = parseLyrics("[00:01.00]One\n[00:03.00]Two", "en", 5_000);
  document.offsetMs = 200;
  assert.deepEqual(lyricsNavigationPositions(document, 5), [1.2, 3.2]);
  document.offsetMs = -2_000;
  assert.deepEqual(lyricsNavigationPositions(document, 5), [1]);
});
