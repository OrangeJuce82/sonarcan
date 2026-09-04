import assert from "node:assert/strict";
import test from "node:test";
import { languages } from "./i18n.ts";
import { lyricsTranslate } from "./lyricsI18n.ts";

test("every supported language translates lyrics navigation", () => {
  for (const language of languages) {
    assert.ok(lyricsTranslate(language, "navigationLyrics").trim());
    assert.ok(lyricsTranslate(language, "previousLine").trim());
    assert.ok(lyricsTranslate(language, "nextLine").trim());
    assert.ok(lyricsTranslate(language, "navigationPending").trim());
    assert.ok(lyricsTranslate(language, "loopSnapLyricsHelp").trim());
  }
  assert.equal(lyricsTranslate("fr", "navigationLyrics"), "Paroles");
  assert.equal(lyricsTranslate("ar", "nextLine"), "سطر الكلمات التالي");
});
