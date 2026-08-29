type ShortcutKeyboardEvent = Pick<
  KeyboardEvent,
  "altKey" | "ctrlKey" | "isComposing" | "metaKey" | "target"
>;

type ClosestTarget = EventTarget & {
  closest?: (selectors: string) => unknown;
};

export function isTextEditingTarget(target: EventTarget | null): boolean {
  const candidate = target as ClosestTarget | null;
  return Boolean(candidate?.closest?.(
    "input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox']",
  ));
}

export function shouldHandleGlobalShortcut(event: ShortcutKeyboardEvent): boolean {
  return !event.isComposing
    && !event.altKey
    && !event.ctrlKey
    && !event.metaKey
    && !isTextEditingTarget(event.target);
}
