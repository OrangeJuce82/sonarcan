type DialogKeyboardEvent = Pick<
  KeyboardEvent,
  "altKey" | "ctrlKey" | "isComposing" | "key" | "metaKey" | "repeat" | "shiftKey" | "target"
>;

type ClosestTarget = EventTarget & {
  closest?: (selectors: string) => unknown;
};

export function shouldConfirmDialogOnEnter(event: DialogKeyboardEvent, enabled: boolean): boolean {
  if (!enabled || event.key !== "Enter" || event.repeat || event.isComposing) return false;
  if (event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return false;
  const target = event.target as ClosestTarget | null;
  return !target?.closest?.("textarea, [contenteditable='true']");
}
