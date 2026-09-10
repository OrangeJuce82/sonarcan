import type { Language } from "./i18n";

const en = {
  addChord: "Add chord at playhead",
  addLyrics: "Add lyrics at playhead",
  editMarker: "Double-click to edit marker",
  editChord: "Double-click to edit chord",
  editLyrics: "Double-click to edit lyrics",
  deleteChord: "Delete chord",
  deleteLyrics: "Delete lyrics",
} as const;

type TimelineMessageKey = keyof typeof en;
type TimelineCatalog = Record<TimelineMessageKey, string>;

const fr: TimelineCatalog = {
  addChord: "Ajouter un accord à la tête de lecture",
  addLyrics: "Ajouter des paroles à la tête de lecture",
  editMarker: "Double-cliquer pour modifier le marqueur",
  editChord: "Double-cliquer pour modifier l’accord",
  editLyrics: "Double-cliquer pour modifier les paroles",
  deleteChord: "Supprimer l’accord",
  deleteLyrics: "Supprimer les paroles",
};

const catalogs: Record<Language, TimelineCatalog> = {
  en,
  fr,
  es: en,
  de: en,
  pt: en,
  it: en,
  zh: en,
  ja: en,
  ko: en,
  ar: en,
  hi: en,
  id: en,
};

export const timelineTranslate = (language: Language, key: TimelineMessageKey): string => catalogs[language][key];
