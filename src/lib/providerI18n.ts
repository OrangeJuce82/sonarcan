import type { Language } from "./i18n";

const en = { searchProvider: "Search provider", openSource: "Open on provider", directOnly: "Direct links", autoBest: "Automatically select the most relevant search result" } as const;
type Key = keyof typeof en;
type Catalog = Record<Key, string>;
const fr: Catalog = { searchProvider: "Fournisseur de recherche", openSource: "Ouvrir chez le fournisseur", directOnly: "Liens directs", autoBest: "Sélectionner automatiquement le résultat le plus pertinent" };
const overrides: Partial<Record<Language, Partial<Catalog>>> = {
  es: { searchProvider: "Proveedor de búsqueda", openSource: "Abrir en el proveedor", directOnly: "Enlaces directos" },
  de: { searchProvider: "Suchanbieter", openSource: "Beim Anbieter öffnen", directOnly: "Direktlinks" },
  pt: { searchProvider: "Provedor de pesquisa", openSource: "Abrir no provedor", directOnly: "Links diretos" },
  it: { searchProvider: "Fornitore di ricerca", openSource: "Apri nel fornitore", directOnly: "Link diretti" },
  zh: { searchProvider: "搜索提供商", openSource: "在提供商中打开", directOnly: "直接链接" },
  ja: { searchProvider: "検索プロバイダー", openSource: "プロバイダーで開く", directOnly: "直接リンク" },
  ko: { searchProvider: "검색 제공자", openSource: "제공자에서 열기", directOnly: "직접 링크" },
  ar: { searchProvider: "مزود البحث", openSource: "فتح لدى المزود", directOnly: "روابط مباشرة" },
  hi: { searchProvider: "खोज प्रदाता", openSource: "प्रदाता पर खोलें", directOnly: "सीधे लिंक" },
  id: { searchProvider: "Penyedia pencarian", openSource: "Buka di penyedia", directOnly: "Tautan langsung" },
};
export const providerTranslate = (language: Language, key: Key): string => language === "fr" ? fr[key] : overrides[language]?.[key] ?? en[key];
