import assert from "node:assert/strict";
import test from "node:test";

import { audioToolsRelease } from "../../scripts/audio-tools-release.mjs";

test("portable FFmpeg archives belong to the pinned immutable autobuild", () => {
  assert.equal(audioToolsRelease.tag, "autobuild-2026-08-29-13-12");
  assert.equal(
    audioToolsRelease.checksumsSha256,
    "3d9d4aaf0d4b1a9cb28f36847680e5c585a71fe29f40e5f948d211e8735be056",
  );
  assert.deepEqual(audioToolsRelease.assets, {
    "linux-x64": "ffmpeg-N-126313-g1ae4048218-linux64-lgpl.tar.xz",
    "win32-x64": "ffmpeg-N-126313-g1ae4048218-win64-lgpl.zip",
  });
  assert.equal(Object.values(audioToolsRelease.assets).some((asset) => asset.includes("latest")), false);
});
