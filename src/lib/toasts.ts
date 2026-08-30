export type ToastLevel = "info" | "success" | "warn" | "error";

export interface ToastMessage {
  id: number;
  level: ToastLevel;
  title: string;
  detail?: string;
}

export function appendToast(
  current: ToastMessage[],
  next: ToastMessage,
  limit = 3,
): ToastMessage[] {
  const withoutDuplicate = current.filter(
    (toast) => toast.level !== next.level
      || toast.title !== next.title
      || toast.detail !== next.detail,
  );
  return [...withoutDuplicate, next].slice(-Math.max(1, limit));
}
