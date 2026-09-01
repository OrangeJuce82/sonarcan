type ShortcutKeyboardEvent = Pick<
  KeyboardEvent,
  "altKey" | "ctrlKey" | "isComposing" | "key" | "metaKey" | "shiftKey" | "target"
>;

type ClosestTarget = EventTarget & {
  closest?: (selectors: string) => unknown;
};

export type ParameterShortcut = "metronomeVolume" | "pitch" | "tempo" | "zoom";
export type ParameterShortcutAction = "decrement" | "increment" | "reset";
export type MetronomeShortcutAction = "decrementVolume" | "incrementVolume" | "nextSound" | "previousSound" | "resetVolume";
export type ShortcutPlatform = "linux" | "macos" | "windows";

export function shortcutPlatformFor(platform: string, userAgent = ""): ShortcutPlatform {
  const identity = `${platform} ${userAgent}`.toLowerCase();
  if (identity.includes("mac") || identity.includes("iphone") || identity.includes("ipad")) return "macos";
  if (identity.includes("win")) return "windows";
  return "linux";
}

export function shortcutKeyLabels(platform: ShortcutPlatform): {
  backspace: string;
  delete: string;
  space: string;
} {
  return platform === "macos"
    ? { backspace: "⌫", delete: "Fn ⌫", space: "Space" }
    : { backspace: "Backspace", delete: "Del", space: "Space" };
}

export function parameterShortcutForKey(key: string): ParameterShortcut | null {
  if (key.toLowerCase() === "t") return "tempo";
  if (key.toLowerCase() === "p") return "pitch";
  if (key.toLowerCase() === "z") return "zoom";
  if (key.toLowerCase() === "m") return "metronomeVolume";
  return null;
}

export function parameterShortcutAction(key: string): ParameterShortcutAction | null {
  if (key === "ArrowUp" || key === "ArrowRight" || key === "+" || key === "=") return "increment";
  if (key === "ArrowDown" || key === "ArrowLeft" || key === "-" || key === "_") return "decrement";
  if (key === "Delete" || key === "Backspace") return "reset";
  return null;
}

export function metronomeShortcutAction(key: string): MetronomeShortcutAction | null {
  if (key === "ArrowUp") return "nextSound";
  if (key === "ArrowDown") return "previousSound";
  if (key === "ArrowRight" || key === "+" || key === "=") return "incrementVolume";
  if (key === "ArrowLeft" || key === "-" || key === "_") return "decrementVolume";
  if (key === "Delete" || key === "Backspace") return "resetVolume";
  return null;
}

export function isTextEditingTarget(target: EventTarget | null): boolean {
  const candidate = target as ClosestTarget | null;
  return Boolean(candidate?.closest?.(
    "input, textarea, select, [contenteditable]:not([contenteditable='false']), [role='textbox']",
  ));
}

export function isTextEntryTarget(target: EventTarget | null): boolean {
  const candidate = target as ClosestTarget | null;
  return Boolean(candidate?.closest?.(
    "textarea, [contenteditable]:not([contenteditable='false']), [role='textbox'], input:not([type]), input[type='text'], input[type='search'], input[type='email'], input[type='url'], input[type='tel'], input[type='password'], input[type='number']",
  ));
}

export function shouldHandlePlayPauseShortcut(event: ShortcutKeyboardEvent): boolean {
  return event.key === " "
    && !event.isComposing
    && !event.altKey
    && !event.ctrlKey
    && !event.metaKey
    && !isTextEntryTarget(event.target);
}

export function shouldHandleParameterShortcut(event: ShortcutKeyboardEvent): boolean {
  return !event.isComposing
    && !event.altKey
    && !event.ctrlKey
    && !event.metaKey
    && !isTextEntryTarget(event.target);
}

export function shouldHandleGlobalShortcut(event: ShortcutKeyboardEvent): boolean {
  return !event.isComposing
    && !event.altKey
    && !event.ctrlKey
    && !event.metaKey
    && !isTextEditingTarget(event.target);
}

export function shouldToggleMetronomeOnRelease(
  event: ShortcutKeyboardEvent,
  activeParameterShortcut: ParameterShortcut | null,
  parameterActionUsed: boolean,
): boolean {
  return activeParameterShortcut === "metronomeVolume"
    && parameterShortcutForKey(event.key) === "metronomeVolume"
    && !parameterActionUsed
    && shouldHandleParameterShortcut(event);
}

export function shouldBlurFocusedSelect(
  activeElement: Pick<Element, "tagName"> | null,
  pointerInsideFocusRegion: boolean,
): boolean {
  return activeElement?.tagName === "SELECT" && !pointerInsideFocusRegion;
}
