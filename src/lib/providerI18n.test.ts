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
  }
});

test("French provider controls are explicit", () => {
  assert.equal(providerTranslate("fr", "searchProvider"), "Fournisseur de recherche");
});
