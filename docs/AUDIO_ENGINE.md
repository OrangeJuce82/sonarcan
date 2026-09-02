# Real-time audio engine

## Loading and decoded-audio cache

Compressed media is decoded on Tauri's blocking worker pool, never on the UI thread or the CPAL callback. The engine keeps a metadata-validated LRU cache of up to three recently used decoded tracks, capped at 384 MiB. Playback and waveform generation share the same immutable PCM data and coordinate in-flight requests, so selecting one track never starts several identical decoders.

Development builds use light optimization for SonArcan and full optimization for third-party audio and DSP crates. On the 175-second MP3 used for the August 2026 loading benchmark, waveform availability improved from about 10.26 seconds with the default debug profile to about 208 ms (172 ms decode plus 36 ms reduction). The release build measured 127 ms overall (123 ms decode plus 4 ms reduction). Beat This! analysis starts only after the selected audio is ready, so it cannot compete with the initial decode. Release optimization remains unchanged.

Chord and downbeat recognition are not part of playback or decoded-audio
ownership. They start concurrently as independent tasks after the selected track
is ready and run in one supervised process containing LV-Chordia and Beat This!
models. Beat This! output never changes or splits the chord timeline. The
process can be killed when track selection changes. Model inference and official
decoding never execute on the CPAL callback. The worker reads the canonical
original media directly; it does not depend on stems, UI beat visualization,
or decoded playback PCM. The webview receives only the final bounded timed-chord
contract and seeks through the existing Rust transport command when a segment
is activated. Chord-card navigation targets the exact model timestamp. The UI
anticipates only the active-chord highlight by 10 ms so transport refresh and
sample rounding cannot briefly leave the preceding chord highlighted; stored,
displayed, and sought timestamps remain unchanged. Cache revisions track the
pinned model contract. The worker trims any sub-frame decoder/rounding overlap
to the following region's start before publishing results, and Rust rejects any
remaining overlap. Chord regions therefore always have ordered,
non-overlapping timing boundaries.

On the August 30, 2026 Apple-silicon integration check, Beat This! detected 75
downbeats in a 173-second MP3 in 6.47 seconds on MPS. The warmed combined
LV-Chordia and Beat This! worker completed the same track in 9.27 seconds.

Every in-flight decode is removed from the coordination set on both success and failure before waiting callers are notified. A damaged or unreadable media file therefore cannot leave waveform, playback, or tempo requests waiting permanently.

Decoded samples are also written asynchronously to `Cache/decoded` in a compact binary PCM format. Its header fingerprints the source size and nanosecond modification time and records channels, sample rate, and frame count. A valid cache bypasses compressed-media decoding on later sessions. The remaining playlist is warmed sequentially in the background after the active track is ready, avoiding competing decoder jobs.

Rapid selections use monotonically increasing load generations. A slow, obsolete decode may populate the cache, but it cannot replace the newer track selected by the user.

The real-time callback never locks or reads this cache. It only sees the selected immutable audio buffer through `ArcSwap`.

## Optional HTDemucs 6s stem mode

Stem mode is disabled by default and never delays ordinary track loading. On an Apple-silicon Mac, enabling it starts the private `sonarcan-mlx-worker` process with the exact `demucs-mlx` environment from `tools/sonarcan-mlx-worker/uv.lock`. Development uses uv-managed Python 3.13.5. Release assembly copies uv's complete standalone CPython distribution and synchronizes the locked production packages into it; uv is not installed or executed on the user's Mac.

The pinned `htdemucs_6s` model is supplied as a release resource and verified against the SHA-256 in its config before MLX loads it. The worker emits bounded newline-delimited JSON for stage changes, segment progress, logs, errors, and completion. Rust supervises and can terminate the child process, treats every event and output path as untrusted, and accepts only the exact vocals, drums, bass, other, guitar, piano contract.

Completed WAV stems are decoded and aligned to the source sample rate and frame count before being committed under `Stems/<track-id>/` as stereo float PCM plus a JSON manifest. The cache key covers the cache format, model revision, track identifier, source size, and nanosecond modification time. Only after all six stems validate does the engine swap an immutable six-buffer set into the callback. Per-stem gain, pan, mute, solo, bypass, and peak values remain atomic and allocation-free in the callback. Bypass selects the original immutable audio buffer without releasing the stem set, enabling immediate original/mix comparisons.

Once the cache is valid, the selected track's six stems can be exported from the
mixer header. WAV export streams the cached float PCM into lossless 32-bit float
WAVE files without loading all stems into memory. MP3 export performs the same
bounded WAV staging one stem at a time, then invokes FFmpeg with the user's MP3
quality preference. Export is unavailable until all six stems validate, writes
into a newly selected directory, and never runs on the audio callback.

