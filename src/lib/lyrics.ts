import type { LyricsDocument, LyricsLine, LyricsSyncLevel, LyricsWord, RemoteLyricsRecord } from "./types";

const MAX_LINES = 10_000;
const MAX_INPUT_LENGTH = 2 * 1024 * 1024;

export class LyricsParseError extends Error {
  readonly code: "invalidTimestamp" | "timestampOutOfRange";

  constructor(code: "invalidTimestamp" | "timestampOutOfRange") {
    super(code);
    this.name = "LyricsParseError";
    this.code = code;
  }
}

export function parseLyrics(input: string, language = "und", durationMs?: number): LyricsDocument {
  if (input.length > MAX_INPUT_LENGTH) throw new Error("Lyrics exceed the 2 MiB safety limit.");
  const trimmed = input.replace(/^\uFEFF/, "").trim();
  if (!trimmed) throw new Error("Lyrics are empty.");
  validateSyncTimestamps(trimmed);
  const lines = /<tt[\s>]/i.test(trimmed) ? parseTtml(trimmed) : parseLrcOrPlain(trimmed);
  if (!lines.length) throw new Error("No lyric line was found.");
  if (lines.length > MAX_LINES) throw new Error("Lyrics contain too many lines.");
  validateTimestampRange(lines, durationMs);
  completeLineEnds(lines, durationMs);
  const syncLevel: LyricsSyncLevel = lines.some((line) => line.words.length) ? "word"
    : lines.some((line) => line.startMs !== null) ? "line" : "none";
  return {
    version: 1,
    provider: "local",
    providerTrackId: null,
    language: language || "und",
    syncLevel,
    attribution: null,
    copyright: null,
    offsetMs: 0,
    lines,
  };
}

export function lyricsEditorContent(document: LyricsDocument, selectedLine = -1): {
  text: string;
  selectionStart: number;
  selectionEnd: number;
} {
  const serialized = document.lines.map((line) => {
    if (line.startMs === null) return line.text;
    const body = line.words.length
      ? line.words.map((word) => `<${formatLrcTime(word.startMs)}>${word.text}`).join("")
      : line.text;
    return `[${formatLrcTime(line.startMs)}]${body}`;
  });
  const text = serialized.join("\n");
  if (selectedLine < 0 || selectedLine >= serialized.length) {
    return { text, selectionStart: 0, selectionEnd: 0 };
  }
  const selectionStart = serialized.slice(0, selectedLine).reduce((length, line) => length + line.length + 1, 0);
  return { text, selectionStart, selectionEnd: selectionStart + (serialized[selectedLine]?.length ?? 0) };
}

export function formatLrcTime(milliseconds: number): string {
  const total = Math.max(0, milliseconds) / 1_000;
  return `${String(Math.floor(total / 60)).padStart(2, "0")}:${(total % 60).toFixed(2).padStart(5, "0")}`;
}

export function lrclibDocument(record: RemoteLyricsRecord, language = "und", durationMs = record.durationSeconds * 1_000): LyricsDocument {
  const content = record.syncedLyrics || record.plainLyrics;
  if (!content) throw new Error("The selected LRCLIB record does not contain lyrics.");
  return {
    ...parseLyrics(content, language, durationMs),
    provider: "lrclib",
    providerTrackId: String(record.id),
    attribution: "LRCLIB",
  };
}

export function activeLyricsLineIndex(document: LyricsDocument | null, currentMs: number): number {
  if (!document || document.syncLevel === "none") return -1;
  const adjusted = currentMs - document.offsetMs;
  let low = 0;
  let high = document.lines.length - 1;
  let result = -1;
  while (low <= high) {
    const middle = (low + high) >> 1;
    const start = document.lines[middle]?.startMs;
    if (start !== null && start !== undefined && start <= adjusted) {
      result = middle;
      low = middle + 1;
    } else high = middle - 1;
  }
  if (result < 0) return -1;
  const end = document.lines[result]?.endMs;
  return end !== null && end !== undefined && adjusted >= end ? -1 : result;
}

export function estimatedLyricsLineIndex(document: LyricsDocument | null, currentMs: number, durationMs: number): number {
  if (!document || document.syncLevel !== "none" || !document.lines.length || !Number.isFinite(durationMs) || durationMs <= 0) return -1;
  const progress = Math.max(0, Math.min(1, currentMs / durationMs));
  return Math.min(document.lines.length - 1, Math.floor(progress * document.lines.length));
}

export function activeLyricsWordIndex(line: LyricsLine | undefined, currentMs: number, offsetMs: number): number {
  if (!line?.words.length) return -1;
  const adjusted = currentMs - offsetMs;
  let result = -1;
  for (let index = 0; index < line.words.length; index += 1) {
    const word = line.words[index];
    if (word.startMs > adjusted) break;
    result = index;
  }
  return result;
}

export function lyricsNavigationPositions(document: LyricsDocument | null, durationSeconds: number): number[] {
  if (!document) return [];
  return document.lines.flatMap((line) => {
    if (line.startMs === null) return [];
    const seconds = (line.startMs + document.offsetMs) / 1_000;
    return Number.isFinite(seconds) && seconds >= 0 && seconds <= durationSeconds ? [seconds] : [];
  });
}

