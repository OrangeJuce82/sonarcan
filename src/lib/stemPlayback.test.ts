import assert from "node:assert/strict";
import test from "node:test";

import { shouldResumeStemPlayback, stemPlaybackResumeRequest } from "./stemPlayback.ts";

test("stem generation remembers playback only when it was requested", () => {
  assert.equal(stemPlaybackResumeRequest(false, "track-a", 4), null);
  assert.deepEqual(stemPlaybackResumeRequest(true, "track-a", 4), {
    trackId: "track-a",
    selectionGeneration: 4,
  });
});

test("stem completion resumes only the same selected track generation", () => {
  const request = stemPlaybackResumeRequest(true, "track-a", 4);
  assert.equal(shouldResumeStemPlayback(request, "track-a", 4), true);
  assert.equal(shouldResumeStemPlayback(request, "track-b", 4), false);
  assert.equal(shouldResumeStemPlayback(request, "track-a", 5), false);
  assert.equal(shouldResumeStemPlayback(null, "track-a", 4), false);
});
