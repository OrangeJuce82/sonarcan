import assert from "node:assert/strict";
import test from "node:test";

import { appendToast, type ToastMessage } from "./toasts.ts";

const toast = (id: number, title = `message ${id}`): ToastMessage => ({
  id,
  level: "info",
  title,
});

test("toast stacks retain only the newest three messages", () => {
  assert.deepEqual(
    appendToast([toast(1), toast(2), toast(3)], toast(4)).map(({ id }) => id),
    [2, 3, 4],
  );
});

test("repeated messages replace their previous occurrence", () => {
  assert.deepEqual(
    appendToast([toast(1, "same"), toast(2)], toast(3, "same")).map(({ id }) => id),
    [2, 3],
  );
});

test("the same title with a different detail remains a distinct toast", () => {
  assert.equal(appendToast(
    [{ ...toast(1, "Import failed"), detail: "First source" }],
    { ...toast(2, "Import failed"), detail: "Second source" },
  ).length, 2);
});
