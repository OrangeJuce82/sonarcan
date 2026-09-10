import assert from "node:assert/strict";
import test from "node:test";
import { languages } from "./i18n.ts";
import { markerTranslate } from "./markersI18n.ts";

test("every supported language translates the marker workflow", () => {
  for (const language of languages) {
    for (const key of ["markers", "add", "loop", "source", "detected", "user", "navigation", "previous", "next", "loopSnap"] as const) {
      assert.ok(markerTranslate(language, key).trim());
    }
  }
  assert.equal(markerTranslate("fr", "markers"), "Marqueurs");
  assert.equal(markerTranslate("fr", "navigation"), "Marqueur");
});
