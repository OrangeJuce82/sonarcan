# Development roadmap

## Phase 0 — technical validation

- [x] Tauri 2 + Svelte + TypeScript shell
- [x] typed command boundary
- [x] structured logging bootstrap
- [x] versioned `.sac` package creation and opening
- [x] WAV, MP3, and FLAC import validation
- [x] audio container probing and metadata extraction
- [x] project and playlist-track renaming
- [x] Save As package duplication
- [x] local Open Recent menu
- [x] initial desktop audio playback through the Tauri webview
- [ ] dedicated Rust real-time audio engine
- [ ] multiresolution waveform extraction
- [ ] spectrum/FFT worker
- [ ] sample-accurate A/B loop
- [ ] time-stretch benchmark and implementation
- [ ] pitch-shift benchmark and implementation
- [ ] BPM and metronome prototype
- [ ] background job queue with cancellation
- [ ] first stem-separation model on Apple Silicon
- [ ] CPU/memory/audio profiling report

## Phase 1 — durable core

- portable and referenced media modes;
- project relinking and migrations;
- playlist editing and persistence;
- crash-safe recovery and backups;
- audio-device diagnostics;
- CI builds on macOS, Linux, and Windows.

## Phase 2 — learning workflow

- detailed and overview waveform navigation;
- reliable transport, seek, volume, and configurable jumps;
- A/B loop with visible boundaries and optional restart delay;
- independent tempo and pitch controls;
- loop trainer with progressive tempo increments;
- keyboard shortcuts for all practice actions;
- BPM, metronome, markers, and sections;
- synchronized playback state.

## Phase 3 — analysis and models

- model manager and compatibility validation;
- stem cache and stem mixer;
- chord analysis and editable chord grid;
- PDF export.

## Phase 4 — import and distribution

- safe ZIP import;
- isolated `yt-dlp` jobs with high-quality MP3 defaults;
- macOS Apple Silicon packaging first;
- Linux packaging second;
- Windows packaging third.
