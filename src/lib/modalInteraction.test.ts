import assert from "node:assert/strict";
import test from "node:test";
import { isPointOutsideModal, shouldDismissModalFromBackdrop } from "./modalInteraction.ts";

test("modal closes only when the pointer starts and ends on its backdrop", () => {
  const bounds = { left: 100, right: 500, top: 100, bottom: 400 };
  const backdrop = Object.assign(new EventTarget(), { getBoundingClientRect: () => bounds });
  const content = new EventTarget();

  assert.equal(isPointOutsideModal({ clientX: 50, clientY: 250 }, backdrop), true);
  assert.equal(isPointOutsideModal({ clientX: 250, clientY: 250 }, backdrop), false);
  assert.equal(shouldDismissModalFromBackdrop({ target: backdrop, clientX: 50, clientY: 250 }, backdrop), true);
  assert.equal(shouldDismissModalFromBackdrop({ target: content, clientX: 50, clientY: 250 }, backdrop), false);
  assert.equal(shouldDismissModalFromBackdrop({ target: backdrop, clientX: 250, clientY: 250 }, backdrop), false);
});
