export type HarmonyView = "piano" | "guitar" | "ukulele" | "lyrics";

const HARMONY_VIEWS: readonly HarmonyView[] = ["piano", "guitar", "ukulele", "lyrics"];

export function nextHarmonyView(view: HarmonyView): HarmonyView {
  const index = HARMONY_VIEWS.indexOf(view);
  return HARMONY_VIEWS[(index + 1) % HARMONY_VIEWS.length] ?? "piano";
}
