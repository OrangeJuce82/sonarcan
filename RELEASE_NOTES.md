# SonArcan 0.1.1-beta.4

SonArcan now uses one application contract and four target-specific packages,
without a duplicate build pipeline. Qualified GPU sessions expose local Beat,
Chords, and Mix analysis; the same package
starts in safe simplified mode when its accelerator probe does not succeed.

The waveform gains aligned marker, chord, and synchronized-lyrics lanes. Their
blocks can be created, renamed, resized, aligned across lanes with Shift, or
removed from an explicit right-click menu. Chord editing reuses the grid's full
validated option selector and Shift can replace every matching chord.

Imports now distinguish YouTube and SoundCloud searches and recognize direct
SoundCloud, Bandcamp, and Mixcloud links. Provider chapters become navigable
track markers when available. Marker navigation joins the `N` shortcut cycle,
and older project practice values are normalized safely when reopened.

## First-run model installation

On the first launch with a qualified GPU backend, SonArcan downloads SCNet-large, HTDemucs, and
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
| SonArcan NVIDIA GPU | Windows x64 or Linux x64 with a compatible NVIDIA GPU | Yes, after the startup probe succeeds | PyTorch CUDA 12.6 |
| SonArcan AMD GPU | Linux x64 with a ROCm 7.2-compatible AMD GPU | Yes, after the startup probe succeeds | PyTorch ROCm 7.2 |

There is no AMD GPU edition for Windows in this beta. AMD's Windows support is
currently limited to selected recent GPUs and requires a separate Python 3.12
runtime, which has not yet completed SonArcan's release qualification. The
Windows NVIDIA package starts in simplified mode on AMD-only or Intel Windows
computers. Intel GPUs are not qualified yet.

GPU releases never run Beat, Chords, or Mix silently on the CPU. At every
application launch, SonArcan exercises the actual production model graphs on the detected
accelerator. If the driver, device, runtime, model, memory, or inference result
is incompatible, SonArcan enters safe degraded mode for that session. Beat,
Chords, Mix, BPM, the analysis metronome, and the piano/guitar/ukulele chord
views are hidden; playback, time navigation, lyrics, spectrum, and stereo meters
remain available. The explanation is shown once per user profile.

## GPU download format

CUDA and ROCm runtimes are too large for GitHub's 2 GiB limit per release file.
Each GPU package is therefore split into numbered `part-000`, `part-001`, …
files, accompanied by a platform/backend-specific `SHA256SUMS` file. Download
every part for one backend, plus its matching `SHA256SUMS` file, into the same
directory. Linux GPU releases are multipart DEBs, Windows NVIDIA is a multipart
Zip64 archive, and macOS remains a conventional single-file download.

### Linux NVIDIA or AMD

Open a terminal in the download directory, set `backend` to `NVIDIA` or `AMD`,
then run:

```bash
cd ~/Downloads
version=v0.1.1-beta.4
backend=NVIDIA # Replace with AMD for the ROCm release.
sha256sum --check "SHA256SUMS-Linux-${backend}-GPU-DEB.txt"
cat "SonArcan-Linux-x86_64-${backend}-GPU-${version}.deb".part-* > "SonArcan-${backend}-GPU.deb"
sudo apt install "./SonArcan-${backend}-GPU.deb"
```

Do not install the reconstructed package if `sha256sum` reports a missing file
or a checksum failure.

Linux releases are currently distributed only as DEB packages.

### Windows NVIDIA

Open PowerShell in the download directory and run:

```powershell
$ErrorActionPreference = 'Stop'
Set-Location "$HOME\Downloads"
$version = 'v0.1.1-beta.4'
$checksumFile = 'SHA256SUMS-Windows-NVIDIA-GPU.txt'
foreach ($line in Get-Content -LiteralPath $checksumFile) {
  $expected, $file = $line -split '\s+', 2
  $actual = (Get-FileHash -LiteralPath $file -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($actual -ne $expected.ToLowerInvariant()) { throw "Checksum mismatch: $file" }
}
$parts = @(Get-ChildItem "SonArcan-Windows-x86_64-NVIDIA-GPU-$version.zip.part-*" | Sort-Object Name)
if ($parts.Count -eq 0) { throw 'No archive parts found' }
$archive = "SonArcan-NVIDIA-GPU-$version.zip"
$output = [IO.File]::Create($archive)
try {
  foreach ($part in $parts) {
    $input = $part.OpenRead()
    try { $input.CopyTo($output) } finally { $input.Dispose() }
  }
} finally { $output.Dispose() }
Expand-Archive -LiteralPath $archive -DestinationPath "SonArcan-NVIDIA-GPU-$version"
& ".\SonArcan-NVIDIA-GPU-$version\SonArcan NVIDIA GPU.exe"
```

PowerShell stops before reconstruction if a part is missing or altered.

## Other improvements

- Timeline lanes, waveform, overview, time scale, playback slider, help,
  transport, loop, and metronome controls share one horizontal axis.
- Shift can be pressed before or during a timeline-edge drag to snap to another
  category without changing the independent A/B loop magnet preference.
- Import provider selection is highlighted, source logos remain beside
  relevance information, and long failed-import titles wrap instead of being
  truncated.
- Every Windows and Linux hardware package carries the shared runtime needed for
  imports but never enables heavy analysis unless its startup probe succeeds.
- Release and CI jobs use one application contract without duplicate aliases,
  manifests, runtime builders, or verification branches.
- Existing analysis caches and `.sac` project data remain preserved when the
  application runs in simplified mode.

See the README for detailed minimum configurations and installation guidance.
