# Stem separation

SonArcan provides an optional four-channel practice mixer backed by HTDemucs v4 standard. The feature is deliberately opt-in because source separation consumes substantially more compute and disk space than normal playback.

## User workflow

1. Select a track and enable four-stem mode.
2. On first use, SonArcan downloads the 84 MB model locally.
3. Progress is reported while HTDemucs separates vocals, drums, bass, and other.
4. The vertical mixer becomes available when all four cached buffers are complete.
5. Later activations for the same unmodified source load the project cache instead of running inference again.

Each channel provides gain from 0% to 200%, mute, and solo. Multiple channels may be soloed. Master gain, playback speed, pitch, loops, Loop Trainer, and the metronome remain global and are applied after stem summing.

## Implementation constraints

- Inference, caching, and mixing are Rust-only.
- TypeScript receives status and control metadata only.
- The CPAL callback performs no inference, I/O, allocation, locking, or IPC.
- A stem set is activated only after all four outputs and the manifest have been committed.
- Cache artifacts are generated data and may be removed safely; SonArcan will regenerate them on demand.

The integration pins `demucs-core` to a reviewed Git revision. Its Apache-2.0 implementation executes the MIT-licensed HTDemucs architecture and weights through Burn. The model is loaded from a local application cache and its internal tensor signature is validated during construction.

## Remaining validation

The automated suite validates the existing real-time engine and both frontend and Rust compilation paths without downloading model weights. Release qualification must additionally run a real model smoke test on Apple Silicon and representative Linux/Windows GPUs, record cold/warm processing time, verify stem reconstruction and alignment, and monitor peak memory use.
