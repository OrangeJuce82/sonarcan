import assert from "node:assert/strict";
import test from "node:test";

import { isTextEditingTarget, isTextEntryTarget, metronomeShortcutAction, parameterShortcutAction, parameterShortcutForKey, shortcutKeyLabels, shortcutPlatformFor, shouldBlurFocusedSelect, shouldHandleGlobalShortcut, shouldHandleParameterShortcut, shouldHandlePlayPauseShortcut, shouldToggleBeatThisDbnShortcut, shouldToggleChordEditModeShortcut, shouldToggleMetronomeOnRelease } from "./globalShortcuts.ts";

function shortcutEvent(overrides: Partial<Parameters<typeof shouldHandleGlobalShortcut>[0]> = {}): Parameters<typeof shouldHandleGlobalShortcut>[0] {
  return {
    altKey: false,
    code: "Space",
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

test("Space toggles playback over controls but not while entering text", () => {
  const button = Object.assign(new EventTarget(), { closest: () => null });
  const select = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("select") ? ({}) : null });
  const input = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("input") ? ({}) : null });
  assert.equal(isTextEntryTarget(input), true);
  assert.equal(isTextEntryTarget(select), false);
  assert.equal(shouldHandlePlayPauseShortcut(shortcutEvent({ key: " ", target: button })), true);
  assert.equal(shouldHandlePlayPauseShortcut(shortcutEvent({ key: " ", target: select })), true);
  assert.equal(shouldHandlePlayPauseShortcut(shortcutEvent({ key: " ", target: input })), false);
  assert.equal(shouldHandlePlayPauseShortcut(shortcutEvent({ key: "Enter", target: button })), false);
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

test("M maps vertical arrows to sound and horizontal arrows to volume", () => {
  assert.equal(metronomeShortcutAction("ArrowUp"), "nextSound");
  assert.equal(metronomeShortcutAction("ArrowDown"), "previousSound");
  assert.equal(metronomeShortcutAction("ArrowRight"), "incrementVolume");
  assert.equal(metronomeShortcutAction("ArrowLeft"), "decrementVolume");
  assert.equal(metronomeShortcutAction("+"), "incrementVolume");
  assert.equal(metronomeShortcutAction("-"), "decrementVolume");
  assert.equal(metronomeShortcutAction("Backspace"), "resetVolume");
  assert.equal(metronomeShortcutAction("Enter"), null);
});

test("parameter shortcuts are global over controls but preserve text entry", () => {
  const button = Object.assign(new EventTarget(), { closest: () => null });
  const select = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("select") ? ({}) : null });
  const range = Object.assign(new EventTarget(), { closest: () => null });
  const textInput = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("input:not") ? ({}) : null });
  assert.equal(shouldHandleParameterShortcut(shortcutEvent({ key: "m", target: button })), true);
  assert.equal(shouldHandleParameterShortcut(shortcutEvent({ key: "t", target: select })), true);
  assert.equal(shouldHandleParameterShortcut(shortcutEvent({ key: "p", target: range })), true);
  assert.equal(shouldHandleParameterShortcut(shortcutEvent({ key: "z", target: textInput })), false);
  assert.equal(shouldHandleParameterShortcut(shortcutEvent({ key: "+", shiftKey: true })), true);
});

test("E toggles chord editing globally except during text entry", () => {
  const button = Object.assign(new EventTarget(), { closest: () => null });
  const select = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("select") ? ({}) : null });
  const textInput = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("input:not") ? ({}) : null });
  assert.equal(shouldToggleChordEditModeShortcut(shortcutEvent({ key: "e", target: button })), true);
  assert.equal(shouldToggleChordEditModeShortcut(shortcutEvent({ key: "E", target: select })), true);
  assert.equal(shouldToggleChordEditModeShortcut(shortcutEvent({ key: "e", target: textInput })), false);
  assert.equal(shouldToggleChordEditModeShortcut(shortcutEvent({ key: "e", ctrlKey: true })), false);
});

test("M toggles the metronome on release only when no volume action was used", () => {
  const releaseM = shortcutEvent({ key: "m" });
  assert.equal(shouldToggleMetronomeOnRelease(releaseM, "metronomeVolume", false), true);
  assert.equal(shouldToggleMetronomeOnRelease(releaseM, "metronomeVolume", true), false);
  assert.equal(shouldToggleMetronomeOnRelease(releaseM, "tempo", false), false);
  assert.equal(shouldToggleMetronomeOnRelease(shortcutEvent({ key: "t" }), "metronomeVolume", false), false);
  const input = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("input:not") ? ({}) : null });
  assert.equal(shouldToggleMetronomeOnRelease(shortcutEvent({ key: "m", target: input }), "metronomeVolume", false), false);
  const select = Object.assign(new EventTarget(), { closest: (selector: string) => selector.includes("select") ? ({}) : null });
  assert.equal(shouldToggleMetronomeOnRelease(shortcutEvent({ key: "m", target: select }), "metronomeVolume", false), true);
});

test("Alt+M toggles Beat This! DBN", () => {
  assert.equal(shouldToggleBeatThisDbnShortcut(shortcutEvent({ code: "Semicolon", key: "µ", altKey: true })), true);
  assert.equal(shouldToggleBeatThisDbnShortcut(shortcutEvent({ code: "KeyM", key: "M", altKey: true })), true);
  assert.equal(shouldToggleBeatThisDbnShortcut(shortcutEvent({ key: "m", shiftKey: true, altKey: true })), false);
  const input = Object.assign(new EventTarget(), { closest: () => ({}) });
  assert.equal(shouldToggleBeatThisDbnShortcut(shortcutEvent({ key: "m", altKey: true, target: input })), false);
});

test("shortcut labels follow macOS, Windows, and Linux keyboards", () => {
  assert.equal(shortcutPlatformFor("MacIntel"), "macos");
  assert.equal(shortcutPlatformFor("Win32"), "windows");
  assert.equal(shortcutPlatformFor("Linux x86_64"), "linux");
  assert.deepEqual(shortcutKeyLabels("macos"), {
    backspace: "⌫", delete: "Fn ⌫", space: "Space",
  });
  assert.deepEqual(shortcutKeyLabels("windows"), {
    backspace: "Backspace", delete: "Del", space: "Space",
  });
});

test("an outside pointer press releases only a focused select", () => {
  assert.equal(shouldBlurFocusedSelect({ tagName: "SELECT" }, false), true);
  assert.equal(shouldBlurFocusedSelect({ tagName: "SELECT" }, true), false);
  assert.equal(shouldBlurFocusedSelect({ tagName: "BUTTON" }, false), false);
  assert.equal(shouldBlurFocusedSelect(null, false), false);
});
