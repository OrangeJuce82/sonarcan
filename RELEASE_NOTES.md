# SonArcan 0.1.6

SonArcan is now a macOS Apple Silicon application. The release contains one
native ExecuTorch runtime with the MLX delegate; Python and PyTorch remain
development-only export tools and are never included in the application.

## Requirements

- Mac with an M1 chip or newer;
- macOS 14 or later;
- integrated Apple GPU available to MLX.

There is no CPU inference fallback and no reduced mode. At launch SonArcan
verifies the native worker and MLX delegate, then installs the SHA-256-pinned
model packs. A failed check keeps the workspace and every tool inaccessible.

## First-run model installation

The application downloads two packs from the `native-models-v1` release:

- chord and rhythm: 204,178,291 bytes;
- HTDemucs and SCNet stem separation: 341,114,095 bytes.

The installer rejects unexpected sizes, hashes, paths, symlinks, and partial
files before publishing each pack atomically. Development checkpoints are never
accepted as release resources.

## Download

Download the Apple Silicon DMG and `SHA256SUMS.txt` from the release assets.
Verify it from Terminal before opening the image:

```bash
cd ~/Downloads
shasum -a 256 --check SHA256SUMS.txt
```

The application bundle must remain below 512 MiB. Model packs stay outside it
and are downloaded once during first-run preparation.

## Other improvements

- Timeline lanes, waveform, overview, time scale, playback slider, help,
  transport, loop, and metronome controls share one horizontal axis.
- Markers, chords, and synchronized lyrics can be created, renamed, resized,
  aligned with Shift, or removed from their contextual menu.
- SoundCloud, Bandcamp, Mixcloud, and YouTube imports retain provider metadata;
  chapters become track markers when available.
- HTDemucs remains the fast four-stem profile. SCNet Large is intentionally the
  slower HQ profile and retains its validated audio behavior.
- Existing analysis caches and `.sac` project data remain preserved if startup
  rejects the machine or model installation fails.
