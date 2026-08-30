import assert from "node:assert/strict";
import test from "node:test";
import { languageDirection, languages, messageCatalogs, translate } from "./i18n.ts";

test("every supported language has the complete message catalog", () => {
  const englishKeys = Object.keys(messageCatalogs.en).sort();
  assert.equal(languages.length, 12);
  for (const language of languages) {
    assert.deepEqual(Object.keys(messageCatalogs[language]).sort(), englishKeys, language);
    for (const key of englishKeys) {
      assert.notEqual(messageCatalogs[language][key as keyof typeof messageCatalogs.en].trim(), "", `${language}.${key}`);
    }
  }
});

test("translations and writing direction follow the selected locale", () => {
  assert.equal(translate("es", "close"), "Cerrar");
  assert.equal(languageDirection("ar"), "rtl");
  assert.equal(languageDirection("ja"), "ltr");
});

test("product and technical names remain stable in every catalog", () => {
  for (const language of languages) {
    for (const key of ["createProjectFile", "saveProjectFile", "openProject", "openGithub", "supportProject"] as const) {
      assert.match(translate(language, key), /SonArcan/, `${language}.${key}`);
    }
    assert.match(translate(language, "exportJson"), /JSON/, `${language}.exportJson`);
    assert.match(translate(language, "exportMarkdown"), /Markdown/, `${language}.exportMarkdown`);
  }
});

test("detected BPM help never advertises the removed editor", () => {
  for (const language of languages) {
    assert.match(translate(language, "bpmEstimateHelp"), /Beat This!/, language);
  }
});