function parseLrcOrPlain(input: string): LyricsLine[] {
  const result: LyricsLine[] = [];
  for (const rawLine of input.split(/\r?\n/)) {
    const timestamps = [...rawLine.matchAll(/\[(\d{1,3}):(\d{2}(?:\.\d{1,3})?)\]/g)];
    const body = rawLine.replace(/\[(?:\d{1,3}):(?:\d{2}(?:\.\d{1,3})?)\]/g, "").trim();
    if (!timestamps.length) {
      if (body && !/^\[[a-z]+:/i.test(body)) result.push({ text: body, startMs: null, endMs: null, words: [] });
      continue;
    }
    const words = parseEnhancedLrcWords(body);
    const text = words.length ? words.map((word) => word.text).join("").trim() : body;
    if (!text) continue;
    for (const timestamp of timestamps) {
      result.push({
        text,
        startMs: minuteTimestamp(timestamp[1], timestamp[2]),
        endMs: null,
        words: words.map((word) => ({ ...word })),
      });
    }
  }
  return result.sort((left, right) => (left.startMs ?? Number.MAX_SAFE_INTEGER) - (right.startMs ?? Number.MAX_SAFE_INTEGER));
}

function validateSyncTimestamps(input: string): void {
  for (const line of input.split(/\r?\n/)) {
    validateTimestampDelimiters(line, "[", "]");
    validateTimestampDelimiters(line, "<", ">");
  }
}

function validateTimestampDelimiters(line: string, opening: "[" | "<", closing: "]" | ">"): void {
  let cursor = 0;
  while (cursor < line.length) {
    const start = line.indexOf(opening, cursor);
    if (start < 0) return;
    const candidateStart = start + 1;
    if (!/^\d{1,3}:/.test(line.slice(candidateStart))) {
      cursor = candidateStart;
      continue;
    }
    const end = line.indexOf(closing, candidateStart);
    if (end < 0 || !validLrcTimestamp(line.slice(candidateStart, end))) {
      throw new LyricsParseError("invalidTimestamp");
    }
    cursor = end + 1;
  }
}

function validLrcTimestamp(value: string): boolean {
  const match = value.match(/^\d{1,3}:(\d{2})(?:\.\d{1,3})?$/);
  return Boolean(match && Number(match[1]) < 60);
}

function validateTimestampRange(lines: LyricsLine[], durationMs?: number): void {
  if (durationMs === undefined) return;
  if (!Number.isFinite(durationMs) || durationMs < 0) throw new LyricsParseError("timestampOutOfRange");
  const outside = (value: number | null): boolean => value !== null && (value < 0 || value > durationMs);
  if (lines.some((line) => outside(line.startMs) || outside(line.endMs)
    || line.words.some((word) => outside(word.startMs) || outside(word.endMs)))) {
    throw new LyricsParseError("timestampOutOfRange");
  }
}

function parseEnhancedLrcWords(body: string): LyricsWord[] {
  const matches = [...body.matchAll(/<(\d{1,3}):(\d{2}(?:\.\d{1,3})?)>([^<]*)/g)];
  return matches.map((match, index) => ({
    text: match[3] ?? "",
    startMs: minuteTimestamp(match[1], match[2]),
    endMs: index + 1 < matches.length ? minuteTimestamp(matches[index + 1][1], matches[index + 1][2]) : null,
  })).filter((word) => word.text.length > 0);
}

function parseTtml(input: string): LyricsLine[] {
  const result: LyricsLine[] = [];
  const paragraphs = input.matchAll(/<p\b([^>]*)>([\s\S]*?)<\/p>/gi);
  for (const paragraph of paragraphs) {
    const attributes = paragraph[1] ?? "";
    const content = paragraph[2] ?? "";
    const startMs = parseTtmlTime(attribute(attributes, "begin"));
    const endMs = parseTtmlTime(attribute(attributes, "end"));
    const words: LyricsWord[] = [];
    for (const span of content.matchAll(/<span\b([^>]*)>([\s\S]*?)<\/span>/gi)) {
      const wordStart = parseTtmlTime(attribute(span[1] ?? "", "begin"));
      const wordEnd = parseTtmlTime(attribute(span[1] ?? "", "end"));
      const text = decodeXml(stripTags(span[2] ?? ""));
      if (text && wordStart !== null) words.push({ text, startMs: wordStart, endMs: wordEnd });
    }
    const text = decodeXml(stripTags(content.replace(/<br\s*\/?>/gi, "\n"))).trim();
    if (text) result.push({ text, startMs, endMs, words });
  }
  return result;
}

function attribute(attributes: string, name: string): string | null {
  const match = attributes.match(new RegExp(`(?:^|\\s)${name}\\s*=\\s*["']([^"']+)["']`, "i"));
  return match?.[1] ?? null;
}

function parseTtmlTime(value: string | null): number | null {
  if (!value) return null;
  if (/^\d+(?:\.\d+)?s$/.test(value)) return Math.round(Number.parseFloat(value) * 1_000);
  const match = value.match(/^(?:(\d+):)?(\d{1,2}):(\d{2}(?:\.\d+)?)$/);
  if (!match) return null;
  return Math.round(((Number(match[1] ?? 0) * 60 + Number(match[2])) * 60 + Number(match[3])) * 1_000);
}

function minuteTimestamp(minutes: string, seconds: string): number {
  return Math.round((Number(minutes) * 60 + Number(seconds)) * 1_000);
}

function stripTags(value: string): string {
  return value.replace(/<[^>]*>/g, "");
}

function decodeXml(value: string): string {
  return value
    .replace(/&lt;/g, "<").replace(/&gt;/g, ">").replace(/&quot;/g, "\"")
    .replace(/&apos;/g, "'").replace(/&amp;/g, "&");
}

function completeLineEnds(lines: LyricsLine[], durationMs?: number): void {
  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];
    if (line.startMs === null || line.endMs !== null) continue;
    const nextStart = lines.slice(index + 1).find((candidate) => candidate.startMs !== null)?.startMs;
    line.endMs = nextStart ?? (durationMs && durationMs > line.startMs ? Math.round(durationMs) : null);
  }
}
