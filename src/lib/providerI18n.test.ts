import assert from "node:assert/strict";
import test from "node:test";
import { languages } from "./i18n.ts";
import { providerTranslate } from "./providerI18n.ts";

test("every language labels provider search and source links", () => {
  for (const language of languages) {
    assert.ok(providerTranslate(language, "searchProvider").trim());
    assert.ok(providerTranslate(language, "openSource").trim());
    assert.ok(providerTranslate(language, "directOnly").trim());
    assert.ok(providerTranslate(language, "autoBest").trim());
    assert.ok(providerTranslate(language, "importHelp").trim());
    assert.ok(providerTranslate(language, "poweredByYtDlp").trim());
    assert.ok(providerTranslate(language, "musicSources").trim());
    assert.ok(providerTranslate(language, "allYtDlpSites").trim());
    assert.ok(providerTranslate(language, "noPublicFormat").trim());
  }
});

test("French provider controls are explicit", () => {
  assert.equal(providerTranslate("fr", "searchProvider"), "Fournisseur de recherche");
  assert.equal(providerTranslate("fr", "poweredByYtDlp"), "Propulsé par yt-dlp");
  assert.match(providerTranslate("fr", "importHelp"), /playlists/);
  assert.match(providerTranslate("fr", "noPublicFormat"), /DRM/);
});

test("import help and yt-dlp credit are translated in every language", () => {
  for (const language of languages.filter((language) => language !== "en")) {
    assert.notEqual(providerTranslate(language, "importHelp"), providerTranslate("en", "importHelp"));
    assert.notEqual(providerTranslate(language, "poweredByYtDlp"), providerTranslate("en", "poweredByYtDlp"));
    assert.notEqual(providerTranslate(language, "musicSources"), providerTranslate("en", "musicSources"));
    assert.notEqual(providerTranslate(language, "allYtDlpSites"), providerTranslate("en", "allYtDlpSites"));
  }
});
