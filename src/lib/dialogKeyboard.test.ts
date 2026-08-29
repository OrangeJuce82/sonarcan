import assert from "node:assert/strict";
import test from "node:test";
import { shouldConfirmDialogOnEnter } from "./dialogKeyboard.ts";

function keyboardEvent(overrides: Partial<Parameters<typeof shouldConfirmDialogOnEnter>[0]> = {}): Parameters<typeof shouldConfirmDialogOnEnter>[0] {
  return {
    key: "Enter",
    repeat: false,
    isComposing: false,
    altKey: false,
    ctrlKey: false,
    metaKey: false,
    shiftKey: false,
    target: new EventTarget(),
    ...overrides,
  };
}

test("plain Enter confirms an enabled dialog action", () => {
  assert.equal(shouldConfirmDialogOnEnter(keyboardEvent(), true), true);
  assert.equal(shouldConfirmDialogOnEnter(keyboardEvent(), false), false);
});

test("Enter remains editable inside multiline fields", () => {
  const textarea = Object.assign(new EventTarget(), { closest: () => ({}) });
  assert.equal(shouldConfirmDialogOnEnter(keyboardEvent({ target: textarea }), true), false);
});

test("composition, modifiers, and key repeat never confirm a dialog", () => {
  assert.equal(shouldConfirmDialogOnEnter(keyboardEvent({ isComposing: true }), true), false);
  assert.equal(shouldConfirmDialogOnEnter(keyboardEvent({ shiftKey: true }), true), false);
  assert.equal(shouldConfirmDialogOnEnter(keyboardEvent({ repeat: true }), true), false);
});
