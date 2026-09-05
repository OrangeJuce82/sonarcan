<div align="center">
  <img src="docs/assets/sonarcan-rounded.png" alt="SonArcan app icon" width="184">

  # SonArcan

  **Dive into the music.**

  A focused, local-first desktop workspace to learn, analyze, isolate, and rehearse music.

  ![Desktop platforms](https://img.shields.io/badge/desktop-macOS%20%7C%20Windows%20%7C%20Linux-0ea5e9)
  [![MIT License](https://img.shields.io/badge/license-MIT-22c55e.svg)](LICENSE)
  [![Support on PayPal](https://img.shields.io/badge/Support-PayPal-0070ba?logo=paypal&logoColor=white)](https://www.paypal.com/paypalme/z5omes)
</div>

> [!NOTE]
> SonArcan Full targets Apple Silicon, NVIDIA GPUs on Windows/Linux, and AMD
> GPUs on Linux. SonArcan Light targets all supported platforms without
> bundling the heavy analysis models or runtimes.

## Minimum and recommended configuration

| Platform | Minimum for playback, lyrics, spectrum and meters | Required for Beat, Chords and Mix | Recommended |
| --- | --- | --- | --- |
| macOS Apple Silicon Full | macOS 14, M1, 8 GB RAM | A working, qualified MLX/MPS accelerator | M2 or newer, 16 GB RAM |
| macOS Apple Silicon Light | macOS 14, M1, 8 GB RAM | Not included | 16 GB RAM for large projects |
| macOS Intel Light | macOS 12, Intel x64, 8 GB RAM | Not included | macOS 13+, 16 GB RAM |
| Windows NVIDIA GPU | Windows 10 1903 or newer, x64, 16 GB RAM | NVIDIA GPU and current driver compatible with CUDA 12.6; startup model probe must pass | Windows 11, 8 GB GPU memory, 32 GB RAM |
| Windows Light | Windows 10 1903 or newer, x64, 8 GB RAM | Not included | Windows 11, 16 GB RAM |
| Linux NVIDIA GPU | Ubuntu 22.04-compatible x64 desktop, glibc 2.35+, 16 GB RAM | NVIDIA GPU and current driver compatible with CUDA 12.6; startup model probe must pass | Ubuntu 22.04 or newer, 8 GB GPU memory, 32 GB RAM |
| Linux AMD GPU | Ubuntu 22.04-compatible x64 desktop, glibc 2.35+, 16 GB RAM | AMD GPU and driver supported by ROCm 7.2; startup model probe must pass | Ubuntu 22.04.5 or newer, 8 GB GPU memory, 32 GB RAM |
| Linux Light | Ubuntu 22.04-compatible x64 desktop, glibc 2.35+, 8 GB RAM | Not included | Ubuntu 22.04 or newer, 16 GB RAM |

SonArcan Full checks the production accelerator and model graphs once when the
application starts. If no compatible and qualified GPU backend is available,
it enters a safe degraded mode for the complete session: Beat, Chords, Mix,
BPM, and the analysis-driven metronome are not shown and cannot be started.
Playback, time navigation, lyrics, spectrum, and the stereo meter remain
available. The explanatory message is shown only once per user profile.

SonArcan Light is a deliberately smaller edition. It keeps playback, imports,
projects, time navigation, loops, training, pitch/tempo controls, lyrics,
spectrum, and the stereo meter, but does not include Beat, Chords, Mix, BPM, or
the analysis metronome. Full and Light use the same `.sac` project format and
never delete analysis data produced by another edition.

Windows and Linux GPU bundles contain a pinned accelerator-specific PyTorch
runtime: CUDA 12.6 for NVIDIA, and ROCm 7.2 for AMD on Linux. PyTorch exposes
ROCm through its CUDA-compatible API, but the packages and installer remain
separate. Windows AMD and Intel GPUs are not qualified in this beta; choose
Light on those systems. SonArcan never silently falls back to the CPU for heavy
analysis jobs.

GPU runtimes exceed GitHub's 2 GiB limit for a single release asset. Their
portable package is consequently published as numbered `part-000`, `part-001`,
… files plus a `SHA256SUMS` file. Download every part for the chosen platform
and backend into one directory, verify the checksums, then concatenate them in
name order. On Linux this reconstructs an installable `.deb`:

```bash
cd ~/Downloads
version=v0.1.0-beta.24
backend=NVIDIA # Replace with AMD for the ROCm release.
sha256sum --check "SHA256SUMS-Linux-${backend}-GPU.txt"
cat "SonArcan-Linux-x86_64-${backend}-GPU-${version}.deb".part-* > "SonArcan-${backend}-GPU.deb"
sudo apt install "./SonArcan-${backend}-GPU.deb"
```

Replace `NVIDIA` with `AMD` for the ROCm build. On Windows, verify the hashes
with `Get-FileHash`, concatenate the numbered files as binary data, then extract
the reconstructed `.zip` and launch `SonArcan NVIDIA GPU.exe`:

```powershell
$ErrorActionPreference = 'Stop'
Set-Location "$HOME\Downloads"
$version = 'v0.1.0-beta.24'
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

Light releases and both macOS releases remain ordinary one-file installers.

## Choose an edition

| Feature | Full / GPU | Light or degraded mode |
| --- | --- | --- |
| Playback, pitch/tempo, loops and trainer | Yes | Yes |
| Lyrics | Yes | Yes; moves into the Mix column |
| Spectrum and stereo meters | Yes | Yes |
| Beat timeline, BPM and analysis metronome | Yes | No |
| Chord detection and navigation | Yes | No |
| Piano, guitar and ukulele chord views | Yes | No; assets are excluded |
| Six-stem Mix and export | Yes | No |

Download **SonArcan** for an Apple-silicon Mac, **SonArcan NVIDIA GPU** for a
supported NVIDIA Windows/Linux computer, **SonArcan AMD GPU** for supported AMD
Linux hardware, or **SonArcan Light** everywhere else. Detailed notes for the
current beta are in [RELEASE_NOTES.md](RELEASE_NOTES.md).

SonArcan is made for musicians who want the useful parts of an audio workstation
without the weight of a full DAW. Import a setlist, understand the music, build
loops, and practice while keeping projects portable and data on your computer.
Full editions also analyze the music and isolate its parts locally.

## ✨ Highlights

- Import WAV, MP3, FLAC, local files, or YouTube sources into portable `.sac` projects.
- Play, seek, change gain, and create seamless A/B loops through a dedicated Rust audio engine.
- Slow down or speed up from 50–200% independently of pitch, with ±12 semitones and fine cent correction.
- In Full editions, detect BPM, beats, downbeats, and timed chords locally, with detected timelines and source-aware disposable caches.
- In Full editions, separate six stems locally with HTDemucs 6s through MLX or portable Torch, then mix or export them.
- Practice with a progressive loop trainer, waveform, spectrum, and stereo meter; Full editions add the synchronized analysis metronome.
- Keep per-track practice settings, recent projects, diagnostics, and a multilingual interface.

For planned work and known product directions, see the [roadmap](docs/ROADMAP.md).

## 🧰 Built with—and grateful for

SonArcan stands on an outstanding open-source audio and desktop ecosystem:

| Area | Tools and projects |
| --- | --- |
| Desktop & interface | [Rust](https://www.rust-lang.org/), [Tauri 2](https://tauri.app/), [Svelte 5](https://svelte.dev/), [TypeScript](https://www.typescriptlang.org/), [Vite](https://vite.dev/) |
| Real-time audio | [CPAL](https://github.com/RustAudio/cpal), [Symphonia](https://github.com/pdeljanov/Symphonia), [Signalsmith Stretch](https://signalsmith-audio.co.uk/code/stretch/), [RustFFT](https://github.com/ejmahler/RustFFT) |
| Source separation | [Apple MLX](https://github.com/ml-explore/mlx), [demucs-mlx](https://pypi.org/project/demucs-mlx/), [PyTorch](https://pytorch.org/), [HTDemucs 6s](https://github.com/facebookresearch/demucs), [Python](https://www.python.org/) |
| Musical analysis | [LV-Chordia](https://github.com/openmirlab/lv-chordia), [Beat This!](https://github.com/CPJKU/beat_this), [PyTorch](https://pytorch.org/), [librosa](https://librosa.org/) |
| Import & media | [FFmpeg](https://ffmpeg.org/), [LAME](https://lame.sourceforge.io/), [yt-dlp](https://github.com/yt-dlp/yt-dlp) |
| Reproducible builds | [npm](https://www.npmjs.com/), [Cargo](https://doc.rust-lang.org/cargo/), [uv](https://docs.astral.sh/uv/), GitHub Actions |

A heartfelt thank-you to every maintainer, researcher, tester, and contributor behind these projects. Their work makes SonArcan possible. Licensing and attribution details are collected in [Third-party notices](THIRD_PARTY_NOTICES.md).

## 🚀 Run from source

### Requirements

- macOS 14+ on Apple Silicon, macOS 12+ on Intel for Light, Windows x64, or a Linux x64 desktop supported by Tauri 2
- Node.js 22+ and npm
- Stable Rust 1.78+ with Cargo
- `uv` exactly `0.9.26`
- FFmpeg and FFprobe on `PATH` for development fallback
- [Tauri 2 prerequisites for the target OS](https://v2.tauri.app/start/prerequisites/)

Install the native tools for your operating system first. The FFmpeg command
below is macOS-specific; use your distribution package manager on Linux or put
FFmpeg and FFprobe on `PATH` on Windows.

```bash
rustup toolchain install stable
brew install ffmpeg
curl -LsSf https://astral.sh/uv/0.9.26/install.sh | sh
uv python install 3.13.5
```

On Windows PowerShell, install the same pinned `uv` release with:

```powershell
powershell -ExecutionPolicy ByPass -c "irm https://astral.sh/uv/0.9.26/install.ps1 | iex"
uv python install 3.13.5
```

Every profile starts with a fresh checkout and the frontend dependencies:

```bash
npm ci
```

Choose exactly one of the following profiles.

### Apple Silicon Full: MLX + MPS

This is the only source profile that prepares the shared HTDemucs model. MLX
handles six-stem separation; PyTorch MPS handles Beat and Chords.

```bash
npm run mlx:sync
npm run mlx:model
npm run chords:downbeat-model
npm run python:runtime
npm run ytdlp:search
npm run ffmpeg:runtime
npm run quality
npm run tauri dev -- --config src-tauri/tauri.macos-arm.conf.json
```

### Windows or Linux Full: Torch GPU

Run `npm run stems:sync`, not `mlx:sync`. These profiles require the verified
`config.json` and `htdemucs_6s.safetensors` produced by the Apple Silicon
`prepare-model` release job under `src-tauri/resources/models/demucs-mlx/`.
Copying an arbitrary model into that directory will fail its identity checks.

For NVIDIA on Linux or in a Unix-like Windows shell:

```bash
export SONARCAN_EDITION=full
export SONARCAN_GPU_BACKEND=nvidia
npm run stems:sync
npm run chords:downbeat-model
npm run python:runtime
npm run verify:gpu-runtime
npm run ytdlp:search
npm run ffmpeg:runtime
npm run quality
npm run tauri dev -- --config src-tauri/tauri.nvidia-gpu.conf.json
```

For AMD ROCm on Linux, replace `nvidia` with `amd` and use
`src-tauri/tauri.amd-gpu.conf.json`. Windows AMD and Intel GPU Full profiles are
not qualified. Native Windows PowerShell sets the NVIDIA environment with:

```powershell
$env:SONARCAN_EDITION = 'full'
$env:SONARCAN_GPU_BACKEND = 'nvidia'
```

Then run the same `npm` commands without the two `export` lines.

### Light

Light does not prepare MLX, Torch, Beat This!, LV-Chordia, or HTDemucs. It keeps
playback, projects, imports, lyrics, spectrum, meters, and time-based practice.

```bash
export SONARCAN_EDITION=light
npm run ytdlp:search
npm run python:light-runtime
npm run verify:light-runtime
npm run ffmpeg:runtime
npm run verify:ffmpeg-release
npm run quality
npm run tauri dev -- --config src-tauri/tauri.portable.conf.json
```

Use `src-tauri/tauri.macos-arm-light.conf.json` or
`src-tauri/tauri.macos-intel-light.conf.json` instead on macOS. In PowerShell,
set the edition with `$env:SONARCAN_EDITION = 'light'` and omit the `export`
line.

`npm run dev` starts only the frontend. Playback, project management, native
menus, analysis, and stems require `npm run tauri dev` and the Rust backend.

## 📦 Desktop bundles

The contents of a desktop bundle depend on its edition. End users never need to
install Python, `uv`, FFmpeg, or model dependencies themselves.

| Build profile | Targets | Analysis implementation | Bundled resources |
| --- | --- | --- | --- |
| **MLX Full** | Apple Silicon | `sonarcan-mlx-worker` for six-stem separation and PyTorch MPS for Beat/Chords | MLX/MPS runtime, HTDemucs, Beat This!, LV-Chordia, FFmpeg and yt-dlp |
| **Torch GPU Full** | Windows/Linux NVIDIA; Linux AMD | `sonarcan-torch-worker` using CUDA 12.6 or ROCm 7.2 for six-stem separation and Beat/Chords | Backend-specific PyTorch runtime, HTDemucs, Beat This!, LV-Chordia, FFmpeg and yt-dlp |
| **Light** | Apple Silicon, Intel Mac, Windows x64 and Linux x64 | No ML worker and no heavy analysis | Minimal Python runtime for yt-dlp plus FFmpeg; no Torch, MLX or analysis models |

The tag workflow is the authoritative cross-platform build recipe: it chooses
the correct worker, accelerator runtime, edition environment, resources, and
Tauri configuration for each target. It verifies the packaged resources before
leaving the release as a draft for manual smoke testing.

GitHub macOS builds are ad-hoc signed so every embedded executable has a
consistent code signature, but they are not notarized or identified by Apple.
On first launch, users must explicitly allow SonArcan under **System Settings →
Privacy & Security → Open Anyway**. The complete workflow and trust model are
documented in the [release guide](docs/RELEASING.md).

## 🎼 Portable projects

A `.sac` project is an inspectable directory. macOS presents it as a single
SonArcan document package; Windows and Linux keep the same portable contents:

```text
My-Band.sac/
├── project.json
├── Audio/
├── Stems/
├── Analysis/
├── Chords/
├── Lyrics/
└── Cache/
```

The manifest stays human-readable. Original media and user-authored data are kept separate from disposable analysis and cache files.

## 📚 Documentation

- [Architecture](docs/ARCHITECTURE.md) · [Development](docs/DEVELOPMENT.md) · [Quality](docs/QUALITY.md)
- [Real-time audio](docs/AUDIO_ENGINE.md) · [Chord analysis](docs/CHORD_ANALYSIS.md) · [Stem separation](docs/STEM_SEPARATION.md)
- [Practice workflow](docs/PRACTICE_WORKFLOW.md) · [Project management](docs/PROJECT_MANAGEMENT.md) · [Waveforms](docs/WAVEFORM.md)
- [Competitive analysis](docs/COMPETITIVE_ANALYSIS.md) · [Roadmap](docs/ROADMAP.md) · [Release guide](docs/RELEASING.md)
- [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md)

## 🤝 Contributors

SonArcan grows through code, ideas, testing, musical feedback, and careful open-source work. Thank you—warmly—to everyone who has helped shape it.

Current repository contributors:

- [OrangeJuce82](https://github.com/OrangeJuce82) — creator and maintainer

Want to join the list? Read [CONTRIBUTING.md](CONTRIBUTING.md), open an issue, or submit a focused pull request. Every thoughtful contribution is welcome.

## ☕ Support SonArcan

If SonArcan helps your practice sessions and you would like to support its continued development, you can offer the project a coffee through PayPal:

<div align="center">
  <a href="https://www.paypal.com/paypalme/z5omes">
    <img src="https://img.shields.io/badge/Buy_me_a_coffee-Support_on_PayPal-0070ba?logo=paypal&logoColor=white" alt="Support SonArcan on PayPal">
  </a>
</div>

Thank you for listening, testing, contributing, sharing, and supporting the project. 💙

## License

SonArcan is available under the [MIT License](LICENSE). Third-party components and models retain their own licenses; see [THIRD_PARTY_NOTICES.md](THIRD_PARTY_NOTICES.md).
