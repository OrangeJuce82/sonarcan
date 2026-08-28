# Real-time audio engine

## Loading and decoded-audio cache

Compressed media is decoded on Tauri's blocking worker pool, never on the UI thread or the CPAL callback. The engine keeps a metadata-validated LRU cache of up to three recently used decoded tracks, capped at 384 MiB. Playback, waveform generation, and tempo analysis share the same immutable PCM data and coordinate in-flight requests, so selecting one track never starts several identical decoders.

Decoded samples are also written asynchronously to `Cache/decoded` in a compact binary PCM format. Its header fingerprints the source size and nanosecond modification time and records channels, sample rate, and frame count. A valid cache bypasses compressed-media decoding on later sessions. The remaining playlist is warmed sequentially in the background after the active track is ready, avoiding competing decoder jobs.

Rapid selections use monotonically increasing load generations. A slow, obsolete decode may populate the cache, but it cannot replace the newer track selected by the user.

The real-time callback never locks or reads this cache. It only sees the selected immutable audio buffer through `ArcSwap`.

## Optional HTDemucs stem mode

Stem mode is disabled by default and never delays ordinary track loading. Enabling it starts the pinned HTDemucs standard model on a dedicated Rust worker. The model runs through Burn's native WGPU backend (Metal on macOS and Vulkan-compatible adapters on Linux and Windows); Python and the webview never receive or process audio samples.

The 84 MB model is downloaded on first use to the application cache and then reused. HTDemucs emits vocals, drums, bass, and other in one inference pass. Completed stems are normalized to the source frame count and committed under `Stems/<track-id>/` as stereo float PCM plus a JSON manifest. The cache key covers the cache format, HTDemucs model revision, track identifier, source size, and nanosecond modification time. A temporary sibling file and atomic rename prevent partial caches from being treated as valid.

When the cache becomes ready, the engine swaps an immutable four-buffer stem set into the callback. Per-stem gain, mute, and solo values are atomics. The callback reads them without locks or allocation, sums the four aligned samples, and feeds that mix into the existing loop, time-stretch, pitch, metronome, and master-gain chain. Disabling stem mode atomically returns playback to the original decoded mix.

The current model revision is pinned in `Cargo.toml`. Updating it requires changing the stem cache revision and repeating separation parity and performance tests.

When looping is enabled, the Rust `play` command atomically positions playback at A before enabling the callback. This rule is enforced by the engine rather than simulated by the frontend.

## Ownership

Playback is owned by Rust and CPAL. The webview never schedules loop boundaries and never owns the audio clock.

On macOS, the CoreAudio stream remains on a dedicated `sonarcan-audio` thread because the native stream is thread-affine. Tauri stores only a thread-safe control facade backed by atomic state.

## Real-time callback contract

The output callback:

- performs no file or network I/O;
- takes no mutex;
- performs no heap allocation;
- does not call Tauri or Svelte;
- reads immutable decoded audio through `ArcSwap`;
- reads transport commands through atomics;
- reports output errors through an atomic counter.

## Looping

A/B positions are converted to source frames. The callback wraps the source position before writing the next output frame, so there is no timer, seek request, or empty buffer between B and A.

A five-millisecond equal-gain boundary crossfade is applied before B. The crossfade is shortened automatically for very small loops. This removes clicks caused by unrelated waveform phases at A and B while keeping the loop continuous.

## Resampling

Linear interpolation currently handles source/output sample-rate differences. This is sufficient for transport validation but will be replaced by the selected production resampler during the DSP benchmark phase.

## Time stretch and pitch shift

The callback owns a preconfigured Signalsmith Stretch processor and all of its input, output, and preroll buffers. The UI sends only atomic control values:

- playback speed from 50% to 200%, independently of pitch;
- pitch transposition from -12 to +12 semitones, independently of speed.
- fine pitch correction in 1-cent increments (`0.01` semitone) for slightly detuned recordings.

Unity speed with zero transposition takes a direct low-overhead path. Other settings stream source frames through the DSP processor. A seek, track change, or loop restart resets and prerolls the processor from the requested source position, avoiding stale buffered audio. Speed and pitch are persisted in each track's practice state.

## Tempo analysis

Automatic BPM analysis runs outside the real-time callback using the shared decoded PCM. It derives a short-hop energy-onset envelope and evaluates normalized autocorrelation candidates from 60 to 200 BPM. Results and confidence are stored under `Analysis/tempo/<track-id>.json`, so reopening a project or revisiting a track does not repeat the analysis.

## Beat grid and metronome

Each track can override the automatically detected tempo with an editable grid BPM from 30 to 300 BPM. A source-time offset identifies beat one. Both values are part of the track practice state, together with the metronome enabled state and volume.

The metronome is synthesized directly in the CPAL callback. It performs no allocation, locking, or IPC. Beat phase is derived from the current source position, BPM, and grid offset, which keeps the click aligned after seeks and A/B loop wraps. Playback speed changes the real-time spacing between clicks automatically while preserving alignment with the source waveform. Every fourth beat is accented; editable time signatures are a later roadmap item.

## Loop trainer

The real-time renderer counts training cycles with atomics. With A/B looping active, one cycle is a B-to-A wrap. In normal playback, one cycle is a complete track; the renderer restarts from the beginning with a short boundary crossfade. After a configurable number of cycles, it increases the playback rate by the configured step. Training stops automatically at the target rate. The callback never waits for the UI to schedule an increment, so continuity is preserved. The enabled state, repetition count, increment, and target are persisted per track.

## End-of-track behavior

Outside an active A/B loop, the engine supports three explicit modes: restart the current track with a boundary crossfade, signal the frontend to advance to the next preloaded playlist item, or stop. A monotonically increasing end generation prevents polling races when the advance signal is consumed. Active full-track Loop Trainer cycles temporarily take precedence; the selected end behavior resumes after training reaches its target.

## Spectrum worker

A dedicated `sonarcan-spectrum` Rust worker analyzes a 2,048-sample Hann window centered on the current source position. RustFFT produces the transform outside the audio callback. The result is reduced to 64 logarithmic bands from 30 Hz to the lower of 20 kHz or Nyquist and normalized to a bounded display range. Only these visualization magnitudes cross IPC; raw samples remain in Rust.

## Current limitations

- A complete decoded track is held in memory.
- Output-device changes require an engine restart.
- Automatic BPM is an estimate; manual correction and beat-grid alignment are not implemented yet.
- The UI polls lightweight atomic status; a versioned event stream will replace polling later.

These limitations are explicit roadmap items and are not hidden behind simulated controls.
