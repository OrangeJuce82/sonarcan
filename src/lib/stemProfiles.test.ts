import test from "node:test";
import assert from "node:assert/strict";
import { preferredStemProfile } from "./stemProfiles.ts";

test("the last used cached profile wins before the HQ fallback", () => {
  assert.equal(preferredStemProfile(["hq", "fast"], "fast"), "fast");
  assert.equal(preferredStemProfile(["hq", "fast"], null), "hq");
  assert.equal(preferredStemProfile(["fast"], "hq"), "fast");
  assert.equal(preferredStemProfile([], "hq"), null);
});
