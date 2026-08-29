# Real-time audio engine

## Loading and decoded-audio cache

Compressed media is decoded on Tauri's blocking worker pool, never on the UI thread or the CPAL callback. The engine keeps a metadata-validated LRU cache of up to three recently used decoded tracks, capped at 384 MiB. Playback, waveform generation, and tempo analysis share the same immutable PCM data and coordinate in-flight requests, so selecting one track never starts several identical decoders.

Decoded samples are also written asynchronously to `Cache/decoded` in a compact binary PCM format. Its header fingerprints the source size and nanosecond modification time and records channels, sample rate, and frame count. A valid cache bypasses compressed-media decoding on later sessions. The remaining playlist is warmed sequentially in the background after the active track is ready, avoiding competing decoder jobs.

Rapid selections use monotonically increasing load generations. A slow, obsolete decode may populate the cache, but it cannot replace the newer track selected by the user.

The real-time callback never locks or reads this cache. It only sees the selected immutable audio buffer through `ArcSwap`.

## Optional HTDemucs 6s stem mode

Stem mode is disabled by default and never delays ordinary track loading. On an Apple-silicon Mac, enabling it starts the private `sonarcan-mlx-worker` process with the exact `demucs-mlx` environment from `tools/sonarcan-mlx-worker/uv.lock`. Development uses uv-managed Python 3.13.5. Release assembly copies uv's complete standalone CPython distribution and synchronizes the locked production packages into it; uv is not installed or executed on the user's Mac.

The pinned `htdemucs_6s` model is supplied as a release resource and verified against the SHA-256 in its config before MLX loads it. The worker emits bounded newline-delimited JSON for stage changes, segment progress, logs, errors, and completion. Rust supervises and can terminate the child process, treats every event and output path as untrusted, and accepts only the exact vocals, drums, bass, other, guitar, piano contract.

Completed WAV stems are decoded and aligned to the source sample rate and frame count before being committed under `Stems/<track-id>/` as stereo float PCM plus a JSON manifest. The cache key covers the cache format, model revision, track identifier, source size, and nanosecond modification time. Only after all six stems validate does the engine swap an immutable six-buffer set into the callback. Per-stem gain, pan, mute, solo, bypass, and peak values remain atomic and allocation-free in the callback. Bypass selects the original immutable audio buffer without releasing the stem set, enabling immediate original/mix comparisons.

The master volume is the final output gain: it applies to the combined music,
stem mix, and metronome signal. Master volume changes, mute/unmute transitions,
stem gain/pan changes, and stem mute/solo changes use a 40 ms callback-side ramp to
avoid clicks and zipper noise. The ramp keeps the real-time path allocation-free
and lock-free.

Interactive seeks are coalesced by the UI to at most one in-flight IPC request,
while the position readout continues to follow the pointer immediately. Each
accepted position generation is joined to the last output frame by an 8 ms
callback-side transition. Rapid scrubbing therefore cannot build an IPC backlog
or send a discontinuous sample step to the output device; the transition uses
only buffers allocated when the stream is created.

The bounded `AudioStatus` snapshot exposes the decaying master peak and
independent left/right output peaks for the UI meters. These scalar values are
the only output-level data crossing IPC; raw audio never leaves the engine.

Python, uv, worker dependencies, and the model revision are pinned in the worker project and `stem_contract.rs`. Updating any of them requires regenerating the lockfile and runtime, changing the cache revision when output compatibility changes, and repeating separation parity and performance tests.

`demucs-mlx 1.4.6` rejects a numeric key found only in the official checkpoint's unused `training_args` metadata. The release model builder strips that one optional metadata field before invoking the package's restricted loader and converter. Constructor data, tensor state, official signature, source checksum, and generated safetensors checksum continue through the upstream validation path. Remove this narrow workaround when the pinned upstream version accepts its official checkpoint unchanged.

When looping is enabled, playback may start anywhere in the track. A position
before A is preserved as a lead-in, while a position at or after B is moved to A
when Play is pressed. Once playback reaches B, the Rust callback wraps to A.
This rule is enforced by the engine rather than simulated by the frontend.

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

The callback owns one preconfigured Signalsmith Stretch processor and all of its input, output, and preroll buffers. When stems are active, their gain/pan/mute/solo mix is computed first; the resulting stereo mix passes through this processor exactly once. Peak values for the six UI meters are reduced inside the current block and published through six atomics only once per callback. The UI sends only bounded control values:

- playback speed from 50% to 200%, independently of pitch;
- pitch transposition from -12 to +12 semitones, independently of speed.
- fine pitch correction in 1-cent increments (`0.01` semitone) for slightly detuned recordings.

Unity speed with zero transposition takes a direct low-overhead path. Other settings stream source frames through the DSP processor. A seek, track change, or loop restart resets and prerolls the processor from the requested source position, avoiding stale buffered audio. Interactive speed and pitch changes retain the streaming state and follow a 40 ms exponential transition, preventing repeated resets and discontinuities while playback continues. Signalsmith restores neutral transposition during transport resets, so the engine deliberately reapplies the selected pitch afterwards. Speed and pitch are persisted in each track's practice state.

The UI updates its readout immediately and applies a 65 ms trailing debounce before crossing IPC. Button steps are 5% for speed and one semitone for pitch; Shift-click selects the fine 1% or one-cent step respectively.

## Tempo analysis

Automatic BPM analysis runs outside the real-time callback using the shared decoded PCM. It derives a short-hop energy-onset envelope and evaluates normalized autocorrelation candidates from 60 to 200 BPM. Results and confidence are stored under `Analysis/tempo/<track-id>.json`, so reopening a project or revisiting a track does not repeat the analysis.

## Beat grid and metronome

Each track can override the automatically detected tempo with an editable grid BPM from 30 to 300 BPM. A source-time offset identifies beat one. Both values are part of the track practice state, together with the metronome enabled state and volume.

The metronome is synthesized directly in the CPAL callback. It performs no allocation, locking, or IPC. Beat phase is derived from the current source position, BPM, and grid offset, which keeps the click aligned after seeks and A/B loop wraps. Playback speed changes the real-time spacing between clicks automatically while preserving alignment with the source waveform. Every fourth beat is accented; editable time signatures are a later roadmap item.

## Loop trainer

The real-time renderer counts training cycles with atomics. With A/B looping
active, one cycle is a complete A-to-B pass followed by the B-to-A wrap. A
lead-in before A is allowed but never counted as a repetition; a partial pass
that starts inside the loop is also completed once before it can be counted.
In normal playback, one cycle is a complete track; the renderer restarts from
the beginning with a short boundary crossfade. After a configurable number of
cycles, it increases the playback rate by the configured step. Training stops
automatically at the target rate. The callback never waits for the UI to
schedule an increment, so continuity is preserved. The enabled state,
repetition count, increment, and target are persisted per track.

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
