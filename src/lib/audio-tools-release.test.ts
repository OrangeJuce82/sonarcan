import assert from "node:assert/strict";
import test from "node:test";

import { audioToolsRelease } from "../../scripts/audio-tools-release.mjs";

test("portable FFmpeg archives belong to the pinned dated autobuild", () => {
  assert.equal(audioToolsRelease.tag, "autobuild-2026-09-10-15-31");
  assert.equal(
    audioToolsRelease.checksumsSha256,
    "005ee3313312031bf9b178eb1e2625f6401fb8ff924b42626fe3d2a23e590382",
  );
  assert.equal(audioToolsRelease.checksumsAssetId, 555281436);
  assert.deepEqual(audioToolsRelease.assets, {
    "linux-x64": "ffmpeg-N-126492-gefb0a7e5e7-linux64-lgpl.tar.xz",
  });
  assert.deepEqual(audioToolsRelease.assetIds, {
    "linux-x64": 555280642,
  });
  assert.equal(Object.values(audioToolsRelease.assets).some((asset) => asset.includes("latest")), false);
});
