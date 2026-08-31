const PITCH_NAMES = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"] as const;
const FLAT_PITCH_NAMES = ["C", "Db", "D", "Eb", "E", "F", "Gb", "G", "Ab", "A", "Bb", "B"] as const;

const NATURAL_PITCHES: Readonly<Record<string, number>> = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 };
const SHORTHAND_DEGREES: Readonly<Record<string, readonly string[]>> = {
  "": ["1", "3", "5"], "1": ["1"], "5": ["1", "5"], maj: ["1", "3", "5"], min: ["1", "b3", "5"],
  dim: ["1", "b3", "b5"], aug: ["1", "3", "#5"], sus2: ["1", "2", "5"], sus4: ["1", "4", "5"],
  "6": ["1", "3", "5", "6"], maj6: ["1", "3", "5", "6"], min6: ["1", "b3", "5", "6"],
  "7": ["1", "3", "5", "b7"], maj7: ["1", "3", "5", "7"], min7: ["1", "b3", "5", "b7"],
  "7b5": ["1", "3", "b5", "b7"], "7#5": ["1", "3", "#5", "b7"],
  minmaj7: ["1", "b3", "5", "7"], dim7: ["1", "b3", "b5", "6"], hdim7: ["1", "b3", "b5", "b7"],
  hdim: ["1", "b3", "b5", "b7"], "9": ["1", "3", "5", "b7", "9"], maj9: ["1", "3", "5", "7", "9"],
  min9: ["1", "b3", "5", "b7", "9"], "11": ["1", "3", "5", "b7", "9", "11"],
  maj11: ["1", "3", "5", "7", "9", "11"], min11: ["1", "b3", "5", "b7", "9", "11"],
  "13": ["1", "3", "5", "b7", "9", "11", "13"], maj13: ["1", "3", "5", "7", "9", "11", "13"],
  min13: ["1", "b3", "5", "b7", "9", "11", "13"],
};
const COMPACT_ALIASES: Readonly<Record<string, string>> = {
  m: "min", mmaj7: "minmaj7", m7b5: "hdim7", m6: "min6", m7: "min7", m9: "min9", m11: "min11", m13: "min13",
};

export type ParsedChord = {
  sourceLabel: string;
  root: number;
  bass: number;
  bassExplicit: boolean;
  degrees: string[];
  pitches: number[];
  pitchNames: string[];
};

