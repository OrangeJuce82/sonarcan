# SonArcan 0.1.7 beta 4

SonArcan now builds, tests, and releases exclusively for macOS on Apple
Silicon. Windows, Linux, Intel macOS, CUDA, ROCm, portable Torch, and CPU
runtime build profiles have been removed.

## Requirements

- Apple M1 or newer;
- macOS 14 or later;
- 8 GB RAM minimum, 16 GB recommended;
- working Apple GPU support through MLX and MPS.

There is no CPU inference fallback. SonArcan exercises the production MPS chord
and rhythm graphs and the MLX stem worker before enabling analysis. If either
probe fails, Beat, Chords, BPM, Mix, and the analysis metronome stay disabled
for that session; playback and project data remain available.

## Download

Download the Apple Silicon DMG and `SHA256SUMS.txt` from the release assets,
then verify the installer from Terminal:

```bash
cd ~/Downloads
shasum -a 256 --check SHA256SUMS.txt
```

The GitHub build is ad-hoc signed, not notarized or identified by Apple. On the
first launch, macOS may require **System Settings → Privacy & Security → Open
Anyway**.

## Included behavior

- Stem generation now waits for the selected audio to finish loading before it
  starts, so a completed separation activates its cached mix immediately.
- The Import Center now expands direct tracks and playlists through bounded
  metadata probes, preserves real titles and artists, and blocks DRM-protected
  or otherwise unavailable sources before they can be queued.
- Music discovery links and the yt-dlp credit use a fixed Rust allowlist, while
  bundled provider logos avoid third-party image requests.
- Project mutations share one write coordinator, preventing background import
  completion from restoring stale project state. Tracks whose packaged media is
  already missing can still be removed safely.
- A dedicated About view presents product identity, open-source tooling,
  licenses, and acknowledgements. The bounded diagnostic console can copy only
  the entries selected by its current severity and origin filters.
- LV-Chordia and Beat This! stay resident after startup qualification; only the
  selected chord vocabulary is decoded initially, and additional vocabularies
  are calculated on demand without repeating rhythm analysis.
- YouTube and SoundCloud text searches reuse a bounded pair of resident yt-dlp
  workers and request only the metadata used by SonArcan.
- Local beat, downbeat, BPM, and timed-chord analysis through MPS.
- Fast HTDemucs and HQ SCNet Large four-stem separation through MLX.
- Waveform editing for markers, chords, and synchronized lyrics.
- Independent tempo and pitch controls, A/B loops, and progressive training.
- Portable `.sac` projects with source media separated from generated caches.

Every launch verifies the pinned Beat This!, HTDemucs, and SCNet Large
checkpoints before probing the accelerator. Missing or invalid cache entries
are downloaded again, size- and SHA-256-verified, then published atomically.
