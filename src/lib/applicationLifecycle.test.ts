import assert from "node:assert/strict";
import test from "node:test";
import { handleWindowCloseRequest, projectOpenDialogOptions } from "./applicationLifecycle.ts";

test("a native window close always becomes an application exit request", () => {
  let prevented = false;
  let exitRequested = false;

  handleWindowCloseRequest(
    { preventDefault: () => { prevented = true; } },
    () => { exitRequested = true; },
  );

  assert.equal(prevented, true);
  assert.equal(exitRequested, true);
});

test("the project picker selects packaged sac documents instead of directories", () => {
  assert.deepEqual(projectOpenDialogOptions("Open a SonArcan project"), {
    directory: false,
    multiple: false,
    title: "Open a SonArcan project",
    filters: [{ name: "Open a SonArcan project", extensions: ["sac"] }],
  });
});
