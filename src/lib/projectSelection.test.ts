import assert from "node:assert/strict";
import test from "node:test";

import { forgetTrackSelection, preferredTrack, rememberedTrackId, rememberTrackSelection } from "./projectSelection.ts";

class MemoryStorage {
  readonly values = new Map<string, string>();
  getItem(key: string): string | null { return this.values.get(key) ?? null; }
  setItem(key: string, value: string): void { this.values.set(key, value); }
}

test("project selection remembers one track per project and can forget it", () => {
  const storage = new MemoryStorage();
  rememberTrackSelection(storage, "/Music/First.sac", "track-b");
  rememberTrackSelection(storage, "/Music/Second.sac", "track-c");
  assert.equal(rememberedTrackId(storage, "/Music/First.sac"), "track-b");
  assert.equal(rememberedTrackId(storage, "/Music/Second.sac"), "track-c");
  forgetTrackSelection(storage, "/Music/First.sac");
  assert.equal(rememberedTrackId(storage, "/Music/First.sac"), null);
});

test("preferredTrack restores a valid selection and otherwise uses the first track", () => {
  const tracks = [{ id: "first" }, { id: "remembered" }];
  assert.equal(preferredTrack(tracks, "remembered")?.id, "remembered");
  assert.equal(preferredTrack(tracks, "removed")?.id, "first");
  assert.equal(preferredTrack([], "remembered"), null);
});

test("invalid stored selection data is ignored", () => {
  const storage = new MemoryStorage();
  storage.values.set("sonarcan.project-track-selection", "not-json");
  assert.equal(rememberedTrackId(storage, "/Music/First.sac"), null);
});
