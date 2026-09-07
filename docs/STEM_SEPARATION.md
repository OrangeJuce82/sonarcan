# Stem separation

SonArcan exposes two optional four-channel separation profiles behind the same
practice mixer:

- **Fast** uses the official HTDemucs four-source model with 25% overlap and no
  random shift;
- **HQ** uses SCNet Large by starrytong with four-way overlap-add.

Apple Silicon executes both profiles with MLX. NVIDIA Windows/Linux and AMD
Linux execute the equivalent graphs with Torch through CUDA or ROCm. Light
editions omit the workers. CPU-only separation is not exposed as a supported
user experience.

## User workflow

1. An empty MIX panel presents Fast and HQ as two explicit buttons. There is no
   preselection and no separation profile in user preferences.
2. The first Full-edition application launch downloads both verified checkpoints.
   Choosing a profile later prepares the platform graph, then separates vocals,
   drums, bass, and other.
3. The first-run screen covers downloads and verification. The mixer progress bar covers model preparation, audio loading,
   inference, writing, validation, and caching. The UI displays a continuously
   updated, smoothed remaining-time estimate.
4. The mixer appears only after all four outputs validate. Later use of the same
   unmodified source/profile loads its project cache.
5. The header switch bypasses the ready mix without unloading it. The header
   Reset action deletes every generated profile for the selected track, resets
   gain/pan/mute/solo, and shows the two model buttons again.

The mixer presents vocals, drums, bass, then other. Each channel provides gain,
pan, a bounded peak meter, mute, solo, a fixed identifying color, and an
editable label. Master gain, speed, pitch, loops, Loop Trainer, and metronome
remain global and are applied after stem summing.

## Model cache and integrity

Checkpoints are not embedded in the installer. The first-run model installation downloads them before opening the workspace. Tauri's
application-data directory contains `models/stem-separation/`, with
`htdemucs-v4/` and `scnet-large-starrytong-v1.0.9/` below it. This resolves to:

- macOS: `~/Library/Application Support/music.sonarcan.desktop/`;
- Windows: `%APPDATA%\music.sonarcan.desktop\`;
- Linux: `~/.local/share/music.sonarcan.desktop/`.

Downloads use an adjacent temporary file, exact byte length and full SHA-256,
then atomic rename. Cached checkpoints are checked before every load. Symlinks,
partial files, and modified files are rejected. Torch checkpoint loading uses
`weights_only=True` and a bounded allowlist; no unrestricted pickle fallback is
used.

Fast uses the official Demucs artifact:

- URL: `https://dl.fbaipublicfiles.com/demucs/hybrid_transformer/955717e8-8726e21a.th`;
- exact size: 84,141,911 bytes;
- SHA-256: `8726e21a993978c7ba086d3872e7608d7d5bfca646ca4aca459ffda844faa8b4`.

On Apple Silicon, the verified Torch artifact is converted once to the safe MLX
safetensors cache. Other supported systems load the same verified weights in
Torch. No unofficial GitHub mirror is used because the upstream weight
redistribution terms have not been established.

HQ uses SCNet Large by starrytong from the v1.0.9 GitHub release:

- URL: `https://github.com/ZFTurbo/Music-Source-Separation-Training/releases/download/v1.0.9/SCNet-large_starrytong_fixed.ckpt`;
- exact size: 168,852,258 bytes;
- SHA-256: `65900dfa07d6b6e5d784c0f143920200a4bd281d6e78a806c549d0b912d5885e`;
- inference source: `openmirlab/scnet-infer` revision
  `a5437e37c8b942baf74529f35a719aa70dfa9bdc`.

SCNet's author publicly confirmed that the SCNet and SCNet-large pretrained
weights use the repository's MIT license and may be redistributed, including
converted weights, with attribution:
`https://github.com/starrytong/SCNet/issues/35`. HTDemucs weight terms remain a
separate release-review item; SonArcan downloads those weights from the official
publisher rather than redistributing them.

## Runtime and project caches

All release workers share one target-native Python 3.13.5 runtime. Apple
Silicon uses MLX; NVIDIA releases resolve pinned CUDA builds; AMD Linux resolves
pinned ROCm builds. uv runs only on development/build machines.

Project results are independent per profile:
`Stems/<track-id>/<fast|hq>/`. Each cache manifest fingerprints the source size,
nanosecond modification time, cache format, and exact profile revision. Rust
owns cache validation, decoded buffers, real-time mixing, and deletion. The
WebView receives bounded progress/control snapshots only; raw audio never
crosses JSON IPC. The CPAL callback performs no model work, I/O, allocation,
locking, or IPC.

## Performance qualification

On 7 September 2026, SCNet Large was measured on a 16 GB MacBook Air M3 with a
15-second synthetic 44.1 kHz stereo file. MLX batch 1 took 56.95 s for inference
and 68.22 s end to end. Batch 2 took 30.30 s for inference and 41.69 s end to
end, while producing all four valid stems. Batch 4 exhausted Metal memory on
GitHub's `macos-15` Apple Silicon release runner. The shared SCNet plan is
therefore batch 2 on MLX, CUDA, and ROCm; MLX also materializes overlap-add and
clears unused allocations between forwards. Representative full-song
benchmarks are still required on every supported accelerator.

The HTDemucs Fast protocol has been exercised end-to-end with both its MLX and
Torch paths, including safe loading and four output files. Representative
full-song cold/warm benchmarks are still required on every supported
accelerator. These figures are regression baselines, not universal speed
promises.
