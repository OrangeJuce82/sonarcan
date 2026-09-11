# Architecture

The native inference migration and its release gates are specified in
[`EXECUTORCH_RUNTIME.md`](EXECUTORCH_RUNTIME.md). Until a feature satisfies
those gates, its existing qualified worker remains the production path.

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

Timed chord analysis follows the same boundary through a pinned Python worker.
The worker reads the canonical original media, runs the learned LV-Chordia
five-model ensemble, and emits three bounded timed-label sequences using the
official `ismir2017`, `submission`, and `full` dictionary decodes. A separate
pinned Beat This! `final0` model detects beats and downbeats. SonArcan retains
its official minimal timeline and the optional Beat This! madmom DBN timeline.
Only these two bounded timestamp sequences cross IPC; frame-level predictions
remain inside the worker. M toggles the metronome; Alt/Option+M and
Alt/Option-clicking the metronome switch between the minimal and DBN timelines. The
waveform, navigation, loop snapping, metronome, and tempo display always use the
selected timeline. DBN is the user-preference default; changing the mode while a
track is selected stores a per-track practice override without changing that global
default. The BPM display estimates the tempo around the
playhead with a robust local median, refreshes twice per second, and applies
playback speed so it represents the tempo currently heard without display
jitter. Downbeats only accent those rhythmic
views. The worker runs LV-Chordia first,
then Beat This!, so the two models do not compete for the same CPU or accelerator
and both results remain available downstream. If either model fails, the worker
retains the other model's bounded result and reports a partial-analysis warning;
partial results are not cached so selecting the track later retries the failed
model. The combined request fails only when neither model produces a result.
Beat This! never changes or splits
an LV-Chordia label, boundary, score, timeline card, or repertoire entry. Neither model uses
stems, UI beat visualization, tonal rules, or a SonArcan decoder. Rust validates the
sequences and downbeat positions, supervises cancellation,
rejects stale generations, and stores a source-identity-checked disposable
cache under `Analysis/chords`. Rust never changes an LV-Chordia chord decision.
No PCM or frame-level probabilities cross JSON IPC.

Heavy analysis is capability-gated once per application launch. On the first qualified launch,
SonArcan installs the pinned SCNet-large, HTDemucs, and Beat This! checkpoints sequentially
into the application-data cache, verifies their sizes and SHA-256 digests, and reports bounded
progress while the welcome screen remains responsive. Interrupted downloads use adjacent
temporary files and resume as a clean retry; subsequent launches verify the cache. LV-Chordia's
five pinned weights remain part of the shared runtime and are verified in the same startup flow.
Installation never constructs an inference model or keeps weights resident in memory.
SonArcan enables
Beat This!, LV-Chordia, and four-stem separation only after the platform backend
has been release-qualified and a bounded on-device inference probe succeeds.
The probe exercises the production accelerator and rejects invalid values,
timeouts, missing drivers, unavailable devices, and silent CPU fallback. Rust
keeps the result as session state and rejects analysis IPC when the probe has not
succeeded, independently of UI visibility. In degraded mode the UI does not
render Beat, Chords, Mix, BPM, or the analysis-driven metronome; navigation is
Time or Lyrics when synchronized lyric lines are available, while playback,
lyrics, spectrum, and the stereo meter remain available. Loop snapping follows
those synchronized lines in Lyrics mode. The explanation is persisted as a
once-per-user-profile notice.
The startup probe runs before model preparation. A missing or rejected GPU
therefore enters simplified mode without downloading analysis checkpoints.
Debian accelerator release builds pin `SONARCAN_GPU_BACKEND` to `nvidia`
or `amd`; source builds without that explicit qualification cannot accidentally
enable GPU analysis. The same application therefore enters
simplified mode without running an accelerator probe, downloading analysis
checkpoints, or exposing analysis commands. It preserves project analysis caches
for later use on qualified hardware.
Apple Silicon uses MLX for stems and MPS for chord/rhythm analysis. NVIDIA
Debian releases use CUDA 12.6, while AMD Debian releases use ROCm 7.2
through PyTorch's CUDA-compatible device API. All backends execute both model
probes on the end-user accelerator before Rust opens the analysis IPC gate.

