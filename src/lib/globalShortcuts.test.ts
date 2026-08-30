import assert from "node:assert/strict";
import test from "node:test";

import { isTextEditingTarget, shouldHandleGlobalShortcut } from "./globalShortcuts.ts";

function shortcutEvent(overrides: Partial<Parameters<typeof shouldHandleGlobalShortcut>[0]> = {}): Parameters<typeof shouldHandleGlobalShortcut>[0] {
  return {
    altKey: false,
    ctrlKey: false,
    isComposing: false,
    metaKey: false,
    target: new EventTarget(),
    ...overrides,
  };
}

test("global shortcuts never intercept text editing", () => {
  const input = Object.assign(new EventTarget(), { closest: () => ({}) });
  assert.equal(isTextEditingTarget(input), true);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ target: input })), false);
});

test("global shortcuts ignore composition and command modifiers", () => {
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent()), true);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ isComposing: true })), false);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ metaKey: true })), false);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ ctrlKey: true })), false);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ altKey: true })), false);
});
