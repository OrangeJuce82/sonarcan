import assert from "node:assert/strict";
import test from "node:test";
import { activeMarkerIndexAt, insertMarkerAt, markerEndSeconds, markerViewportBlocks, replaceMarker } from "./markers.ts";
import type { TrackMarker } from "./types.ts";

const markers: TrackMarker[] = [
  { id: "intro", label: "Intro", startSeconds: 0, endSeconds: 12, origin: "source" },
  { id: "verse", label: "Verse", startSeconds: 12, origin: "user" },
  { id: "chorus", label: "Chorus", startSeconds: 32, endSeconds: 48, origin: "detected" },
];

test("open markers end at the following marker or track end", () => {
  assert.equal(markerEndSeconds(markers, 1, 60), 32);
  assert.equal(markerEndSeconds(markers, 2, 60), 48);
});

test("an explicit marker end is clipped at the next marker", () => {
  const overlapping = [
    { ...markers[0], endSeconds: 30 },
    { ...markers[1], startSeconds: 12 },
  ];
  assert.equal(markerEndSeconds(overlapping, 0, 60), 12);
});

test("marker viewport blocks clip regions without changing their loop bounds", () => {
  const blocks = markerViewportBlocks(markers, 60, 0.25, 2);
  assert.deepEqual(blocks.map((block) => block.marker.id), ["verse", "chorus"]);
  assert.equal(blocks[0].startSeconds, 12);
  assert.equal(blocks[0].endSeconds, 32);
  assert.equal(Math.round(blocks[0].leftPercent), 0);
  assert.equal(Math.round(blocks[1].widthPercent), 43);
});

test("active markers use half-open ranges and the latest overlapping marker", () => {
  assert.equal(activeMarkerIndexAt(markers, 11.99, 60), 0);
  assert.equal(activeMarkerIndexAt(markers, 12, 60), 1);
  assert.equal(activeMarkerIndexAt(markers, 33, 60), 2);
  assert.equal(activeMarkerIndexAt(markers, 55, 60), -1);
});

test("editing a marker keeps the timeline sorted", () => {
  const changed = replaceMarker(markers, { ...markers[2], startSeconds: 5, endSeconds: 9 });
  assert.deepEqual(changed.map((marker) => marker.id), ["intro", "chorus", "verse"]);
});

test("inserting a marker inside another shortens the preceding marker", () => {
  const inserted = insertMarkerAt(
    [{ id: "first", label: "First", startSeconds: 0, endSeconds: 30, origin: "user" }],
    { id: "second", label: "Second", startSeconds: 12, endSeconds: 30, origin: "user" },
    30,
  );
  assert.equal(inserted[0].endSeconds, 12);
  assert.equal(markerEndSeconds(inserted, 0, 30), 12);
});
