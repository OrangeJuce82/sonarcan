# SonArcan 0.1.0-beta.26

This beta introduces platform-specific Full GPU and Light releases. Choose the
installer whose name matches your computer; all editions share the same `.sac`
project format.

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
version=v0.1.0-beta.26
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
version=v0.1.0-beta.26
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
$version = 'v0.1.0-beta.26'
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

## Other fixes

- Previous/next navigation now moves consistently to the adjacent beat, chord,
  or synchronized lyric marker, even from the middle of the current segment.
- The previous-track control restarts the current track when playback is past
  its first second, and moves to the preceding track when already at the start.
- Clicking a timed chord or lyric seeks to it without changing the selected
  navigation mode.
- Waveform clicks now seek to the exact pointed position without beat, chord,
  or lyric snapping; keyboard and transport navigation remain unchanged.
- Startup now loads and applies user preferences before rendering the workspace,
  then keeps a localized bootstrap screen visible until the recent or temporary
  project is active.
- Windows desktop builds no longer leave a console window open.
- Intel macOS FFmpeg assembly now falls back safely when NASM is unavailable.
- Full and Light editions use distinct product names and bundle identifiers.

See the README for detailed minimum configurations and installation guidance.
