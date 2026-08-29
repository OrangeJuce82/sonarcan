const droppedAudioExtensions = new Set([
  "aac", "aif", "aiff", "alac", "flac", "m4a", "mp3", "ogg", "opus", "wav", "wma",
]);

export function droppedAudioPaths(paths: string[]): string[] {
  return paths.filter((path) => {
    const clean = path.split(/[?#]/, 1)[0];
    const extension = clean.includes(".") ? clean.slice(clean.lastIndexOf(".") + 1).toLowerCase() : "";
    return droppedAudioExtensions.has(extension);
  });
}
