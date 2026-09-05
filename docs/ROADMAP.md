# Development roadmap

**Current focus:** harden release qualification, background-task lifecycle,
project recovery, and cross-platform GPU editions.
**Last updated:** 2026-09-06

**Architecture invariant:** playback and every DSP operation run in the Rust real-time engine. TypeScript/Svelte is only a control surface and must never process audio.

## Phase 0 — technical validation

- [x] Tauri 2 + Svelte + TypeScript shell
- [x] typed command boundary
- [x] structured logging bootstrap
- [x] hidden-by-default bottom application console for colored Rust and WebView logs
- [x] native View menu console toggle
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
- [x] Play preserves a lead-in before A and restarts at A at or after B
- [x] contextual hover help for practice and transport controls
- [x] twelve-language UI, help, dialogs, native menus, and Arabic RTL layout
- [x] production Rust time-stretch from 50% to 200% with pitch preservation
- [x] independent Rust pitch shift from -12 to +12 semitones
- [x] 1-cent fine tuning and frequency-based pitch regression test
- [x] streaming pitch reset fix with a stem-mix-to-pitch integration regression test
- [x] debounced speed/pitch controls with click-free 40 ms real-time parameter smoothing
- [x] global header volume, mute, compact slider, and Rust-backed output peak meter
- [x] Beat This! beat/downbeat analysis with a source-aware disposable cache and indicative BPM
- [x] waveform grid driven by individual detected beat timestamps
- [x] Rust real-time metronome synchronized to detected beats, speed changes, seeks, and loops
- [x] bounded concurrent background import queue with progress and error diagnostics
- [x] user cancellation for imports plus supervised chord and stem worker termination
- [x] optional HTDemucs 6s separation through supervised MLX and portable Torch workers
- [x] pinned target-native CPython, MLX/CUDA/ROCm dependencies, uv lockfile, model, and release runtimes
- [x] versioned, source-fingerprinted per-track stem cache
- [x] sample-synchronous vocals/drums/bass/other/guitar/piano real-time mixer
- [x] structured worker logs, segment progress, failure reporting, and process cancellation
- [ ] benchmark HTDemucs cold start and full-song inference across supported Apple-silicon Macs
- [ ] queued separation jobs when switching tracks during inference
- [ ] CPU/memory/audio profiling report

## Phase 1 — durable core

- [ ] portable and referenced media modes
- [ ] project relinking and migrations
- [x] playlist naming and persistence
- [x] inline track-title editing and persistent drag-and-drop playlist ordering
- [x] per-track position, tempo, loop, trainer, and stem-mix persistence
- [x] global master/metronome volume and user preferences
- [x] playlist reordering and deletion with project-owned media/cache cleanup
- [ ] crash-safe recovery and backups
- [ ] audio-device diagnostics
- [x] CI native builds on macOS ARM64, Linux x64, and Windows x64

## Phase 2 — learning workflow

- [x] detailed and overview waveform navigation
- [x] reliable transport, seek, volume, and configurable time/beat/chord/lyrics navigation
- [x] restart, advance, and stop end-of-track modes
- [x] A/B loop with visible boundaries
- [x] configurable one-to-sixty-second jumps
- [ ] optional loop restart delay
- [x] tempo control with pitch preservation in the initial webview prototype
- [x] restore tempo control in the Rust engine with production DSP
- [x] production DSP for independent tempo and pitch
- [x] loop trainer with configurable repetitions, increments, and target tempo
- [x] Loop Trainer compatibility with A/B loops and complete-track normal playback
- [x] keyboard shortcuts for playback, jumps, and loop points
- [x] Beat This! beat/downbeat detection, indicative BPM, synchronized metronome, and waveform grid
- [x] LV-Chordia-compatible piano inversions and guitar/ukulele positions
- [ ] time signatures, markers, and sections
- [x] persisted and synchronized playback state

## Phase 3 — analysis and models

- [ ] model manager and compatibility validation
- [x] stem cache and six-channel vertical stem mixer
- [x] LV-Chordia chord analysis, three dictionary views, and bounded per-region corrections
- [x] piano, guitar, and ukulele chord views with validated positions
- [x] JAMS export of the effective chord timeline
- [x] plain, LRC, enhanced LRC, and TTML lyrics with optional LRCLIB lookup
- [ ] PDF export

## Phase 4 — import and distribution

- [ ] safe ZIP import
- [x] unified local-file, dropped-text, pasted-text, URL, playlist, and search Import Center
- [x] isolated, bounded `yt-dlp` jobs with verified tool download and high-quality MP3 defaults
- [x] query-grouped YouTube results with explicit download confirmation
- [x] one-pass remote extraction/conversion and conversion-free copy for conforming local media
- [x] structured `yt-dlp` diagnostics with actionable user guidance and in-app Rust logs
- [x] explicit pasted-text analysis without application clipboard monitoring
- [x] import cancellation and automatic completed-job pruning
- [ ] import retry controls
- [x] macOS Apple Silicon packaging with MLX
- [x] macOS Intel Light packaging after current LV-Chordia/Torch wheels stopped supporting Full
- [x] Linux x64 Light, NVIDIA CUDA, and AMD ROCm packaging
- [x] Windows x64 Light and NVIDIA CUDA packaging