In qualified GPU mode, the analysis workspace first places the chord grid beside a
multi-view harmony panel using a 40/60 split. Beneath it, the four-stem mixer sits
beside a right-hand column containing two equal-height, user-selectable visualization slots. Spectrum, output meter, and bounded energy history reuse bounded Rust snapshots without transferring raw audio. The chord panel wraps segments into a vertically
scrollable grid. Playback can follow the active segment automatically. Standard (`submission`) is the default;
Essentiel and Complet expose the other native model views. The
panel can filter the uncalibrated model score, color by score or root, show a
consistent sharp or flat spelling, follow the playback pitch transposition, and
switch between its timed grid, an alphabetical repertoire of unique chords, and
duration-weighted chord statistics. The statistics are a pure presentation
derivation of the displayed timeline and never change cached analysis.
In degraded mode, the lyrics panel occupies the mixer's column, the spectrum
and stereo meter retain the right-hand column, and the harmony row is omitted.
The audio header exposes one user navigation mode: Time, Beat, Chord, Marker, or Lyrics. Left
and Right and the transport jump buttons share that mode. Waveform clicks always
seek to the exact pointed position, independently of the navigation mode and loop
magnetism.
Time uses a configurable one-to-sixty-second step and defaults to ten seconds;
Beat, Chord, Marker, and Lyrics activate when their bounded navigation points become
available. The selector visibly remains on Time while the preferred mode is being
orchestrated, then switches automatically after valid points arrive. Unavailable
options are disabled and `N` cycles only the currently available modes. Lyrics
uses synchronized line starts including the saved display offset; Marker uses the
ordered starts of the current track's project markers and remains available in
degraded mode. Four non-interactive
states centered in the Audio header expose Beat This!, chord, lyrics, and separated-mix
orchestration. Left/Right and the transport jump controls move to the adjacent point.
Shift+Right selects the next playlist track. Shift+Left reuses the previous-track
transport behavior: it restarts the current track at or after one second and
selects the previous playlist track only while the cursor is before one second.
The preference is global user state and is never stored in a project or track.
Clicking a timed chord, marker, or lyric seeks to its timestamp without changing the
selected navigation mode. Loop magnetism uses chord boundaries in Chord mode,
synchronized line starts in Lyrics mode, marker starts in Marker mode, and Beat This! beats in Time or Beat
mode, falling back to beats while chord data is unavailable. `I` cycles the
piano, guitar, ukulele, and lyrics views. Global shortcuts remain inactive while
editing text.
The detailed waveform places those same visible chord segments in a compact,
clickable lane using the waveform viewport and playhead, so zooming, panning,
automatic follow, chord filtering, edits, and transposition remain synchronized.
Synchronized lyric lines use a second compact lane below the detailed waveform.
Its segments use the same viewport and saved lyrics offset, highlight the active
line, and seek directly on click without changing the selected navigation mode.
When both Beat This! and chord analysis are available, the chord lane shows a neutral
beat-count badge for each playable chord. Each detected beat is assigned once: a
chord boundary is associated with the closest active beat, using the midpoint
between adjacent beats as the decision boundary, and chord ranges are half-open.
A beat near a shared boundary therefore belongs to the following chord even when
the model transition is slightly early or late, without changing either analysis
result. The waveform lane and timed chord grid render the same derived count.
The per-track four-state beat-grid control applies one reversible presentation
layer to the selected raw or DBN Beat This! timeline. Off passes that selected
timeline through unchanged. Auto follows the dominant interval cluster; Eighth
selects the coarser detected octave and Sixteenth its denser counterpart. Only
sustained intervals close to a 2:1 ratio are changed in the three active modes:
dense runs retain alternating source beats with downbeat-aware phase selection,
while sparse runs gain midpoint beats that are never promoted to downbeats. The
derived timeline is shared by BPM display, metronome, navigation, loop snapping,
waveform markers, and chord beat-count badges; cached analysis stays untouched.
The harmony panel preserves each source JAMS/Harte label and uses one pure,
typed parser for its three-octave piano and validated guitar and ukulele
positions.
Piano chord tones and fretted-instrument fingering markers reuse the active
chord color, with theme-aware mixing for readable labels and root emphasis.
Piano, guitar, and ukulele positions prefer the pinned, MIT-licensed
`chords-db` corpus. SonArcan validates every published position against the
parsed LV-Chordia tones and rejects mismatched notes or slash basses. A bounded,
deterministic fretboard search fills missing guitar and ukulele coverage with
positions limited to chord tones and a four-fret hand window; unavoidable
omissions and omitted slash basses remain explicit in the UI. Generated
fingerings never claim a thumb-over technique. The complete horizontal
fretboard remains a theory view and shows every chord tone independently of the
selected playable position. Piano keeps validated corpus positions first and
synthesizes a complete position when needed; an explicit slash bass is always
placed alone below the full harmony so it remains the sounding bass. Each
instrument keeps a bounded position navigator.
`N` is retained in data and rendered as `-`.
User chord corrections remain separate from the disposable model cache. Simple
label corrections use the bounded per-track overlay keyed by LV-Chordia
vocabulary and native segment times. The first structural edit materializes a
bounded user timeline for the active detail mode, with stable UUIDs and
non-overlapping regions. Moving, adding, or deleting those regions therefore
never rewrites the underlying LV-Chordia output, and resetting edits restores
the detected timeline.
Each track also owns a bounded plain-text practice note in the same persisted
practice state. The note editor sits below the playlist; the playlist consumes
the remaining height of the left column and scrolls independently.
When an older project is opened, legacy numeric practice values are bounded to
the current playback, training, loop, and stem-mix contracts before reaching
the UI; already valid values and user-authored timeline data remain unchanged.

