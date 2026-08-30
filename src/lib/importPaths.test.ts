import assert from "node:assert/strict";
import test from "node:test";

import { droppedAudioPaths } from "./importPaths.ts";

test("playlist drops keep supported and convertible audio paths only", () => {
  assert.deepEqual(droppedAudioPaths([
    "/Music/song.MP3",
    "/Music/session.m4a",
    "/Music/notes.txt",
    "/Music/folder",
  ]), ["/Music/song.MP3", "/Music/session.m4a"]);
});

test("playlist drops ignore URL suffixes when checking audio extensions", () => {
  assert.deepEqual(droppedAudioPaths(["/Music/song.flac#copy"]), ["/Music/song.flac#copy"]);
});
