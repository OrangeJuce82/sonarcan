# Stem separation

SonArcan provides an optional six-channel practice mixer backed by one
`htdemucs_6s` model. Apple Silicon uses Apple MLX; Windows and Linux use
portable Torch inference. The feature is deliberately opt-in because
source separation consumes substantially more compute and disk space than
normal playback.

## User workflow

1. Select a track and enable six-stem mode on a supported desktop.
2. SonArcan starts the target's pinned, bundled Python worker and shared model.
3. Structured progress is reported while HTDemucs separates vocals, drums, bass, other, guitar, and piano.
4. The vertical mixer becomes available when all six cached buffers are complete.
5. Later activations for the same unmodified source load the project cache instead of running inference again.

The mixer presents vocals, drums, bass, guitar, piano, then other. Each channel provides a vertical gain fader from 0% to 200%, pan, a bounded LED peak meter, mute, solo, a fixed identifying color, and a user-editable lower label. Double-click resets gain to 0 dB and pan to center. Multiple channels may be soloed. Names, pan, gain, mute, and solo are saved in the track practice state. Master gain, playback speed, pitch, loops, Loop Trainer, and the metronome remain global and are applied after stem summing.

The header switch is a real-time bypass after generation. Switching it off restores the original track without dropping the six decoded buffers and disables all controls inside the mixer; switching it back on therefore does not start Python, reload the model, or reread the cache. Cancelling while separation is still running remains destructive and terminates the worker.

## Implementation constraints

- Rust selects and supervises the private MLX or Torch worker; caching and real-time mixing remain Rust-owned.
- TypeScript receives status and control metadata only.
- The CPAL callback performs no inference, I/O, allocation, locking, or IPC.
- A stem set is activated only after all six outputs and the manifest have been committed.
- Cache artifacts are generated data and may be removed safely; SonArcan will regenerate them on demand.
- Validation and cache-write durations are logged separately with the model name. Matching stereo stems avoid a redundant alignment copy, and cache PCM is encoded and written in bounded blocks.

The MLX worker pins Python 3.13.5; the portable worker pins Python 3.12.12 and
CPU Torch. uv is used only on development and build machines. The model config
records and validates the official source identity and generated Safetensors
SHA-256. Torch reconstructs the upstream module from that same file and rejects
missing, extra, or shape-mismatched tensors.

Portable inference follows Demucs' documented fast CPU profile: the shift trick
is disabled on CPU, retained on accelerators, and window overlap is reduced from
25% to 10%. Inference runs under Torch inference mode and uses deterministic
shift selection. The worker logs model-load, decode, inference, and output-write
durations separately. A measured model load takes only a small fraction of a
second, so releases do not duplicate the shared MLX-layout tensors for Intel.

## September 2026 performance measurements

The optimization benchmark uses 30 seconds of generated 44.1 kHz stereo float
audio on a 16 GB MacBook Air M3. Each comparison uses warmed runtime and Metal
caches. Wall-clock results include model loading, decode, inference, and six WAV
writes.

| Backend | Previous profile | Optimized profile | Change |
| --- | ---: | ---: | ---: |
| Portable Torch, CPU | 9.04 s | 8.47 s | -6.3% wall time; -11.1% CPU time |
| Portable Torch, MPS | 6.54 s | 5.74 s | -12.2% wall time |
| Native MLX, batch 2 | 3.91 s | 3.32 s | -15.1% wall time |

The MLX batch sweep was repeated after selecting 10% overlap. Batch 2 completed
in 3.32 seconds with a 2.65 GiB MLX peak; batch 4 took 4.31 seconds with a
3.90 GiB peak; batch 8 took 5.36 seconds with a 3.90 GiB peak. Batch 2 therefore
remains the release default. Portable Torch model loading measured 0.14 seconds,
so a second platform-specific tensor artifact would target the wrong bottleneck.
Every desktop release job now runs a 15-second end-to-end separation and prints
its real-time factor, providing target-native Linux, Windows, and Apple
Silicon measurements for release qualification.

## Remaining validation

The automated suite validates both workers, the strict shared-model
reconstruction, six-buffer cache, real-time engine, frontend, and native builds
on all three release targets. Release qualification additionally runs model
self-tests in each bundled runtime. MLX performance qualification remains
specific to Apple Silicon; portable CPU timings are recorded separately.
