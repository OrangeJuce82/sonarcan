# Development roadmap

**Current focus:** complete the Phase 0 audio core, with spectrum analysis and beat-grid refinement next.
**Last updated:** 2026-08-27

**Architecture invariant:** playback and every DSP operation run in the Rust real-time engine. TypeScript/Svelte is only a control surface and must never process audio.

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
- [x] automatic restoration of the most recent project at startup
- [x] native OS application menus and standard accelerators
- [x] initial desktop audio playback through the Tauri webview
- [x] dedicated Rust/CPAL real-time playback engine
- [x] cached waveform extraction
- [x] detailed/overview waveform zoom and navigation
- [x] Rust FFT worker with a synchronized 64-band logarithmic spectrum view
- [x] sample-level A/B loop with boundary crossfade
- [x] draggable A/B edges and movable highlighted loop region
- [x] A/B range mirrored in detailed and overview waveforms
- [x] loop enable/disable without discarding its range
- [x] Rust-backed volume slider and mute button
- [x] bounded decoded-audio LRU cache and adjacent-track preloading
- [x] persistent fingerprinted PCM cache and sequential full-playlist warming
- [x] cancellable asynchronous track selection
- [x] in-session waveform display cache
- [x] direct, mode-free A / whole-loop / B mouse adjustment
- [x] focus-safe A, B, L, and Escape loop shortcuts
- [x] parallel audio/waveform loading with modern independent loading states
- [x] Play starts at A whenever looping is enabled
- [x] contextual hover help for practice and transport controls
- [x] English/French UI, help, dialogs, and native menus
- [x] production Rust time-stretch from 50% to 200% with pitch preservation
- [x] independent Rust pitch shift from -12 to +12 semitones
- [x] 1-cent fine tuning and frequency-based pitch regression test
- [x] automatic BPM analysis with a persistent per-track cache
- [x] editable and persisted BPM with automatic-analysis fallback
- [x] button and keyboard tap tempo with outlier-resistant interval averaging
- [x] visible beat grid with an editable beat-one anchor and 10 ms nudging
- [x] Rust real-time metronome synchronized to speed changes, seeks, and loops
- [ ] background job queue with cancellation
- [x] optional HTDemucs standard four-stem separation in native Rust
- [x] Metal/Vulkan/WebGPU inference backend through Burn
- [x] versioned, source-fingerprinted per-track stem cache
- [x] sample-synchronous vocals/drums/bass/other real-time mixer
- [ ] benchmark HTDemucs cold start and full-song inference across supported hardware
- [ ] cancellation and queued separation jobs when switching tracks during inference
- [ ] CPU/memory/audio profiling report

## Phase 1 — durable core

- [ ] portable and referenced media modes
- [ ] project relinking and migrations
- [x] playlist naming and persistence
- [x] inline track-title editing and persistent drag-and-drop playlist ordering
- [x] per-track position, tempo, volume, and loop persistence
- [ ] playlist reordering and deletion
- [ ] crash-safe recovery and backups
- [ ] audio-device diagnostics
- [ ] CI builds on macOS, Linux, and Windows

## Phase 2 — learning workflow

- [x] detailed and overview waveform navigation
- [x] reliable transport, seek, volume, and five-second jumps
- [x] restart, advance, and stop end-of-track modes
- [x] A/B loop with visible boundaries
- [ ] configurable jumps and optional loop restart delay
- [x] tempo control with pitch preservation in the initial webview prototype
- [x] restore tempo control in the Rust engine with production DSP
- [x] production DSP for independent tempo and pitch
- [x] loop trainer with configurable repetitions, increments, and target tempo
- [x] Loop Trainer compatibility with A/B loops and complete-track normal playback
- [x] keyboard shortcuts for playback, jumps, and loop points
- [x] automatic BPM detection and cached display
- [x] editable BPM, synchronized metronome, and beat grid
- [ ] time signatures, draggable grid gesture, markers, and sections
- [x] persisted and synchronized playback state

## Phase 3 — analysis and models

- [ ] model manager and compatibility validation
- [x] stem cache and four-channel vertical stem mixer
- [ ] chord analysis and editable chord grid
- [ ] PDF export

## Phase 4 — import and distribution

- [ ] safe ZIP import
- [ ] isolated `yt-dlp` jobs with high-quality MP3 defaults
- [ ] macOS Apple Silicon packaging first
- [ ] Linux packaging second
- [ ] Windows packaging third