The music volume is applied to whichever musical source is active: the original
audio or the six-stem mix. The metronome is added afterwards, then the master
volume is applied to the combined signal. Master, music,
mute/unmute, stem gain/pan, and stem mute/solo changes use a 40 ms callback-side
ramp to avoid clicks and zipper noise. The ramp keeps the real-time path
allocation-free and lock-free.

Each decoded track is analyzed outside the callback with BS.1770 K-weighting,
absolute and relative gating, and an oversampled peak estimate. The persistent
PCM cache stores the resulting integrated loudness and peak values alongside
the source identity. Loudness normalization is enabled by default, targets
−16 LUFS, and is capped so the normalized track does not exceed −1 dBFS. It is
applied non-destructively to the original audio or active stem mix before the
music-volume control; the canonical imported media is never rewritten.

The master control uses 100% as unity gain and ranges from 0% to 200% (+6 dB).
After music, metronome, and master gain are combined, a stereo-linked safety
limiter with a −1 dBFS ceiling and 120 ms release replaces hard clipping. Its
state and frame scratch space are allocated with the stream, and the callback
publishes only a scalar reduction indicator for the UI.

Interactive seeks are coalesced by the UI to at most one in-flight IPC request,
while the position readout continues to follow the pointer immediately. Each
accepted position generation is joined to the last output frame by an 8 ms
callback-side transition. Rapid scrubbing therefore cannot build an IPC backlog
or send a discontinuous sample step to the output device; the transition uses
only buffers allocated when the stream is created.

The bounded `AudioStatus` snapshot exposes the decaying master peak and
independent left/right output peaks for the UI meters. Meter levels are measured
after the limiter and calibrated so its −1 dBFS ceiling is the top of the UI
scale. These scalar values are the only output-level data crossing IPC; raw
audio never leaves the engine.

Python, uv, worker dependencies, and the model revision are pinned in the worker project and `stem_contract.rs`. Updating any of them requires regenerating the lockfile and runtime, changing the cache revision when output compatibility changes, and repeating separation parity and performance tests.

`demucs-mlx 1.4.6` rejects a numeric key found only in the official checkpoint's unused `training_args` metadata. The release model builder strips that one optional metadata field before invoking the package's restricted loader and converter. Constructor data, tensor state, official signature, source checksum, and generated safetensors checksum continue through the upstream validation path. Remove this narrow workaround when the pinned upstream version accepts its official checkpoint unchanged.

When looping is enabled, playback may start before A as a lead-in. Seeking to B
or anywhere after B disables Loop and Training so playback can continue freely
outside the former loop. Activating Loop or Training seeks to A. Once active
playback reaches B, the Rust callback wraps to A. The seek and wrap rules are
enforced by the engine rather than simulated only by the frontend.

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

A ten-millisecond equal-gain boundary crossfade is applied before B. In stem mode, gain, pan, mute, and solo are applied first, then the six stems are summed to stereo and one shared fade envelope is applied to that final mix. Original-audio mode uses the same final-mix path. The crossfade is shortened automatically for very small loops, and playback resumes after the part of the loop head already consumed by that overlap. This avoids replaying the faded-in head at A, removes the second discontinuity that is especially audible on isolated stems, and preserves the level of correlated material.

## Resampling

Linear interpolation currently handles source/output sample-rate differences. This is sufficient for transport validation but will be replaced by the selected production resampler during the DSP benchmark phase.

## Time stretch and pitch shift

The callback owns one preconfigured Signalsmith Stretch processor and all of its input, output, and preroll buffers. When stems are active, their gain/pan/mute/solo mix is computed first; the resulting stereo mix passes through this processor exactly once. Peak values for the six UI meters are reduced inside the current block and published through six atomics only once per callback. The UI sends only bounded control values:

- playback speed from 50% to 200%, independently of pitch;
- pitch transposition from -12 to +12 semitones, independently of speed.
- fine pitch correction in 1-cent increments (`0.01` semitone) for slightly detuned recordings.

Unity speed with zero transposition takes a direct low-overhead path. Other settings stream source frames through the DSP processor. A seek, track change, or loop restart resets and prerolls the processor from the requested source position, avoiding stale buffered audio. Interactive speed and pitch changes retain the streaming state and follow a 40 ms exponential transition, preventing repeated resets and discontinuities while playback continues. Signalsmith restores neutral transposition during transport resets, so the engine deliberately reapplies the selected pitch afterwards. Speed and pitch are persisted in each track's practice state.

