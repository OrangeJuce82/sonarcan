import type { Language } from "./i18n";

export interface AboutCopy {
  title: string;
  description: string;
  localFirst: string;
  toolsTitle: string;
  toolsDescription: string;
  audioTools: string;
  analysisTools: string;
  desktopTools: string;
  licensesTitle: string;
  licensesDescription: string;
  sourceCode: string;
  reportIssue: string;
  thirdPartyNotices: string;
  thanksTitle: string;
  thanksDescription: string;
  close: string;
}

const english: AboutCopy = {
  title: "About SonArcan",
  description: "A local-first workspace to analyze, isolate, and rehearse music.",
  localFirst: "Projects, playback, and audio analysis stay on your Mac.",
  toolsTitle: "Tools",
  toolsDescription: "SonArcan brings together open-source audio, analysis, and desktop technologies.",
  audioTools: "Audio · CPAL, Symphonia, Signalsmith Stretch, FFmpeg, LAME",
  analysisTools: "Analysis · LV-Chordia, Beat This!, madmom, MLX, HTDemucs, SCNet Large",
  desktopTools: "Application · Rust, Tauri, Svelte, Python, yt-dlp, LRCLIB",
  licensesTitle: "Open source and licenses",
  licensesDescription: "SonArcan is released under the MIT License. Bundled tools, models, and data retain their own licenses.",
  sourceCode: "Source code",
  reportIssue: "Report an issue",
  thirdPartyNotices: "Third-party notices",
  thanksTitle: "Thanks",
  thanksDescription: "Maintained by OrangeJuce82. Thank you to every SonArcan contributor and to the maintainers, researchers, and communities behind the open-source projects above.",
  close: "Close",
};

const french: AboutCopy = {
  title: "À propos de SonArcan",
  description: "Un espace de travail local pour analyser, isoler et répéter la musique.",
  localFirst: "Les projets, la lecture et l’analyse audio restent sur votre Mac.",
  toolsTitle: "Outils",
  toolsDescription: "SonArcan réunit des technologies open source dédiées à l’audio, à l’analyse et aux applications de bureau.",
  audioTools: "Audio · CPAL, Symphonia, Signalsmith Stretch, FFmpeg, LAME",
  analysisTools: "Analyse · LV-Chordia, Beat This!, madmom, MLX, HTDemucs, SCNet Large",
  desktopTools: "Application · Rust, Tauri, Svelte, Python, yt-dlp, LRCLIB",
  licensesTitle: "Open source et licences",
  licensesDescription: "SonArcan est distribué sous licence MIT. Les outils, modèles et données intégrés conservent leurs propres licences.",
  sourceCode: "Code source",
  reportIssue: "Signaler un problème",
  thirdPartyNotices: "Licences tierces",
  thanksTitle: "Remerciements",
  thanksDescription: "SonArcan est maintenu par OrangeJuce82. Merci à chaque personne ayant contribué à SonArcan, ainsi qu’aux mainteneurs, équipes de recherche et communautés des projets open source cités ci-dessus.",
  close: "Fermer",
};

export function aboutCopy(language: Language): AboutCopy {
  if (language === "fr") return french;
  const localizedLabels: Partial<Record<Language, Pick<AboutCopy, "title" | "close">>> = {
    es: { title: "Acerca de SonArcan", close: "Cerrar" },
    de: { title: "Über SonArcan", close: "Schließen" },
    pt: { title: "Sobre SonArcan", close: "Fechar" },
    it: { title: "A proposito di SonArcan", close: "Chiudi" },
    zh: { title: "关于 SonArcan", close: "关闭" },
    ja: { title: "SonArcan について", close: "閉じる" },
    ko: { title: "SonArcan 소개", close: "닫기" },
    ar: { title: "حول SonArcan", close: "إغلاق" },
    hi: { title: "SonArcan के बारे में", close: "बंद करें" },
    id: { title: "Tentang SonArcan", close: "Tutup" },
  };
  return { ...english, ...localizedLabels[language] };
}
