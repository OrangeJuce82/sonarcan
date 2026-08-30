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

Timed chord analysis follows the same boundary through a pinned LV-Chordia
Python worker. The worker reads the canonical original media, runs the learned
five-model ensemble, and emits four bounded timed-label sequences: the official
`ismir2017`, `submission`, and `full` dictionary decodes plus direct native
triad-head output. It does not use stems, tonal rules, the beat grid, or a
SonArcan decoder. Rust validates the four sequences, supervises cancellation,
rejects stale generations, and stores a source-identity-checked disposable
cache under `Analysis/chords`. Rust never changes an LV-Chordia chord decision.
No PCM or frame-level probabilities cross JSON IPC.

The chord panel wraps segments into a vertically scrollable grid. Playback can
follow the active segment automatically. Standard (`submission`) is the default;
Essentiel, Fondamentaux, and Complet expose the other native model views. The
panel can filter the uncalibrated model score, color by score or root, show a
full-width keyboard, and switch to an alphabetical repertoire of unique chords.
`N` is retained in data and rendered as `-`.

The webview is strictly a control surface. It never decodes audio or owns playback timing, looping, gain, time-stretching, or pitch-shifting. Those operations always run in Rust; TypeScript only sends control parameters and displays snapshots of engine state.

The macOS window keeps its decorated native title bar explicitly visible, with
the standard traffic-light controls and the `SonArcan` title. The application
header below it is fixed to the webview viewport rather than relying on a
scrolling sticky layer, so project controls and the output mixer remain visible
while the workspace is scrolled.

## Localization

User-facing interface and help text use typed English and French catalogs. The saved language preference defaults to the operating-system language and rebuilds the native Tauri menu immediately when changed. Internal identifiers, project manifests, logs, and source code remain in English.

## Project format

The package format starts at version `1`. Every manifest read validates the version before exposing the project to the application. Writes use a temporary sibling file followed by an atomic rename.

Imported media is copied into the project `Audio/` directory and the original source path is retained for duplicate detection and future relinking. Relative manifest paths and rebasing after a package move will be implemented before the format is considered stable.

Playback now uses the dedicated Rust/CPAL engine. The earlier webview media prototype has been removed.

Project manifests are untrusted input. A stored media path is canonicalized and
must resolve to a regular file below the package's `Audio` directory before audio
read or deletion. This also prevents symlink and traversal escapes.

When no recent project can be restored, the Rust project service immediately
creates a randomly named `.sac` package below the operating system's temporary
directory. No creation dialog blocks startup or the New Project action. This
temporary package is persisted continuously and remembered like any other
project, so it reopens after a restart while it still exists. If the operating
system removed it, startup reports the unavailable path, forgets that stale
entry, and creates a fresh temporary project without failing.

Save promotes the current temporary package through Save As to a user-selected
`.sac` destination; Save As always creates a copy. A destination cannot already
exist or be nested inside its source package. Native application exit and window
close requests are intercepted while the active project remains temporary, and
the UI offers Save and Quit, Quit Without Saving Elsewhere, or Cancel. Choosing
to quit without promotion leaves the temporary package available for the next
startup, subject to normal operating-system temporary-file cleanup.

Activating another project invalidates every in-flight track load before clearing
the waveform, transport, loop, tempo, spectrum, stem, and meter state. The audio
engine is paused and its track controls return to defaults, so a late waveform or
status response cannot repopulate an empty project. When tracks exist, the UI
restores the last selected track from bounded local user state and falls back to
the first track if that selection no longer exists. This selection is a UI
convenience and is deliberately not stored in the portable project manifest. An
empty project replaces the complete practice workspace with an import action.

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

Global user-facing feedback is rendered as a bounded stack of at most three
overlay toasts, never as a page-level banner that shifts the workspace. Toasts
use the same information, warning, and error color language as the application
console, add a success level for completed user actions, dismiss automatically
after the saved one-to-ten-second user preference (three seconds by default),
pause while hovered or focused, and always expose a close button. They use a
compact title/icon presentation with an optional two-line detail and animate
briefly when entering or leaving. Validation, progress, and failures owned by a
specific panel remain inline, while decisions
that can lose data remain modal. Import batches emit one terminal summary rather
than one notification per track.

The application console is a bounded diagnostic view, not a real-time sink. Rust `tracing` events and forwarded WebView `console.*` calls are retained in memory outside the audio callback. The native View menu exposes the hidden-by-default bottom panel. External-tool failures retain both a concise user-facing explanation and their bounded technical output.

The six-stem MLX process is an implementation detail behind the Rust stem service. It receives only canonical project media/model paths through a direct argument array and returns bounded NDJSON status; raw audio never crosses Tauri IPC. Debug builds resolve the locked uv environment. Release builds resolve the preassembled Python/MLX runtime and model from signed application resources, and never bootstrap uv or packages on the user's machine.

The stem mixer persists its six display names and control state in each track. Its header switch changes an atomic Rust bypass while retaining the immutable decoded stem buffers. The WebView receives only six bounded peak scalars in the normal audio-status snapshot; it never receives stem audio.

## Import pipeline

Local paths and remote sources enter one Rust-owned background queue. Each queued item retains its destination project and the preference snapshot active when it was submitted, so concurrent imports cannot leak into another project. Concurrency and batch size are bounded.

The application never reads or monitors the system clipboard. Text enters the
Import Center only through an explicit paste or drop initiated by the user.
Plain-text YouTube searches are resolved into groups of at most five candidates;
each group keeps its original query visible. A group containing one result is
selected, while a group containing several results starts with no selection.
No download begins until the user confirms the current selection. The eventual
`yt-dlp` fallback for an explicitly submitted unresolved search is `ytsearch1`.
Text edits are debounced, and normalized query results plus in-flight requests
are reused from a bounded session cache. Reordering an unchanged line therefore
performs no network request, and selections that still exist are preserved.
Uncached searches run sequentially to avoid request bursts. The Import Center
creates every query group immediately, reports indexed completed/total progress,
and publishes each group's candidates as soon as that query finishes. A failed
query remains isolated in its group and does not hide completed results or stop
later searches.

Supported local media is copied directly when it already matches the requested audio shape. Otherwise FFmpeg performs one conversion before project import. Remote media is extracted by `yt-dlp` directly into the selected final audio format, avoiding a second conversion pass. Release builds resolve the signed, pinned FFmpeg/FFprobe runtime from the application resources and pass its directory explicitly to `yt-dlp`; development builds may fall back to a system FFmpeg. Automatically downloaded `yt-dlp` releases are checked against the publisher's SHA-256 manifest before execution.

Duplicate prevention has two deliberately separate layers. Text analysis removes
obvious repeats by normalized URL, search text, or case-insensitive local
filename so the selection UI stays concise. The project service remains the
authority: before copying media it compares a Chromaprint acoustic fingerprint
of at most the first ten seconds, plus total duration, against every existing
track and every item in the same batch. This identifies the same recording after
MP3, WAV, or FLAC conversion while keeping CPU and cache size bounded;
the job fails visibly instead of silently skipping the file. Fingerprints are
versioned caches under `Analysis/fingerprints/`, never part of the project DTO or
JSON IPC payload, and are lazily rebuilt for older projects.

## Planned module extraction

As the Rust code grows, the project domain, audio engine, DSP, workers, and diagnostics will move into focused workspace crates. Extraction should happen when boundaries are proven by code, not before.