Signalsmith's processing window has distinct input and output latency. The
renderer therefore owns two source positions: an internal lookahead position
used only to feed the processor, and the audible output position published to
the UI and used by the metronome. Reset preroll contains the real source audio
between those positions, including loop wrapping and crossfades. Once a live
speed ramp settles, a new preroll is applied at that rate so its
rate-dependent lookahead cannot accumulate as a cursor or metronome offset.
Device buffering still adds the same platform-dependent delay to the complete
mixed stream; it does not alter the relative alignment of music and click.

At 48 kHz, the uncompensated processor placed an impulse 5,760 output frames
(120 ms) late at `1×`, 6,712 frames (about 140 ms) late at `0.75×`, and 8,640
frames (180 ms) late at `0.5×`. The compensated preroll regression measures 0,
11, and 0 frames of error respectively (at most 0.23 ms), and a renderer-level
test verifies that the published position follows the compensated audible
output rather than the internal lookahead.

The UI updates its readout immediately and applies a 65 ms trailing debounce before crossing IPC. Button steps are 5% for speed and one semitone for pitch; Shift-click selects the fine 1% or one-cent step respectively.

## Tempo analysis

Beat This! detects the track's beat and downbeat timestamps in the supervised
analysis worker. The displayed BPM is an indication derived from the median
interval between detected beats; it is not an editable timing source. The old
energy-autocorrelation tempo analyzer and its separate cache have been removed.

## Beat grid and metronome

The waveform grid and metronome use Beat This!'s individual beat timestamps,
not a regular interval synthesized from BPM. Detected downbeats receive the
accented grid line and click. This preserves expressive tempo variation and
prevents a wrong scalar BPM from shifting the grid. The metronome stays silent
before the first detected beat. Its enabled state remains part of track practice
state; volume and timbre are global preferences.

The detailed waveform introduces downbeats at `1.5×` zoom and draws every beat
and chord block once its viewport contains at most 30 seconds. Shorter tracks
show those details while fitted in full. An independent UI magnet can
snap A/B placement to the nearest detected beat. It does not synthesize
subdivisions or alter playback timing. Its enabled state is a global user
preference, not per-track practice state. Preference controls apply and save
automatically when changed, and `M` remains the metronome shortcut.

The metronome is synthesized directly in the CPAL callback. It performs no allocation, locking, or IPC. The user can choose an electronic sine burst, a woodblock made from short modal resonances, or a metallic sound made from inharmonic partials. Each timbre uses a higher pitch and gain for detected downbeats. Beat phase is derived from the detected beat timeline and current source position, which keeps the click aligned after seeks and A/B loop wraps. Playback speed changes the real-time spacing between clicks automatically while preserving alignment with the source waveform.

## Loop trainer

Enabling Training also enables the current A/B loop. If no explicit bounds are
available, Rust uses the complete track as A/B; this rule is enforced by the
audio engine rather than simulated by the WebView. Playback immediately returns
to the configured start rate. The default user profile is 50% start, 100% end,
a 5% step, and one complete loop per step.

The real-time renderer counts training cycles with atomics. One cycle is a
complete A-to-B pass followed by the B-to-A wrap. A lead-in before A is allowed
but never counted as a repetition; a partial pass that starts inside the loop is
also completed once before it can be counted. After the configured number of
cycles, the renderer increases the playback rate by the configured step.
Training stops automatically at the end rate while the A/B loop remains active.
The callback never waits for the UI to schedule an increment, so continuity is
preserved. Enabled state, start/end rates, step, and loops per step are persisted
per track. Global preferences only provide defaults for new/reset track settings.
The Training dialog applies and persists each change immediately; its reset
action restores these user defaults.

## End-of-track behavior

Outside an active A/B loop, the engine supports three explicit modes: restart the current track with a boundary crossfade, signal the frontend to advance to the next preloaded playlist item, or stop. A monotonically increasing end generation prevents polling races when the advance signal is consumed. Active full-track Loop Trainer cycles temporarily take precedence; the selected end behavior resumes after training reaches its target.

## Spectrum worker

A dedicated `sonarcan-spectrum` Rust worker analyzes a 2,048-sample Hann window centered on the current source position. RustFFT produces the transform outside the audio callback. The result is reduced to 64 logarithmic bands from 30 Hz to the lower of 20 kHz or Nyquist and normalized to a bounded display range. Only these visualization magnitudes cross IPC; raw samples remain in Rust.

## Current limitations

- A complete decoded track is held in memory.
- Output-device changes require an engine restart.
- The displayed BPM is an indicative summary; timing deliberately follows the detected beat sequence instead.
- The UI polls lightweight atomic status; a versioned event stream will replace polling later.

These limitations are explicit roadmap items and are not hidden behind simulated controls.
