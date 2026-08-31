type ShortcutKeyboardEvent = Pick<
  KeyboardEvent,
  "altKey" | "ctrlKey" | "isComposing" | "key" | "metaKey" | "shiftKey" | "target"
>;

type ClosestTarget = EventTarget & {
  closest?: (selectors: string) => unknown;
};

export type ParameterShortcut = "metronomeVolume" | "pitch" | "tempo" | "zoom";
export type ParameterShortcutAction = "decrement" | "increment" | "reset";
export type ShortcutPlatform = "linux" | "macos" | "windows";

export function shortcutPlatformFor(platform: string, userAgent = ""): ShortcutPlatform {
  const identity = `${platform} ${userAgent}`.toLowerCase();
  if (identity.includes("mac") || identity.includes("iphone") || identity.includes("ipad")) return "macos";
  if (identity.includes("win")) return "windows";
  return "linux";
}

export function shortcutKeyLabels(platform: ShortcutPlatform): {
  alt: string;
  backspace: string;
  delete: string;
  shift: string;
  space: string;
} {
  return platform === "macos"
    ? { alt: "⌥", backspace: "⌫", delete: "Fn ⌫", shift: "⇧", space: "Space" }
    : { alt: "Alt", backspace: "Backspace", delete: "Del", shift: "Shift", space: "Space" };
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

export function shouldHandleChordNavigationShortcut(event: ShortcutKeyboardEvent): boolean {
  return !event.isComposing
    && event.altKey
    && !event.ctrlKey
    && !event.metaKey
    && !event.shiftKey
    && (event.key === "ArrowLeft" || event.key === "ArrowRight")
    && !isTextEditingTarget(event.target);
}

export function shouldBlurFocusedSelect(
  activeElement: Pick<Element, "tagName"> | null,
  pointerInsideFocusRegion: boolean,
): boolean {
  return activeElement?.tagName === "SELECT" && !pointerInsideFocusRegion;
}
