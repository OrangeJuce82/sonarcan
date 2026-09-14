import type { Language } from "./i18n";

interface LogCopyText {
  label: string;
  success: string;
  failure: string;
}

const text: Record<Language, LogCopyText> = {
  en: { label: "Copy filtered logs", success: "Logs copied", failure: "Could not copy logs" },
  fr: { label: "Copier les logs filtrés", success: "Logs copiés", failure: "Impossible de copier les logs" },
  es: { label: "Copiar registros filtrados", success: "Registros copiados", failure: "No se pudieron copiar los registros" },
  de: { label: "Gefilterte Protokolle kopieren", success: "Protokolle kopiert", failure: "Protokolle konnten nicht kopiert werden" },
  pt: { label: "Copiar registos filtrados", success: "Registos copiados", failure: "Não foi possível copiar os registos" },
  it: { label: "Copia i registri filtrati", success: "Registri copiati", failure: "Impossibile copiare i registri" },
  zh: { label: "复制筛选后的日志", success: "日志已复制", failure: "无法复制日志" },
  ja: { label: "絞り込んだログをコピー", success: "ログをコピーしました", failure: "ログをコピーできませんでした" },
  ko: { label: "필터링된 로그 복사", success: "로그를 복사했습니다", failure: "로그를 복사할 수 없습니다" },
  ar: { label: "نسخ السجلات المصفّاة", success: "تم نسخ السجلات", failure: "تعذر نسخ السجلات" },
  hi: { label: "फ़िल्टर किए गए लॉग कॉपी करें", success: "लॉग कॉपी किए गए", failure: "लॉग कॉपी नहीं किए जा सके" },
  id: { label: "Salin log yang difilter", success: "Log disalin", failure: "Log tidak dapat disalin" },
};

export const logCopyText = (language: Language): LogCopyText => text[language];