Track practice state also stores at most 512 named timeline markers. A marker
has a stable UUID, a start, an optional explicit end, and an origin (`source`,
`detected`, or `user`). Missing ends are derived from the next marker or the
track duration in the presentation layer. Marker labels and timestamps are
validated in Rust before the manifest is saved. Markers, chords, and
synchronized lyrics share one waveform interaction: click selects and seeks,
double-click edits the block text, edge handles change its timing, Delete removes
the selected block, and right-click opens an explicit menu containing the remove
action. The trailing plus creates a block at the playhead ending at the next
block or track end. Double-clicking an edge snaps it to the adjacent block or
track boundary. Waveform chord editing reuses the grid's complete validated
option list and its filtering, pointer, keyboard, wheel, and Shift-to-replace-all
semantics. During an edge drag, Shift applies a separate ten-pixel cross-lane
magnet to starts and ends from the other two categories; it does not read or
change the loop magnet preference. Timeline edits remain non-overlapping and
never change canonical audio.

Lyrics are an optional per-track document stored under `Lyrics/<track-id>.json`.
The versioned, bounded DTO supports plain text, line timing, word timing, source
attribution, and a user-controlled display offset. The Svelte lyrics panel owns
presentation, navigation, and editing; Rust validates and atomically persists the
document outside the audio callback. Inline editing accepts UTF-8 plain text,
LRC, enhanced LRC, and TTML syntax. Malformed synchronized timestamps and timing
outside the current audio duration are rejected. LRCLIB provides the optional
zero-account lookup boundary.
A selected match is stored in the project so playback remains independent of the
service and works offline. Automatic lookup removes parenthesized or bracketed
qualifiers and standalone punctuation from the track title, then makes at most
three increasingly broad searches by dropping one trailing word per pass. A synchronized
result from any pass takes priority; otherwise the first available plain-text result set
is used. Plain text follows playback with a linear visual estimate but never exposes
Lyrics navigation points. Manual search remains available. Provider identifiers and attribution
remain attached until an edit creates a local copy. The native Songs menu exports lyrics as
synchronized LRC (including enhanced word timing) or as a simple Markdown
document. No credential or network request is stored in a project.

The webview is strictly a control surface. It never decodes audio or owns playback timing, looping, gain, time-stretching, or pitch-shifting. Those operations always run in Rust; TypeScript only sends control parameters and displays snapshots of engine state.

The macOS window keeps its decorated native title bar explicitly visible, with
the standard traffic-light controls and the `SonArcan` title. The application
header below it is fixed to the webview viewport rather than relying on a
scrolling sticky layer, so project controls and the output mixer remain visible
while the workspace is scrolled.

## Localization

User-facing interface, help text, accessibility labels, dialogs, and native menus use complete typed catalogs for English, French, Spanish, German, Portuguese, Italian, Simplified Chinese, Japanese, Korean, Arabic, Hindi, and Indonesian. The saved language preference defaults to the first supported operating-system language, rebuilds the native Tauri menu immediately when changed, and switches document direction for Arabic. Automated catalog-parity tests reject missing or extra message keys. Internal identifiers, project manifests, logs, technical format names, and source code remain in English.

## Project format

The package format starts at version `1`. Every manifest read validates the version before exposing the project to the application. Writes use a temporary sibling file followed by an atomic rename.
Save As asks for explicit confirmation before replacing an existing `.sac`.
It builds a complete sibling package before publication, moves the old package
aside, and restores it if publication fails. Arbitrary directories and symbolic
links are never accepted as overwrite targets.

Imported media is copied into the project `Audio/` directory and the original source path is retained for duplicate detection and future relinking. Relative manifest paths and rebasing after a package move will be implemented before the format is considered stable.

