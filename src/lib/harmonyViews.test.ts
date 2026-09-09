import assert from "node:assert/strict";
import test from "node:test";

import { nextHarmonyView } from "./harmonyViews.ts";

test("the harmony shortcut cycles through instruments and lyrics", () => {
  assert.equal(nextHarmonyView("piano"), "guitar");
  assert.equal(nextHarmonyView("guitar"), "ukulele");
  assert.equal(nextHarmonyView("ukulele"), "lyrics");
  assert.equal(nextHarmonyView("lyrics"), "piano");
});
