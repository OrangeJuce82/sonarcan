# SonArcan 0.1.0-beta.28

This beta makes Full-edition installation smaller and adds a guided, verified
first-run setup for the analysis models. It also expands four-stem separation,
improves synchronized practice tools, and reduces playback rendering load.

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
drums, bass, and other.

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
files, accompanied by a platform/backend/format-specific `SHA256SUMS` file.
Download every part for one edition and package format, plus its matching
`SHA256SUMS` file, into the same directory. Linux Light is published as DEB,
RPM, and AppImage; macOS and Windows Light remain conventional single-file
downloads.

### Linux NVIDIA or AMD

Open a terminal in the download directory, set `backend` to `NVIDIA` or `AMD`,
then run:

```bash
cd ~/Downloads
version=v0.1.0-beta.28
backend=NVIDIA # Replace with AMD for the ROCm release.
sha256sum --check "SHA256SUMS-Linux-${backend}-GPU-DEB.txt"
cat "SonArcan-Linux-x86_64-${backend}-GPU-${version}.deb".part-* > "SonArcan-${backend}-GPU.deb"
sudo apt install "./SonArcan-${backend}-GPU.deb"
```

Do not install the reconstructed package if `sha256sum` reports a missing file
or a checksum failure.

On Fedora with a CUDA 12.6-compatible NVIDIA setup, download the NVIDIA RPM
parts instead and run:

```bash
cd ~/Downloads
version=v0.1.0-beta.28
backend=NVIDIA
sha256sum --check "SHA256SUMS-Linux-${backend}-GPU-RPM.txt"
cat "SonArcan-Linux-x86_64-${backend}-GPU-${version}.rpm".part-* > "SonArcan-${backend}-GPU.rpm"
sudo dnf install "./SonArcan-${backend}-GPU.rpm"
```

For Light on Fedora, download the single RPM and install it with
`sudo dnf install ./<downloaded-file>.rpm`. The Light AppImage is the portable
alternative: make it executable with `chmod +x ./<downloaded-file>.AppImage`,
then launch it directly. The AMD RPM is intended for RPM-based systems that AMD
lists as compatible with ROCm 7.2; Fedora is not an officially supported ROCm
7.2 host, so Fedora users with an AMD GPU should choose Light.

### Windows NVIDIA

Open PowerShell in the download directory and run:

```powershell
$ErrorActionPreference = 'Stop'
Set-Location "$HOME\Downloads"
$version = 'v0.1.0-beta.28'
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

- Synchronized lyrics are displayed directly on the waveform.
- Chord blocks can show their beat counts, and analysis navigation behaves more
  consistently across beat, chord, lyric, and time modes.
- Practice controls and the desktop release variants have been refined.
- Playback interface rendering is bounded to reduce avoidable GPU and UI load.
- Worker contract tests remain dependency-free, and CI now uses the same Python
  3.13 generation required by the packaged workers on every platform.

See the README for detailed minimum configurations and installation guidance.