Playback now uses the dedicated Rust/CPAL engine. The earlier webview media prototype has been removed.

Project manifests are untrusted input. A stored media path is canonicalized and
must resolve to a regular file below the package's `Audio` directory before audio
read or deletion. Containment uses filesystem identity when macOS preserves
different Unicode spellings for the same canonical path, while continuing to
prevent symlink and traversal escapes.

When no recent project can be restored, the Rust project service immediately
creates a randomly named `.sac` package below the operating system's temporary
directory. No creation dialog blocks startup or the New Project action. This
temporary package is persisted continuously and remembered like any other
project, so it reopens after a restart while it still exists. If the operating
system removed it, startup reports the unavailable path, forgets that stale
entry, and creates a fresh temporary project without failing.
While this startup work is pending, the header distinguishes checking recent
projects, restoring the latest project, and creating a temporary project.
The application shell is not rendered until saved user preferences have been
loaded and applied and the startup project has been activated. A neutral branded
bootstrap view avoids exposing default language, theme, audio settings, or an
empty-project workspace before the configured interface is ready. Once the
language is known, that view reports the localized project-loading state.
The preferred chord-analysis vocabulary is also loaded at this boundary;
Essential is the default for new and legacy preference profiles.

On macOS, opening an associated `.sac` package can deliver the native document
event before Tauri has run application setup. That path is retained in a small
process-level queue that does not access managed Tauri state. Startup consumes
it only after the accelerator capability probe and model preparation decision,
while an already-running application also receives an event to activate the
requested project immediately. A document-open event received during startup
therefore cannot activate a project against the initial simplified-mode UI state.

Save promotes the current temporary package through Save As to a user-selected
`.sac` destination; Save As always creates a copy. A destination cannot already
exist or be nested inside its source package. Native application exit and window
close requests are intercepted while the active project remains temporary, and
the UI offers Save and Quit, Quit Without Saving Elsewhere, or Cancel. Choosing
to quit without promotion leaves the temporary package available for the next
startup, subject to normal operating-system temporary-file cleanup.

The native Open dialog selects `.sac` packages as documents rather than exposing
their internal directories. On macOS, that package selection is the complete
authorization interaction: SonArcan verifies that the package is writable and
that its manifest and every referenced audio file are readable before activation,
and reports a failure directly without opening a second folder picker. Save As
similarly verifies that its selected parent is writable before copying.
Cancellation and failed verification are shown as actionable toasts, and a failed
copy removes only the destination package that SonArcan created for that operation.

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
analysis and model initialization stays lazy; the selected track is loaded first.
The frontend background scheduler serializes decoded-cache warming, prioritizes
the next playlist track, and pauses new warming tasks while loading, analysis,
stem separation, or imports are active. Work already inside a decoder may finish,
but obsolete queued work is discarded when track or project selection changes.

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

The header resource indicator measures system-wide CPU and used physical memory,
so Python inference descendants and media tools remain included regardless of
their process topology. GPU utilization comes from the platform driver (Apple AGX,
`nvidia-smi`, or `rocm-smi`); all three meters therefore represent machine-wide pressure.
For RAM, the detail also reports the used amount in megabytes. Unsupported or
unavailable GPU telemetry is shown as unavailable rather than estimated. Sampling
stays outside the audio callback.

Four-stem inference is an implementation detail behind one Rust stem service.
After the startup capability probe succeeds, Apple Silicon selects the MLX
worker. NVIDIA Debian selects the CUDA worker and AMD Debian selects the
ROCm worker. CPU-only heavy analysis is not an accepted user experience, so a
failed accelerator probe closes the service gate. Both workers receive only canonical project media/model paths
through direct argument arrays and return the same bounded NDJSON protocol.
Both expose Fast HTDemucs and HQ SCNet Large by starrytong behind the same four-output
contract. No profile is preselected or stored as a user preference. On first
use, the worker downloads the selected upstream checkpoint into the shared
application-data model cache, then accepts it only when its exact byte length
and full SHA-256 match the pinned contract. Checkpoints use Torch's tensor-only
loader; Apple Silicon converts HTDemucs once to a safe MLX cache. Raw
audio never crosses Tauri IPC. Release builds resolve one target-native,
preassembled Python 3.13 runtime and never install packages on the user's
machine. It contains the chord/downbeat worker and exactly one stem backend,
but not either stem checkpoint, so
CPython, NumPy, SciPy, and PyTorch are not duplicated.

