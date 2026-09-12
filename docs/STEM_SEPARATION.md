# Stem separation

SonArcan exposes two optional four-channel separation profiles behind the same
practice mixer:

- **Fast** uses HTDemucs with 25% overlap and no random shift;
- **HQ** uses SCNet Large by starrytong with four-way overlap-add.

Apple Silicon executes both profiles on the Apple GPU. NVIDIA Debian packages
execute the equivalent graphs through CUDA. A failed qualification blocks the
workspace; CPU-only separation is not a supported user experience.

## User workflow

1. An empty MIX panel presents Fast and HQ as two explicit buttons. There is no
   preselection and no separation profile in user preferences.
2. The first launch with a qualified GPU backend downloads the verified native
   model pack. Choosing a profile later prepares its graph, then separates vocals,
   drums, bass, and other.
3. The first-run screen covers downloads and verification. The mixer progress bar covers model preparation, audio loading,
   inference, writing, validation, and caching. The UI displays a continuously
   updated, smoothed remaining-time estimate.
4. The mixer appears only after all four outputs validate. Later use of the same
   unmodified source/profile loads its project cache.
5. The header switch bypasses the ready mix without unloading it. The header
   Reset action deletes every generated profile for the selected track, resets
   gain/pan/mute/solo, and shows both model buttons again.

The mixer presents vocals, drums, bass, then other. Each channel provides gain,
pan, a bounded peak meter, mute, solo, a fixed identifying color, and an
editable label. Master gain, speed, pitch, loops, Loop Trainer, and metronome
remain global and are applied after stem summing.

## Model cache and integrity

Checkpoints are not embedded in the installer. The first-run model installation
downloads a backend-specific `.pte` pack before opening the workspace. Tauri's
application-data directory stores it under `models/executorch/stem-separation/`.
This resolves below:

- macOS: `~/Library/Application Support/music.sonarcan.desktop/`;
- Debian: `~/.local/share/music.sonarcan.desktop/`.

Downloads use an adjacent temporary file, exact byte length and full SHA-256,
then bounded extraction and atomic rename. Every contained file has its own
SHA-256 and is checked before the pack is published. Symlinks, partial files,
path traversal, modified files, and packs above 512 MiB are rejected.

The development exporters use the pinned HTDemucs and SCNet checkpoints only to
produce and compare backend-specific `.pte` programs. SCNet Large by starrytong
comes from the v1.0.9 GitHub release:

- URL: `https://github.com/ZFTurbo/Music-Source-Separation-Training/releases/download/v1.0.9/SCNet-large_starrytong_fixed.ckpt`;
- exact size: 168,852,258 bytes;
- SHA-256: `65900dfa07d6b6e5d784c0f143920200a4bd281d6e78a806c549d0b912d5885e`;
- inference source: `openmirlab/scnet-infer` revision
  `a5437e37c8b942baf74529f35a719aa70dfa9bdc`.

SCNet's author publicly confirmed that the SCNet and SCNet-large pretrained
weights use the repository's MIT license and may be redistributed, including
converted weights, with attribution:
`https://github.com/starrytong/SCNet/issues/35`. The native model pack preserves
the applicable MIT notices and provenance for both model families.

## Runtime and project caches

The release target is a selective native runtime with no bundled Python
interpreter, PyTorch installation, or `site-packages`. Export tooling remains a
build-time dependency only.

Project results are independent per profile under
`Stems/<track-id>/<fast|hq>/`. Each cache manifest fingerprints the source size,
nanosecond modification time, cache format, and exact profile revision. Rust
owns cache validation, decoded buffers, real-time mixing, and deletion. The
WebView receives bounded progress/control snapshots only; raw audio never
crosses JSON IPC. The CPAL callback performs no model work, I/O, allocation,
locking, or IPC.

## Performance qualification

On 12 September 2026, the final arm64 ExecuTorch MLX worker passed the complete
native 485,100-sample SCNet chunk. Output probes differed from the reference by
less than `8e-7`, peak amplitude by less than `3e-7`, and aggregate energy by
0.05%. The first compiled run took approximately 383 seconds; a warm validation
took 329.5 seconds and reached 2,069,200,896 bytes maximum RSS in the test
harness. SCNet is intentionally the slower HQ path. These figures are honest
regression baselines, not universal speed promises, and no lower-quality model
or CPU fallback hides the cost. The HTDemucs native path passed the same graph,
split, STFT/ISTFT, and output-probe gates in 7.01 seconds on its bounded sample,
with 0.35% aggregate energy drift from the reference.
