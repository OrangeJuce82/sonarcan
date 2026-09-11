# SonArcan 0.1.3

SonArcan uses one complete application contract and two target-specific
packages. The workspace opens only when the complete Beat, Chords, and Mix
pipeline passes its accelerator probe.

The waveform gains aligned marker, chord, and synchronized-lyrics lanes. Their
blocks can be created, renamed, resized, aligned across lanes with Shift, or
removed from an explicit right-click menu. Chord editing reuses the grid's full
validated option selector and Shift can replace every matching chord.

Imports now distinguish YouTube and SoundCloud searches and recognize direct
SoundCloud, Bandcamp, and Mixcloud links. Provider chapters become navigable
track markers when available. Marker navigation joins the `N` shortcut cycle,
and older project practice values are normalized safely when reopened.

## First-run model installation

On the first launch with a qualified accelerator, SonArcan downloads SCNet-large, HTDemucs, and
Beat This! one at a time from their pinned upstream locations. The welcome
screen identifies each model and shows both per-model and overall progress.
Every checkpoint is checked against its expected byte length and SHA-256 digest
before it is moved atomically into the application cache. An interrupted or
invalid download is never used and can be retried from the same screen.

LV-Chordia remains bundled with the shared analysis runtime and its five checkpoints are
verified before the workspace opens. Subsequent launches reuse all verified
cached models, so the network setup happens only once unless the cache is
removed or a future release changes a checkpoint.

Stem separation now offers two four-stem profiles: SCNet-large for the primary
high-quality path and HTDemucs as the alternate profile. Both produce vocals,
drums, bass, and other. SCNet-large processes two overlapping chunks per
forward pass on every accelerator. MLX also releases intermediate GPU
allocations between passes to avoid unified-memory exhaustion on
memory-constrained Macs.

## Which file should I download?

| Release name | Computer | Beat, Chords, Mix | Included compute runtime |
| --- | --- | --- | --- |
| SonArcan | Apple-silicon Mac (M1 or newer) | Yes, after the startup probe succeeds | Apple MLX and MPS |
| SonArcan NVIDIA GPU | Debian-compatible Linux x64 with a compatible NVIDIA GPU | Yes, after the startup probe succeeds | CUDA |

Linux distribution is limited to Debian-compatible x64 systems; Intel GPUs are
not qualified yet.

Releases never run Beat, Chords, or Mix silently on the CPU. At every
application launch, SonArcan exercises the actual production model graphs on the detected
accelerator. If the driver, device, runtime, model, memory, or inference result
is incompatible, SonArcan displays a blocking error and does not open the
workspace.

## GPU download format

The release target is a single lightweight DEB using a selective native runtime.
Multipart packages and bundled Python/PyTorch environments are rejected by the
release checks.

### Linux NVIDIA

Open a terminal in the download directory, then run:

```bash
cd ~/Downloads
version=v0.1.3
sha256sum --check SHA256SUMS-Linux-NVIDIA-GPU-DEB.txt
sudo apt install "./SonArcan-Linux-x86_64-NVIDIA-GPU-${version}.deb"
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