The stem mixer persists its four display names and control state in each track. Its header switch changes an atomic Rust bypass while retaining the immutable decoded stem buffers. Reset deletes both per-profile caches for the selected track, restores neutral controls, and returns the UI to the two explicit model choices. The WebView receives only four bounded peak scalars and bounded progress/ETA state; it never receives stem audio.

## Import pipeline

Local paths and remote sources enter one Rust-owned background queue. Each queued item retains its destination project and the preference snapshot active when it was submitted, so concurrent imports cannot leak into another project. Concurrency and batch size are bounded.

The application never reads or monitors the system clipboard. Text enters the
Import Center only through an explicit paste or drop initiated by the user.
Plain-text YouTube searches inspect up to ten metadata-only results and resolve
them into groups of the five most relevant candidates; each group keeps its
original query visible. Search results display a lazy YouTube thumbnail from a
single CSP-allowlisted origin. Thumbnail URLs are constructed only from the
validated video identifier, and a trailing bracketed identifier is removed from
the title only when it exactly matches that same identifier. The identifier is
shown as a separate link; selecting it asks Rust to construct and open the fixed
YouTube watch URL, while the thumbnail itself remains non-interactive. The
download output template deliberately omits the video identifier so it cannot
leak from the staging filename into the imported track title; playlist indices
continue to keep batch filenames distinct. A bounded local heuristic compares case-insensitive
word tokens so punctuation and artist/title order do not distort otherwise exact
matches. For an explicit `artist - title` query, the artist may match either the
video title or its channel. The verified-channel field emitted by the pinned
`yt-dlp` search represents both YouTube's verification check and its Official
Artist Channel music-note badge; it outweighs the much weaker textual hints in
channel names such as “Official”, “VEVO”, or “Topic”. Verification, popularity,
and search position remain limited to a small tie-break contribution. Unrequested
covers, live versions, remixes, karaoke, reactions, and tutorials remain penalized. Results
whose titles differ only by case, punctuation, spacing, or presentation markers
such as “Official Audio” collapse to the highest-ranked candidate. The UI exposes
the score as a relevance indicator rather than a calibrated probability. By default, the
best candidate is selected automatically; a global user preference can disable
that behavior, in which case only a group containing one result is selected.
No download begins until the user confirms the current selection. The eventual
`yt-dlp` fallback for an explicitly submitted unresolved search is `ytsearch1`.
Text edits are debounced, and normalized query results plus in-flight requests
are reused from a bounded session cache. Reordering an unchanged line therefore
performs no network request, and selections that still exist are preserved.
Uncached searches run with a strict concurrency limit of two. Starting a new
text analysis invalidates its generation and terminates obsolete `yt-dlp`
processes rather than merely ignoring their late results. The Import Center
creates every query group immediately, reports indexed completed/total progress,
and publishes each group's candidates as soon as that query finishes. A failed
query remains isolated in its group and does not hide completed results or stop
later searches.

Supported local media is copied directly when it already matches the requested audio shape. Otherwise FFmpeg performs one conversion before project import. Public remote media supported by `yt-dlp`, including YouTube, SoundCloud, Bandcamp, and Mixcloud URLs, is extracted directly into the selected final audio format, avoiding a second conversion pass. Authenticated, live, and upcoming content is intentionally excluded. Bounded clean info JSON is retained only inside the temporary download directory long enough to turn provider chapters into persisted source markers, then deleted with the staging directory. Text search offers the two native, reliable yt-dlp engines in scope: YouTube and SoundCloud; Bandcamp and Mixcloud remain direct-link providers. Every recognized provider candidate carries a bounded source URL and provider identity so the frontend can render its local logo and ask Rust to open only an allowlisted HTTPS provider URL. Search and download both prefer the pinned official `yt-dlp` zipimport artifact through SonArcan's shared Python 3.13 resolver; this avoids the standalone macOS executable's per-process self-extraction cost. The standalone executable remains only a compatibility fallback when the fast runtime is unavailable. Release builds resolve the signed, pinned FFmpeg/FFprobe runtime from the application resources and pass its directory explicitly to `yt-dlp`; development builds may fall back to a system FFmpeg. Downloaded fallback releases are checked against the publisher's SHA-256 manifest before execution.

On the August 30, 2026 Apple-silicon benchmark, the former 35 MiB standalone
macOS executable took 8.85 seconds for `--version` and 9.62 seconds for a
five-result search. The verified 3 MiB zipimport artifact took 0.51 seconds and
1.47 seconds respectively for the same operations. Search logs retain only
elapsed time and result count; the private query is never logged.

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
