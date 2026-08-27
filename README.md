# SonArcan

> Dive into the music.

SonArcan is a professional desktop workspace for musicians who need to learn, analyze, isolate, and rehearse songs from a band playlist. It is intentionally not a DAW.

## Current status

The repository contains the first Phase 0 vertical slice:

- Rust + Tauri 2 desktop shell;
- Svelte 5 + TypeScript user interface;
- versioned `.sac` project packages;
- atomic project manifest writes;
- WAV, MP3, and FLAC import validation;
- audio container probing with duration, sample-rate, and channel metadata;
- media copied into the project `Audio/` directory;
- playlist selection and working webview playback controls;
- duplicate source detection;
- structured Rust logging;
- local diagnostics;
- project-format unit tests;
- initial single-window workspace UI.

Audio playback, waveform extraction, DSP, model inference, and chord analysis are planned next. Placeholder controls in the UI intentionally do not pretend that these systems are already implemented.

## Prerequisites

- Node.js 22 or newer;
- Rust stable 1.78 or newer;
- platform dependencies required by Tauri 2.

See the official Tauri prerequisites for macOS, Linux, or Windows before building the desktop application.

## Development

```bash
npm install
npm run check
npm run tauri dev
```

Run the frontend alone when working on layout:

```bash
npm run dev
```

Run the Rust checks from the Tauri directory:

```bash
cd src-tauri
cargo fmt --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test
```

## Project packages

A SonArcan project is an inspectable directory with a `.sac` extension:

```text
My-Band.sac/
├── project.json
├── Audio/
├── Stems/
├── Analysis/
├── Chords/
└── Cache/
```

`project.json` is versioned and human-readable. Regenerable cache data is kept separate from user-authored project data.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Development roadmap](docs/ROADMAP.md)
- [Practice workflow](docs/PRACTICE_WORKFLOW.md)
- [Project management](docs/PROJECT_MANAGEMENT.md)
- [Contributing and debugging](docs/DEVELOPMENT.md)
- [Product specification](CAHIER_DES_CHARGES.md) — currently written in French; implementation documentation and source code are in English.
