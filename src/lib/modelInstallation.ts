import type { Language } from "./i18n";

export interface ModelInstallProgress {
  modelId: string;
  modelName: string;
  stage: "checking" | "downloading" | "verified" | "complete";
  progress: number;
  completedBytes: number;
  totalBytes: number;
  modelIndex: number;
  modelCount: number;
}

export interface ModelInstallResult {
  installed: boolean;
  modelCount: number;
}

export interface ModelInstallationCopy {
  title: string;
  introduction: string;
  checking: string;
  downloading: string;
  verified: string;
  complete: string;
  failure: string;
  retry: string;
  quit: string;
}

const english: ModelInstallationCopy = {
  title: "Preparing SonArcan",
  introduction: "The analysis models are installed once and then remain available offline.",
  checking: "Checking",
  downloading: "Downloading",
  verified: "Verified",
  complete: "Models ready",
  failure: "The models could not be installed. Check your connection and available disk space.",
  retry: "Retry",
  quit: "Quit",
};

const french: ModelInstallationCopy = {
  title: "Préparation de SonArcan",
  introduction: "Les modèles d’analyse sont installés une seule fois, puis restent disponibles hors connexion.",
  checking: "Vérification de",
  downloading: "Téléchargement de",
  verified: "Modèle vérifié :",
  complete: "Modèles prêts",
  failure: "Les modèles n’ont pas pu être installés. Vérifiez la connexion et l’espace disque disponible.",
  retry: "Réessayer",
  quit: "Quitter",
};

const incompatibleHardwareEnglish: ModelInstallationCopy = {
  ...english,
  title: "Incompatible hardware",
  introduction: "SonArcan requires Apple Silicon, or Linux with a supported NVIDIA GPU, proprietary driver, and compatible CUDA runtime.",
  failure: "No supported GPU runtime is available. SonArcan has no CPU mode, so the application is blocked before the workspace opens.",
};

const incompatibleHardwareFrench: ModelInstallationCopy = {
  ...french,
  title: "Matériel incompatible",
  introduction: "SonArcan exige Apple Silicon, ou Linux avec un GPU NVIDIA pris en charge, son pilote propriétaire et un runtime CUDA compatible.",
  failure: "Aucun runtime GPU pris en charge n’est disponible. SonArcan n’a pas de mode CPU : l’application est bloquée avant l’ouverture de l’espace de travail.",
};

export function modelInstallationCopy(language: Language, incompatibleHardware = false): ModelInstallationCopy {
  if (incompatibleHardware) return language === "fr" ? incompatibleHardwareFrench : incompatibleHardwareEnglish;
  return language === "fr" ? french : english;
}

export function modelInstallationMessage(
  progress: ModelInstallProgress,
  copy: ModelInstallationCopy,
): string {
  if (progress.stage === "complete") return copy.complete;
  const prefix = progress.stage === "downloading"
    ? copy.downloading
    : progress.stage === "verified"
      ? copy.verified
      : copy.checking;
  return `${prefix} ${progress.modelName}…`;
}

export function formatInstallBytes(bytes: number): string {
  return `${(Math.max(0, bytes) / (1024 * 1024)).toFixed(0)} Mo`;
}
