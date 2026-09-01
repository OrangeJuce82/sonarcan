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
  assert.equal(translate("de", "preferences"), "Präferenzen");
  assert.equal(translate("pt", "playlist"), "Lista de reprodução");
  assert.equal(translate("it", "close"), "Chiudi");
  assert.equal(translate("zh", "preferences"), "偏好设置");
  assert.equal(translate("ja", "close"), "閉じる");
  assert.equal(translate("ko", "preferences"), "환경설정");
  assert.equal(translate("ar", "close"), "إغلاق");
  assert.equal(translate("hi", "preferences"), "प्राथमिकताएँ");
  assert.equal(translate("id", "close"), "Tutup");
  assert.equal(languageDirection("ar"), "rtl");
  assert.equal(languageDirection("ja"), "ltr");
});

test("recent preference actions are translated in every supported language", () => {
  for (const language of languages) {
    assert.ok(translate(language, "resetPreferences").trim().length > 0, language);
    assert.ok(translate(language, "resetTrainingDefaults").trim().length > 0, language);
    assert.ok(translate(language, "loopSnap").trim().length > 0, language);
  }
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
