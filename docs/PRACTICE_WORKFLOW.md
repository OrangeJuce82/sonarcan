# Practice workflow reference

This document translates the useful interaction model observed in musician practice players into SonArcan requirements. It describes behavior, not visual imitation.

## Core screen hierarchy

1. A detailed waveform provides precise navigation around the playhead.
2. An overview waveform shows the whole song, loop boundaries, sections, and marks.
3. The practice strip keeps loop, tempo, pitch, and jump controls visible at all times.
4. The transport remains centered and usable without opening another panel.

## First implementation order

### 1. Reliable transport

- play and pause;
- seek by clicking or dragging the timeline;
- configurable backward and forward jumps;
- previous and next track;
- volume;
- restore the last playback position.

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
- retain the setting per track.

### 4. Pitch

- adjust in semitones independently from tempo;
- support fine tuning later;
- display reset and increment/decrement controls;
- use a dedicated DSP implementation rather than pretending that playback-rate changes are pitch shifting.

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

## SonArcan-specific decisions

- The visual language remains SonArcan's dark teal and amber design.
- Controls use text labels and tooltips where an icon would be ambiguous.
- The first webview player validates interaction behavior only.
- The dedicated Rust audio engine is required for sample-accurate looping, production-quality time-stretch, pitch shifting, metronome synchronization, and dropout diagnostics.

