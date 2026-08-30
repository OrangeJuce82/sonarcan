# Stem separation

SonArcan provides an optional six-channel practice mixer backed by `htdemucs_6s` and Apple MLX. The feature is deliberately opt-in because source separation consumes substantially more compute and disk space than normal playback.

## User workflow

1. Select a track and enable six-stem mode on an Apple-silicon Mac.
2. SonArcan starts its pinned, bundled Python/MLX worker and model.
3. Structured progress is reported while HTDemucs separates vocals, drums, bass, other, guitar, and piano.
4. The vertical mixer becomes available when all six cached buffers are complete.
5. Later activations for the same unmodified source load the project cache instead of running inference again.

The mixer presents vocals, drums, bass, guitar, piano, then other. Each channel provides a vertical gain fader from 0% to 200%, pan, a bounded LED peak meter, mute, solo, a fixed identifying color, and a user-editable lower label. Double-click resets gain to 0 dB and pan to center. Multiple channels may be soloed. Names, pan, gain, mute, and solo are saved in the track practice state. Master gain, playback speed, pitch, loops, Loop Trainer, and the metronome remain global and are applied after stem summing.

The header switch is a real-time bypass after generation. Switching it off restores the original track without dropping the six decoded buffers and disables all controls inside the mixer; switching it back on therefore does not start Python, reload the model, or reread the cache. Cancelling while separation is still running remains destructive and terminates the worker.

## Implementation constraints

- Rust supervises the private MLX worker; caching and real-time mixing remain Rust-owned.
- TypeScript receives status and control metadata only.
- The CPAL callback performs no inference, I/O, allocation, locking, or IPC.
- A stem set is activated only after all six outputs and the manifest have been committed.
- Cache artifacts are generated data and may be removed safely; SonArcan will regenerate them on demand.
- Validation and cache-write durations are logged separately with the model name. Matching stereo stems avoid a redundant alignment copy, and cache PCM is encoded and written in bounded blocks.

The worker pins Python 3.13.5 and every package in `uv.lock`. uv is used only on development and build machines; a relocatable runtime and safe MLX model are included as signed application resources. The model config records and validates the official source identity and generated safetensors SHA-256.

## Remaining validation

The automated suite validates the worker protocol, six-buffer cache, real-time engine, frontend, and Rust compilation paths without requiring Metal. Release qualification additionally runs a real model smoke test on Apple Silicon, records cold/warm processing time, verifies stem reconstruction and alignment, and monitors peak memory use. Intel macOS, Linux, and Windows are not MLX stem targets.
