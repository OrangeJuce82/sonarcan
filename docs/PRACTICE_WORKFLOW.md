# Practice workflow reference

This document translates the useful interaction model observed in musician practice players into SonArcan requirements. It describes behavior, not visual imitation.

## Core screen hierarchy

1. A detailed waveform provides precise navigation around the playhead.
2. An overview waveform shows the whole song, loop boundaries, sections, and marks.
3. An editable beat grid is shown at both waveform levels and drives the Rust metronome.
4. The practice strip keeps loop, tempo, pitch, and jump controls visible at all times.
5. The transport remains centered and usable without opening another panel.

## First implementation order

### 1. Reliable transport

- play and pause;
- seek by clicking or dragging the timeline;
- configurable backward and forward jumps;
- previous and next track;
- volume;
- apply the configured start rule consistently whenever a track loads.

The current implementation persists position, tempo, volume, and A/B boundaries per track in `project.json`. Saves are debounced during playback and flushed immediately when playback pauses or the active track changes. The stored position remains project state but does not determine where a newly loaded track starts.

Every track load, including project restoration and playlist switches, uses the
same start rule. A track with Loop off starts at the beginning. A track with
Loop on starts at the beginning by default, while the user preference can make
it start at A instead.

The last selected track is remembered per project as local user-interface state.
Reopening a known project restores that track; a missing remembered track falls
back to the first playlist entry. Opening an empty project clears every track
control and shows a dedicated import action instead of stale practice panels.

### 2. A/B practice loop

- set A at the current position;
- set B at the current position;
- show both boundaries on the overview;
- repeat `[A, B)` without user intervention;
- clear either boundary or the complete loop;
- preserve the loop while changing tempo;
- add an optional restart delay later.

### 3. Tempo

- adjust playback speed independently from pitch;
- display both percentage and effective BPM when BPM is known;
- provide reset and small increment/decrement controls;
- provide button and `T` keyboard tap tempo using a robust rolling median;
- retain the setting per track.

### 4. Pitch

- adjust in semitones independently from tempo;
- support fine tuning later;
- display reset and increment/decrement controls;
- use a dedicated DSP implementation rather than pretending that playback-rate changes are pitch shifting.
- provide 1-cent fine tuning for historical or otherwise slightly detuned recordings.

## Numeric control interaction

Every adjustable integer or floating-point parameter uses the same interaction model: `+` and `−` apply one configured step, vertical dragging changes the value continuously by steps, the mouse wheel changes a focused value, and double-click restores its default. BPM additionally interprets repeated stationary clicks as tap tempo. The pitch control uses 1-cent steps; speed, BPM, volume, metronome volume, and Loop Trainer settings use domain-appropriate steps.

### 5. Marks and navigation

- create a named mark at the current position;
- jump to previous or next mark;
- distinguish simple marks from saved loop marks;
- show marks on both waveform levels;
- persist marks in the `.sac` manifest.

### 6. Practice automation

- repeat a loop a configurable number of times;
- increase tempo after a successful repetition group;
- stop at a configured target tempo;
- optionally add a delay before restarting the loop.

The implemented Loop Trainer performs the first three operations directly in the Rust renderer. It counts complete A/B repetitions when Loop is active and complete-track repetitions in normal mode. A lead-in before A is available for context but is not counted; the first counted cycle always reaches B after entering the loop. Repetition count, percentage increment, target speed, progress, and enabled state are visible in the practice workspace and persisted per track.

## SonArcan-specific decisions

- The visual language remains SonArcan's dark teal and amber design.
- Controls use text labels and tooltips where an icon would be ambiguous.
- The first webview player validates interaction behavior only.
- The dedicated Rust audio engine is required for sample-accurate looping, production-quality time-stretch, pitch shifting, metronome synchronization, and dropout diagnostics.

## Current keyboard shortcuts

| Action | Shortcut |
|---|---|
| Play or pause | Space |
| Jump backward/forward five seconds | Left/Right Arrow |
| Set loop A/B | A / B |
| Clear loop | Escape |
| Toggle metronome | M |
| Tap tempo | T |
