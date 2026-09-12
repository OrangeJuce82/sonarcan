# SonArcan 0.1.5

SonArcan now ships a native-only application contract for macOS Apple Silicon
and Linux x86_64/arm64. Python and PyTorch remain development export tools and
are never included in an application package.

The waveform gains aligned marker, chord, and synchronized-lyrics lanes. Their
blocks can be created, renamed, resized, aligned across lanes with Shift, or
removed from an explicit right-click menu. Chord editing reuses the grid's full
validated option selector and Shift can replace every matching chord.

Imports now distinguish YouTube and SoundCloud searches and recognize direct
SoundCloud, Bandcamp, and Mixcloud links. Provider chapters become navigable
track markers when available. Marker navigation joins the `N` shortcut cycle,
and older project practice values are normalized safely when reopened.

## First-run model installation

Backend-specific `.pte` programs are installed outside the application bundle.
The small application package contains only the selectively linked ExecuTorch
worker. Development checkpoints (`.ckpt`, `.th`, `.sdict`, or `safetensors`)
are rejected by the release verifier.

Stem separation offers HTDemucs Fast and SCNet Large HQ four-stem production
profiles, both producing vocals, drums, bass, and other. SCNet Large processes
two overlapping chunks per forward pass on every accelerator. MLX also releases intermediate GPU
allocations between passes to avoid unified-memory exhaustion on
memory-constrained Macs. SCNet is intentionally the slow HQ profile: the
qualified 11-second Apple Silicon workload took about 329.5 seconds warm and
roughly 383 seconds on its first compiled pass. HTDemucs remains the faster
alternative when turnaround time matters more than the HQ profile.

## Which file should I download?

| Release name | Computer | Beat, Chords, Mix | Included compute runtime |
| --- | --- | --- | --- |
| SonArcan | Apple-silicon Mac (M1 or newer) | Native model programs only | MLX |
| SonArcan Linux x86_64 | Debian-compatible Linux x64 with NVIDIA GPU | Native model programs only | CUDA |
| SonArcan Linux arm64 | Debian-compatible Linux arm64 with NVIDIA GPU | Native model programs only | CUDA |

Linux requires a supported NVIDIA GPU, its proprietary driver, and a compatible
CUDA runtime. There is no CPU mode and no AMD/Intel GPU path. On incompatible
hardware SonArcan displays a blocking message at startup; the workspace and all
tools remain inaccessible.

Releases never run Beat, Chords, or Mix silently on the CPU. At every launch,
SonArcan verifies the required GPU, driver, and native delegate before it opens
the workspace. It then verifies the SHA-256-pinned model packs. If the device, driver,
runtime, delegate, or models are unavailable or invalid, a blocking error is
shown and the workspace does not open. Complete model-graph execution is part
of release qualification on representative GPU hardware.

## Linux download format

The release target is a single lightweight DEB using a selective native runtime.
Multipart packages and bundled Python/PyTorch environments are rejected by the
release checks.

### Linux

Open a terminal in the download directory, then run:

```bash
cd ~/Downloads
version=v0.1.5
sha256sum --check SHA256SUMS-Linux-x86_64-DEB.txt
sudo apt install "./SonArcan-Linux-x86_64-${version}.deb"
```

Do not install the reconstructed package if `sha256sum` reports a missing file
or a checksum failure.

Linux releases are currently distributed only as DEB packages.

## Other improvements

- Timeline lanes, waveform, overview, time scale, playback slider, help,
  transport, loop, and metronome controls share one horizontal axis.
- Shift can be pressed before or during a timeline-edge drag to snap to another
  category without changing the independent A/B loop magnet preference.
- Import provider selection is highlighted, source logos remain beside
  relevance information, and long failed-import titles wrap instead of being
  truncated.
- The package contains one selective native inference runtime; model weights are
  installed and verified on first launch.
- Release and CI jobs use one application contract without duplicate aliases,
  manifests, runtime builders, or verification branches.
- Existing analysis caches and `.sac` project data remain preserved when startup
  rejects incompatible hardware.

See the README for detailed minimum configurations and installation guidance.