function pitchIndex(note: string): number | null {
  const match = /^([A-G])([#b]*)$/.exec(note);
  if (!match) return null;
  const natural = NATURAL_PITCHES[match[1]];
  if (natural === undefined) return null;
  const accidentals = [...match[2]].reduce((sum, accidental) => sum + (accidental === "#" ? 1 : -1), 0);
  return (natural + accidentals + 120) % 12;
}

export function degreeSemitones(degree: string): number | null {
  const match = /^([#b]*)(\d+)$/.exec(degree);
  if (!match) return null;
  const number = Number(match[2]);
  if (!Number.isInteger(number) || number < 1 || number > 28) return null;
  const scale = [0, 2, 4, 5, 7, 9, 11];
  const base = scale[(number - 1) % 7] + Math.floor((number - 1) / 7) * 12;
  const accidental = [...match[1]].reduce((sum, value) => sum + (value === "#" ? 1 : -1), 0);
  return base + accidental;
}

function splitBass(suffix: string): { quality: string; bassDegree: string | null } {
  let depth = 0;
  for (let index = suffix.length - 1; index >= 0; index -= 1) {
    const character = suffix[index];
    if (character === ")") depth += 1;
    else if (character === "(") depth -= 1;
    else if (character === "/" && depth === 0) return { quality: suffix.slice(0, index), bassDegree: suffix.slice(index + 1) };
  }
  return { quality: suffix, bassDegree: null };
}

function chordDegrees(qualityExpression: string): string[] | null {
  const match = /^([^()]*)(\([^()]*\))?$/.exec(qualityExpression);
  if (!match) return null;
  let base = match[1];
  if (base in COMPACT_ALIASES) base = COMPACT_ALIASES[base];
  const explicitOnly = base === "" && Boolean(match[2]);
  const shorthand = explicitOnly ? [] : SHORTHAND_DEGREES[base];
  if (!explicitOnly && !shorthand) return null;
  const degrees = [...(shorthand ?? [])];
  const modifiers = match[2] ? match[2].slice(1, -1).split(",").filter(Boolean) : [];
  for (const modifier of modifiers) {
    const omitted = modifier.startsWith("*");
    const degree = omitted ? modifier.slice(1) : modifier;
    const semitones = degreeSemitones(degree);
    if (semitones === null) return null;
    if (omitted) {
      for (let index = degrees.length - 1; index >= 0; index -= 1) {
        if (degreeSemitones(degrees[index]) === semitones) degrees.splice(index, 1);
      }
    } else degrees.push(degree);
  }
  const unique = new Map<number, string>();
  for (const degree of degrees) {
    const semitones = degreeSemitones(degree);
    if (semitones !== null && !unique.has((semitones + 120) % 12)) unique.set((semitones + 120) % 12, degree);
  }
  return [...unique.values()];
}

/** Parse LV-Chordia's JAMS/Harte labels and SonArcan's compact display spelling. */
export function parseChordLabel(label: string): ParsedChord | null {
  if (["N", "X", "-"].includes(label)) return null;
  const match = /^([A-G](?:#|b)*)(?::(.*)|(.*))$/.exec(label);
  if (!match) return null;
  const root = pitchIndex(match[1]);
  if (root === null) return null;
  const canonical = match[2] !== undefined;
  const suffix = canonical ? match[2] : match[3];
  const { quality, bassDegree } = splitBass(suffix);
  const degrees = chordDegrees(quality);
  if (!degrees?.length) return null;
  let bass = root;
  const bassExplicit = Boolean(bassDegree);
  if (bassDegree) {
    if (canonical) {
      const interval = degreeSemitones(bassDegree);
      if (interval === null) return null;
      bass = (root + interval) % 12;
    } else {
      const explicitBass = pitchIndex(bassDegree);
      if (explicitBass === null) return null;
      bass = explicitBass;
    }
  }
  const pitches = degrees
    .map((degree) => degreeSemitones(degree))
    .filter((interval): interval is number => interval !== null)
    .map((interval) => (root + interval) % 12)
    .sort((left, right) => ((left - root + 12) % 12) - ((right - root + 12) % 12));
  if (bassExplicit && !pitches.includes(bass)) pitches.unshift(bass);
  return { sourceLabel: label, root, bass, bassExplicit, degrees, pitches, pitchNames: pitches.map((pitch) => PITCH_NAMES[pitch]) };
}

export function keyboardPosition(pitch: number, root: number): number {
  return pitch < root ? pitch + 12 : pitch;
}

export function chordKeyboardPositions(chord: ParsedChord, inversion = 0, voicing: "close" | "open" = "close"): number[] {
  const intervals = chord.pitches
    .filter((pitch, index, values) => values.indexOf(pitch) === index)
    .map((pitch) => (pitch - chord.root + 12) % 12)
    .sort((left, right) => left - right);
  const bassInterval = (chord.bass - chord.root + 12) % 12;
  const requestedInversion = chord.bassExplicit ? Math.max(0, intervals.indexOf(bassInterval)) : inversion;
  const boundedInversion = intervals.length ? ((requestedInversion % intervals.length) + intervals.length) % intervals.length : 0;
  const rotated = intervals.map((_, index) => {
    const source = intervals[(index + boundedInversion) % intervals.length];
    return source + (index + boundedInversion >= intervals.length ? 12 : 0);
  });
  if (chord.bassExplicit && !intervals.includes(bassInterval)) rotated.unshift(bassInterval - 12);
  if (voicing === "open" && rotated.length >= 3) {
    for (let index = 1; index < rotated.length; index += 2) rotated[index] += 12;
    rotated.sort((left, right) => left - right);
  }
  const absolute = rotated.map((interval) => chord.root + interval);
  const minimum = Math.min(...absolute);
  const shift = minimum < 0 ? 12 : minimum >= 12 ? -12 : 0;
  return absolute.map((position) => position + shift);
}

export const keyboardPitchNames = PITCH_NAMES;
export const keyboardFlatPitchNames = FLAT_PITCH_NAMES;
