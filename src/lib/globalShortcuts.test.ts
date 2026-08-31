import assert from "node:assert/strict";
import test from "node:test";

import { isTextEditingTarget, parameterShortcutAction, parameterShortcutForKey, shortcutKeyLabels, shortcutPlatformFor, shouldBlurFocusedSelect, shouldHandleChordNavigationShortcut, shouldHandleGlobalShortcut } from "./globalShortcuts.ts";

function shortcutEvent(overrides: Partial<Parameters<typeof shouldHandleGlobalShortcut>[0]> = {}): Parameters<typeof shouldHandleGlobalShortcut>[0] {
  return {
    altKey: false,
    ctrlKey: false,
    isComposing: false,
    key: "Space",
    metaKey: false,
    shiftKey: false,
    target: new EventTarget(),
    ...overrides,
  };
}

test("global shortcuts never intercept text editing", () => {
  const input = Object.assign(new EventTarget(), { closest: () => ({}) });
  assert.equal(isTextEditingTarget(input), true);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ target: input })), false);
});

test("Option plus an arrow navigates chords outside editing controls", () => {
  assert.equal(shouldHandleChordNavigationShortcut(shortcutEvent({ altKey: true, key: "ArrowLeft" })), true);
  assert.equal(shouldHandleChordNavigationShortcut(shortcutEvent({ altKey: true, key: "ArrowRight" })), true);
  assert.equal(shouldHandleChordNavigationShortcut(shortcutEvent({ altKey: false, key: "ArrowRight" })), false);
  assert.equal(shouldHandleChordNavigationShortcut(shortcutEvent({ altKey: true, key: "ArrowUp" })), false);
  assert.equal(shouldHandleChordNavigationShortcut(shortcutEvent({ altKey: true, key: "ArrowLeft", shiftKey: true })), false);
  const input = Object.assign(new EventTarget(), { closest: () => ({}) });
  assert.equal(shouldHandleChordNavigationShortcut(shortcutEvent({ altKey: true, key: "ArrowLeft", target: input })), false);
});

test("global shortcuts ignore composition and command modifiers", () => {
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent()), true);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ isComposing: true })), false);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ metaKey: true })), false);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ ctrlKey: true })), false);
  assert.equal(shouldHandleGlobalShortcut(shortcutEvent({ altKey: true })), false);
});

test("parameter shortcuts pair T, P, Z, and M with arrows, signs, and reset", () => {
  assert.equal(parameterShortcutForKey("T"), "tempo");
  assert.equal(parameterShortcutForKey("p"), "pitch");
  assert.equal(parameterShortcutForKey("Z"), "zoom");
  assert.equal(parameterShortcutForKey("m"), "metronomeVolume");
  assert.equal(parameterShortcutForKey("x"), null);
  assert.equal(parameterShortcutAction("ArrowUp"), "increment");
  assert.equal(parameterShortcutAction("ArrowRight"), "increment");
  assert.equal(parameterShortcutAction("+"), "increment");
  assert.equal(parameterShortcutAction("ArrowDown"), "decrement");
  assert.equal(parameterShortcutAction("ArrowLeft"), "decrement");
  assert.equal(parameterShortcutAction("-"), "decrement");
  assert.equal(parameterShortcutAction("Delete"), "reset");
  assert.equal(parameterShortcutAction("Backspace"), "reset");
  assert.equal(parameterShortcutAction("Enter"), null);
});

test("shortcut labels follow macOS, Windows, and Linux keyboards", () => {
  assert.equal(shortcutPlatformFor("MacIntel"), "macos");
  assert.equal(shortcutPlatformFor("Win32"), "windows");
  assert.equal(shortcutPlatformFor("Linux x86_64"), "linux");
  assert.deepEqual(shortcutKeyLabels("macos"), {
    alt: "⌥", backspace: "⌫", delete: "Fn ⌫", shift: "⇧", space: "Space",
  });
  assert.deepEqual(shortcutKeyLabels("windows"), {
    alt: "Alt", backspace: "Backspace", delete: "Del", shift: "Shift", space: "Space",
  });
});

test("an outside pointer press releases only a focused select", () => {
  assert.equal(shouldBlurFocusedSelect({ tagName: "SELECT" }, false), true);
  assert.equal(shouldBlurFocusedSelect({ tagName: "SELECT" }, true), false);
  assert.equal(shouldBlurFocusedSelect({ tagName: "BUTTON" }, false), false);
  assert.equal(shouldBlurFocusedSelect(null, false), false);
});
