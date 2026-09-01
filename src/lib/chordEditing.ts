import { parseChordLabel } from "./chordNotes.ts";
import { presentChordLabel, type ChordAccidentalMode } from "./chordViews.ts";
import type { ChordEdit, ChordMode, TimedChord } from "./types.ts";

// Compact equivalents of LV-Chordia's shared Standard/submission vocabulary.
const COMMON_CHORD_TEMPLATES = [
  "C", "Cm", "C7", "Cmaj7", "Cm7", "Cdim", "Cdim7", "Cm7b5", "Caug",
  "Csus2", "Csus4", "Csus4(b7)", "C9", "Cmaj9", "Cm9", "C11", "C13",
  "C/E", "C/G", "C/Bb", "C/D", "Cm/Eb", "Cm/G", "Cm/Bb", "Cm/D",
] as const;

const NATURAL_PITCHES: Readonly<Record<string, number>> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };

export type ChordEditKeyboardAction = "cancel" | "commit";
export type ChordEditPointerTarget = "editor" | "option" | "outside";

export function chordEditPointerAction(
  target: ChordEditPointerTarget,
  button: number,
  shiftKey: boolean,
): "cancel" | "commit" | "commitAll" | null {
  if (button !== 0) return null;
  if (target === "outside") return "cancel";
  if (target === "option") return shiftKey ? "commitAll" : "commit";
  return null;
}

export function shouldSeekChordFromClick(altKey: boolean): boolean {
  return altKey;
}

export function centeredChordOptionScrollTop(
  selectedIndex: number,
  optionCount: number,
  viewportHeight: number,
  scrollHeight: number,
): number {
  if (selectedIndex < 0 || optionCount <= 0 || viewportHeight <= 0 || scrollHeight <= viewportHeight) return 0;
  const optionHeight = scrollHeight / optionCount;
  const centered = (selectedIndex + 0.5) * optionHeight - viewportHeight / 2;
  return Math.max(0, Math.min(scrollHeight - viewportHeight, centered));
}

export function chordEditKeyboardAction(
  key: string,
): ChordEditKeyboardAction | null {
  if (key === "Enter") return "commit";
  if (key === "Escape") return "cancel";
  return null;
}

export function chordEditKey(mode: ChordMode, chord: Pick<TimedChord, "startSeconds" | "endSeconds">): string {
  return `${mode}:${chord.startSeconds}:${chord.endSeconds}`;
}

export function applyChordEdits(
  chords: readonly TimedChord[],
  edits: readonly ChordEdit[],
  mode: ChordMode,
): TimedChord[] {
  const labels = new Map(
    edits
      .filter((edit) => edit.mode === mode)
      .map((edit) => [chordEditKey(mode, edit), edit.label]),
  );
  return chords.map((chord) => {
    const label = labels.get(chordEditKey(mode, chord));
    return label === undefined
      ? chord
      : { ...chord, label, sourceLabel: label, edited: true };
  });
}

export function normalizeChordEntry(value: string): string {
  const normalizedSymbols = value.trim().replaceAll("♯", "#").replaceAll("♭", "b");
  const match = /^([a-gA-G])([bB#]?)(.*)$/.exec(normalizedSymbols);
  if (!match) return normalizedSymbols;
  const accidental = match[2] === "B" ? "b" : match[2];
  const suffix = match[3].replace(/\/([a-gA-G])([bB#]?)/g, (_, note: string, bassAccidental: string) => (
    `/${note.toUpperCase()}${bassAccidental === "B" ? "b" : bassAccidental}`
  ));
  return `${match[1].toUpperCase()}${accidental}${suffix}`;
}

export function validateChordEntry(value: string, accidentals: ChordAccidentalMode): string | null {
  const normalized = normalizeChordEntry(value);
  if (!parseChordLabel(normalized)) return null;
  return presentChordLabel(normalized, 0, accidentals);
}

export function chordSuggestions(value: string): string[] {
  const normalized = normalizeChordEntry(value);
  const match = /^([A-G])([#b]?)(.*)$/.exec(normalized);
  if (!match) return [];
  const natural = NATURAL_PITCHES[match[1]];
  if (natural === undefined) return [];
  const offset = match[2] === "#" ? 1 : match[2] === "b" ? -1 : 0;
  const pitch = (natural + offset + 12) % 12;
  const spelling: ChordAccidentalMode = match[2] === "b" ? "flat" : "sharp";
  const query = normalized.toLocaleLowerCase("en");
  return COMMON_CHORD_TEMPLATES
    .map((template) => presentChordLabel(template, pitch, spelling))
    .filter((label, index, labels) => labels.indexOf(label) === index && label.toLocaleLowerCase("en").startsWith(query));
}

export function chordEditOptions(accidentals: ChordAccidentalMode): string[] {
  return Array.from({ length: 12 }, (_, pitch) => (
    COMMON_CHORD_TEMPLATES.map((template) => presentChordLabel(template, pitch, accidentals))
  )).flat().filter((label, index, labels) => labels.indexOf(label) === index);
}

export function updateChordEdits(
  chords: readonly TimedChord[],
  edits: readonly ChordEdit[],
  mode: ChordMode,
  selectedKey: string,
  replacement: string,
  replaceAllSimilar: boolean,
): ChordEdit[] {
  const effective = applyChordEdits(chords, edits, mode);
  const selected = effective.find((chord) => chordEditKey(mode, chord) === selectedKey);
  if (!selected) return [...edits];
  const selectedCanonicalLabel = presentChordLabel(selected.label, 0, "sharp");
  const targetKeys = new Set(
    effective
      .filter((chord) => !replaceAllSimilar || presentChordLabel(chord.label, 0, "sharp") === selectedCanonicalLabel)
      .filter((chord) => replaceAllSimilar || chordEditKey(mode, chord) === selectedKey)
      .map((chord) => chordEditKey(mode, chord)),
  );
  const next = edits.filter((edit) => !targetKeys.has(chordEditKey(edit.mode, edit)));
  for (const chord of chords) {
    if (!targetKeys.has(chordEditKey(mode, chord))
      || presentChordLabel(chord.label, 0, "sharp") === presentChordLabel(replacement, 0, "sharp")) continue;
    next.push({ mode, startSeconds: chord.startSeconds, endSeconds: chord.endSeconds, label: replacement });
  }
  return next.sort((left, right) => left.mode.localeCompare(right.mode) || left.startSeconds - right.startSeconds || left.endSeconds - right.endSeconds);
}
