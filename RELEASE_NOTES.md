# SonArcan 0.1.1-beta.1

This beta reduces WebView rendering work during playback and exposes the raw
beat post-processing status needed to keep rhythm presentation explicit. It
also retains the guided, verified first-run model setup and four-stem profiles
introduced in the previous beta.

Windows YouTube searches and imports now keep their `yt-dlp` process hidden
instead of flashing a terminal window. SonArcan Light also restores the global
A/B loop shortcuts and makes synchronized Lyrics navigation and loop snapping
available without installing any analysis model.

## First-run model installation

On the first Full-edition launch, SonArcan downloads SCNet-large, HTDemucs, and
Beat This! one at a time from their pinned upstream locations. The welcome
screen identifies each model and shows both per-model and overall progress.
Every checkpoint is checked against its expected byte length and SHA-256 digest
before it is moved atomically into the application cache. An interrupted or
invalid download is never used and can be retried from the same screen.

LV-Chordia remains bundled with the Full runtime and its five checkpoints are
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
| SonArcan | Apple-silicon Mac (M1 or newer) | Yes | Apple MLX and MPS |
| SonArcan NVIDIA GPU | Windows x64 or Linux x64 with a compatible NVIDIA GPU | Yes, after the startup probe succeeds | PyTorch CUDA 12.6 |
| SonArcan AMD GPU | Linux x64 with a ROCm 7.2-compatible AMD GPU | Yes, after the startup probe succeeds | PyTorch ROCm 7.2 |
| SonArcan Light | Apple-silicon Mac, Intel Mac, Windows x64, or Linux x64 | No | No ML runtime or models |

There is no AMD GPU edition for Windows in this beta. AMD's Windows support is
currently limited to selected recent GPUs and requires a separate Python 3.12
runtime, which has not yet completed SonArcan's release qualification. Use
SonArcan Light on an AMD-only Windows computer. Intel GPUs are not qualified yet.

Full GPU releases never run Beat, Chords, or Mix silently on the CPU. At every
application launch, SonArcan exercises the actual production model graphs on the detected
accelerator. If the driver, device, runtime, model, memory, or inference result
is incompatible, SonArcan enters safe degraded mode for that session. Beat,
Chords, Mix, BPM, the analysis metronome, and the piano/guitar/ukulele chord
views are hidden; playback, time navigation, lyrics, spectrum, and stereo meters
remain available. The explanation is shown once per user profile.

Light is the smallest and safest download for older hardware. It physically
excludes Torch, MLX, the analysis models, and the chord-instrument frontend
assets rather than merely hiding them.

## GPU download format

CUDA and ROCm runtimes are too large for GitHub's 2 GiB limit per release file.
Each GPU package is therefore split into numbered `part-000`, `part-001`, …
files, accompanied by a platform/backend-specific `SHA256SUMS` file. Download
every part for one edition, plus its matching `SHA256SUMS` file, into the same
directory. Linux Light is published as DEB and AppImage; macOS and Windows
Light remain conventional single-file downloads.

### Linux NVIDIA or AMD

Open a terminal in the download directory, set `backend` to `NVIDIA` or `AMD`,
then run:

```bash
cd ~/Downloads
version=v0.1.1-beta.1
backend=NVIDIA # Replace with AMD for the ROCm release.
sha256sum --check "SHA256SUMS-Linux-${backend}-GPU-DEB.txt"
cat "SonArcan-Linux-x86_64-${backend}-GPU-${version}.deb".part-* > "SonArcan-${backend}-GPU.deb"
sudo apt install "./SonArcan-${backend}-GPU.deb"
```

Do not install the reconstructed package if `sha256sum` reports a missing file
or a checksum failure.

On distributions without DEB support, use the Light AppImage: make it executable
with `chmod +x ./<downloaded-file>.AppImage`, then launch it directly. GPU
editions are currently distributed only as DEB packages.

### Windows NVIDIA

Open PowerShell in the download directory and run:

```powershell
$ErrorActionPreference = 'Stop'
Set-Location "$HOME\Downloads"
$version = 'v0.1.1-beta.1'
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

- Playback status remains sampled at 20 Hz, while expensive Svelte position
  updates are bounded to 10 Hz and still react immediately at chord, lyric, and
  metronome boundaries.
- The detailed and overview playheads now move on isolated composited
  transforms instead of layout-affecting `left` updates. The seek control and
  position readout keep the full status cadence without invalidating the main
  component tree.
- Beat post-processing exposes its raw status so the selected presentation mode
  remains distinguishable from the detected source timeline.
- Synchronized lyrics are displayed directly on the waveform.
- Chord blocks can show their beat counts, and analysis navigation behaves more
  consistently across beat, chord, lyric, and time modes.
- Practice controls and the desktop release variants have been refined.
- Playback interface rendering is bounded to reduce avoidable GPU and UI load.
- Worker contract tests remain dependency-free, and CI now uses the same Python
  3.13 generation required by the packaged workers on every platform.
- Light release jobs run only the shared application checks and Light leakage
  verifiers; model-worker suites and accelerator qualifications run only for
  Full editions.
- Linux Light is available as DEB and AppImage, while Linux GPU editions are
  published as DEB.

See the README for detailed minimum configurations and installation guidance.
