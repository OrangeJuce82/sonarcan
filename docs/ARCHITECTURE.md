# Architecture

## Principles

SonArcan is built around three non-negotiable properties: stable real-time audio, reproducible failures, and durable project data. Tauri is a desktop and IPC boundary, not the application core.

```text
Svelte UI
   │ typed commands and versioned events
Tauri boundary
   │
Rust application services
   ├── project domain
   ├── audio engine
   ├── analysis workers
   ├── model workers
   └── diagnostics
```

The Rust core must remain usable and testable without a webview. UI commands validate input, call a core service, and serialize a result. Business rules do not belong in Tauri command handlers or Svelte components.

## Frontend structure

`App.svelte` is the composition shell, not a destination for new domain logic.
Reusable visual controls live in `src/lib/*.svelte`; serialized contracts live in
`src/lib/types.ts`; IPC calls live in `src/lib/backend.ts`; pure formatting,
waveform reduction, beat-grid projection, and similar calculations live in typed
modules with Node tests. Larger UI responsibilities are extracted only after
their state and event boundary is stable.

House components remain lightweight and dependency-free. Shared CSS custom
properties define the visual language. Every interactive component implements
keyboard operation, visible focus, disabled state, and accessible naming where
native semantics are insufficient.

## Frontend boundary

The frontend uses TypeScript interfaces matching serialized Rust DTOs. Commands are appropriate for finite request/response operations such as opening a project. Versioned events or channels will be used for job progress and playback state.

Raw audio buffers and full-resolution waveform data must not cross the JSON IPC boundary. The UI receives bounded visualization data, metadata, or references to cached artifacts.

The webview is strictly a control surface. It never decodes audio or owns playback timing, looping, gain, time-stretching, or pitch-shifting. Those operations always run in Rust; TypeScript only sends control parameters and displays snapshots of engine state.

## Localization

User-facing interface and help text use typed English and French catalogs. The saved language preference defaults to the operating-system language and rebuilds the native Tauri menu immediately when changed. Internal identifiers, project manifests, logs, and source code remain in English.

## Project format

The package format starts at version `1`. Every manifest read validates the version before exposing the project to the application. Writes use a temporary sibling file followed by an atomic rename.

Imported media is copied into the project `Audio/` directory and the original source path is retained for duplicate detection and future relinking. Relative manifest paths and rebasing after a package move will be implemented before the format is considered stable.

Playback now uses the dedicated Rust/CPAL engine. The earlier webview media prototype has been removed.

Project manifests are untrusted input. A stored media path is canonicalized and
must resolve to a regular file below the package's `Audio` directory before audio
read or deletion. This also prevents symlink and traversal escapes.

## Real-time audio contract

The audio callback must never:

- perform file or network I/O;
- allocate an unbounded amount of memory;
- wait on a blocking mutex;
- invoke Tauri or touch the UI;
- run analysis or model inference;
- emit synchronous logs.

Control messages will use bounded queues and preallocated buffers. Dropout and underrun counters will be exported through non-real-time diagnostics.

UI polling is bounded and guarded against overlapping requests. Prefer versioned
events when they reduce IPC frequency without introducing callback work. Optional
analysis and model initialization stays lazy; the selected track is loaded first,
and background cache warming must yield to active user work.

## Error model

Expected failures use typed Rust errors and are serialized as actionable messages at the Tauri boundary. A corrupt file, missing source, unsupported format, failed model, or cancelled job must not terminate the process.

The application console is a bounded diagnostic view, not a real-time sink. Rust `tracing` events and forwarded WebView `console.*` calls are retained in memory outside the audio callback. The native View menu exposes the hidden-by-default bottom panel. External-tool failures retain both a concise user-facing explanation and their bounded technical output.

## Import pipeline

Local paths and remote sources enter one Rust-owned background queue. Each queued item retains its destination project and the preference snapshot active when it was submitted, so concurrent imports cannot leak into another project. Concurrency and batch size are bounded.

Supported local media is copied directly when it already matches the requested audio shape. Otherwise FFmpeg performs one conversion before project import. Remote media is extracted by `yt-dlp` directly into the selected final audio format, avoiding a second conversion pass. Automatically downloaded `yt-dlp` releases are checked against the publisher's SHA-256 manifest before execution.

## Planned module extraction

As the Rust code grows, the project domain, audio engine, DSP, workers, and diagnostics will move into focused workspace crates. Extraction should happen when boundaries are proven by code, not before.
