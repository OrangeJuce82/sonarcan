import assert from "node:assert/strict";
import test from "node:test";
import { nearestTimelineSnapPosition, snappedTimelineRange, timelineBoundaryPoints, type TimelineLaneItem } from "./timelineLane.ts";

const items: TimelineLaneItem[] = [
  { id: "first", label: "First", startSeconds: 2, endSeconds: 6 },
  { id: "middle", label: "Middle", startSeconds: 9, endSeconds: 12 },
  { id: "last", label: "Last", startSeconds: 15, endSeconds: 18 },
];

test("double-clicking a timeline edge snaps to its adjacent block", () => {
  assert.deepEqual(snappedTimelineRange(items, "middle", "start", 20), { startSeconds: 6, endSeconds: 12 });
  assert.deepEqual(snappedTimelineRange(items, "middle", "end", 20), { startSeconds: 9, endSeconds: 15 });
});

test("outer timeline edges snap to the track boundaries", () => {
  assert.deepEqual(snappedTimelineRange(items, "first", "start", 20), { startSeconds: 0, endSeconds: 6 });
  assert.deepEqual(snappedTimelineRange(items, "last", "end", 20), { startSeconds: 15, endSeconds: 20 });
});

test("cross-lane snap points include starts and ends in time order", () => {
  assert.deepEqual(timelineBoundaryPoints(
    [{ startSeconds: 8, endSeconds: 12 }],
    [{ startSeconds: 2, endSeconds: 8 }],
  ), [2, 8, 12]);
});

test("Shift snapping uses the closest cross-lane boundary within its visual tolerance", () => {
  assert.equal(nearestTimelineSnapPosition(7.92, [2, 8, 12], 0.1), 8);
  assert.equal(nearestTimelineSnapPosition(7.7, [2, 8, 12], 0.1), 7.7);
});
