import { execFile as execFileCallback } from "node:child_process";
import { readFile, writeFile } from "node:fs/promises";
import { resolve } from "node:path";
import { promisify } from "node:util";

const checkout = process.argv[2];
if (!checkout) throw new Error("Usage: node scripts/update-instrument-chord-corpus.mjs /path/to/chords-db");

const sourceRevision = "df06fa7b425cf5fd29485ff6591236b3557e3fac";
const execFile = promisify(execFileCallback);
const { stdout: checkoutRevision } = await execFile("git", ["-C", checkout, "rev-parse", "HEAD"]);
if (checkoutRevision.trim() !== sourceRevision) {
  throw new Error(`Expected chords-db ${sourceRevision}, received ${checkoutRevision.trim()}`);
}
const packageManifest = JSON.parse(await readFile(resolve(checkout, "package.json"), "utf8"));
if (packageManifest.name !== "@tombatossals/chords-db" || packageManifest.license !== "MIT") {
  throw new Error("The selected checkout is not the expected MIT-licensed chords-db source.");
}
const degreeSemitones = (degree) => {
  const match = /^([#b]*)(\d+)$/.exec(degree);
  if (!match) throw new Error(`Invalid degree: ${degree}`);
  const scale = [0, 2, 4, 5, 7, 9, 11];
  const number = Number(match[2]);
  const accidental = [...match[1]].reduce((sum, value) => sum + (value === "#" ? 1 : -1), 0);
  return (scale[(number - 1) % 7] + Math.floor((number - 1) / 7) * 12 + accidental + 120) % 12;
};
const pitchIndex = (note) => {
  const match = /^([A-G])([#b]?)$/.exec(note);
  if (!match) throw new Error(`Invalid pitch: ${note}`);
  const natural = { C: 0, D: 2, E: 4, F: 5, G: 7, A: 9, B: 11 }[match[1]];
  return (natural + (match[2] === "#" ? 1 : match[2] === "b" ? -1 : 0) + 12) % 12;
};

const qualities = {
  major: ["1", "3", "5"], minor: ["1", "b3", "5"], m: ["1", "b3", "5"], dim: ["1", "b3", "b5"], dim7: ["1", "b3", "b5", "6"],
  sus: ["1", "4", "5"], sus2: ["1", "2", "5"], sus4: ["1", "4", "5"], sus2sus4: ["1", "2", "4", "5"], "7sus4": ["1", "4", "5", "b7"],
  aug: ["1", "3", "#5"], "5": ["1", "5"], "6": ["1", "3", "5", "6"], "69": ["1", "3", "5", "6", "9"],
  "7": ["1", "3", "5", "b7"], "7b5": ["1", "3", "b5", "b7"], aug7: ["1", "3", "#5", "b7"],
  "9": ["1", "3", "5", "b7", "9"], "9b5": ["1", "3", "b5", "b7", "9"], aug9: ["1", "3", "#5", "b7", "9"],
  "7b9": ["1", "3", "5", "b7", "b9"], "7#9": ["1", "3", "5", "b7", "#9"], "7sharp9": ["1", "3", "5", "b7", "#9"],
  "7b5b9": ["1", "3", "b5", "b7", "b9"], "7sharp5b9": ["1", "3", "#5", "b7", "b9"],
  "7b5sharp9": ["1", "3", "b5", "b7", "#9"], "7sharp5sharp9": ["1", "3", "#5", "b7", "#9"],
  "11": ["1", "3", "5", "b7", "9", "11"], "9#11": ["1", "3", "5", "b7", "9", "#11"], "9sharp11": ["1", "3", "5", "b7", "9", "#11"],
  "13": ["1", "3", "5", "b7", "9", "11", "13"],
  maj7: ["1", "3", "5", "7"], maj7b5: ["1", "3", "b5", "7"], "maj7#5": ["1", "3", "#5", "7"], maj7sharp5: ["1", "3", "#5", "7"], maj7sus2: ["1", "2", "5", "7"],
  maj9: ["1", "3", "5", "7", "9"], maj11: ["1", "3", "5", "7", "9", "11"], maj13: ["1", "3", "5", "7", "9", "11", "13"],
  m6: ["1", "b3", "5", "6"], m69: ["1", "b3", "5", "6", "9"], m7: ["1", "b3", "5", "b7"], m7b5: ["1", "b3", "b5", "b7"],
  m9: ["1", "b3", "5", "b7", "9"], m11: ["1", "b3", "5", "b7", "9", "11"], mmaj7: ["1", "b3", "5", "7"],
  mmaj7b5: ["1", "b3", "b5", "7"], mmaj9: ["1", "b3", "5", "7", "9"], mmaj11: ["1", "b3", "5", "7", "9", "11"],
  add9: ["1", "3", "5", "9"], madd9: ["1", "b3", "5", "9"], add11: ["1", "3", "5", "11"],
};

function semantics(rootName, suffix) {
  const slash = /^(major|minor|m9|m|7)?\/([A-G](?:#|b)?)$/.exec(suffix);
  const quality = slash?.[1] ?? suffix;
  const degrees = qualities[quality || "major"];
  if (!degrees) return null;
  const root = pitchIndex(rootName);
  const pitches = [...new Set(degrees.map((degree) => (root + degreeSemitones(degree)) % 12))].sort((left, right) => left - right);
  const bass = slash ? pitchIndex(slash[2]) : null;
  return `${root}|${pitches.join(",")}|${bass ?? "-"}`;
}

function absolutePosition(position) {
  const baseFret = Number(position.baseFret ?? 1);
  const frets = position.frets.map((fret) => fret > 0 && baseFret > 1 ? fret + baseFret - 1 : fret);
  const fingers = position.fingers ?? frets.map((fret) => fret > 0 ? 1 : 0);
  return { frets, fingers };
}

async function instrumentCorpus(instrument) {
  const input = JSON.parse(await readFile(resolve(checkout, "lib", `${instrument}.json`), "utf8"));
  const output = {};
  for (const entries of Object.values(input.chords)) {
    for (const entry of entries) {
      const key = semantics(entry.key, entry.suffix);
      if (!key) continue;
      const positions = instrument === "piano"
        ? entry.positions.map((position) => ({ pitches: position.frets.map(pitchIndex) }))
        : entry.positions.map(absolutePosition);
      output[key] = [...(output[key] ?? []), ...positions];
    }
  }
  return output;
}

const corpus = {
  source: { name: "@tombatossals/chords-db", revision: sourceRevision, license: "MIT" },
  instruments: {
    guitar: await instrumentCorpus("guitar"),
    ukulele: await instrumentCorpus("ukulele"),
    piano: await instrumentCorpus("piano"),
  },
};
await writeFile(resolve("src/lib/instrumentChordCorpus.json"), `${JSON.stringify(corpus)}\n`);
