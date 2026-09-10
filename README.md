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
> SonArcan is distributed as one application. It enables local analysis only
> when a qualified GPU backend passes its startup probe; otherwise it starts in
> a safe simplified mode.

## Minimum and recommended configuration

| Platform | Minimum for playback, lyrics, spectrum and meters | Required for Beat, Chords and Mix | Recommended |
| --- | --- | --- | --- |
| macOS Apple Silicon | macOS 14, M1, 8 GB RAM | A working, qualified MLX/MPS accelerator | M2 or newer, 16 GB RAM |
| Windows NVIDIA package | Windows 10 1903 or newer, x64, 8 GB RAM | NVIDIA GPU and current driver compatible with CUDA 12.6; startup model probe must pass | Windows 11, 8 GB GPU memory, 32 GB RAM |
| Linux NVIDIA package | DEB-compatible x64 desktop, glibc 2.35+, 8 GB RAM | NVIDIA GPU and current driver compatible with CUDA 12.6; startup model probe must pass | 8 GB GPU memory, 32 GB RAM |
| Linux AMD package | DEB-compatible x64 desktop, glibc 2.35+, 8 GB RAM | AMD GPU and driver supported by ROCm 7.2; startup model probe must pass | 8 GB GPU memory, 32 GB RAM |

SonArcan checks the production accelerator and model graphs once when the
application starts. If no compatible and qualified GPU backend is available,
it enters a safe degraded mode for the complete session: Beat, Chords, Mix,
BPM, and the analysis-driven metronome are not shown and cannot be started.
Playback, time navigation, lyrics, spectrum, and the stereo meter remain
available. The explanatory message is shown only once per user profile.

Windows and Linux GPU bundles contain a pinned accelerator-specific PyTorch
runtime: CUDA 12.6 for NVIDIA, and ROCm 7.2 for AMD on Linux. PyTorch exposes
ROCm through its CUDA-compatible API, but the packages and installer remain
separate. Windows AMD and Intel GPUs are not qualified in this beta; the Windows
NVIDIA package starts in simplified mode on those systems. SonArcan never
silently falls back to the CPU for heavy analysis jobs. Every package remains
usable without a qualified GPU in simplified mode and never attempts those jobs.

GPU runtimes exceed GitHub's 2 GiB limit for a single release asset. Each DEB
or portable archive is consequently published as numbered `part-000`,
`part-001`, … files plus a platform-specific `SHA256SUMS` file. Download every
part for the chosen platform and backend into one directory, verify
the checksums, then concatenate them in name order. On Debian or Ubuntu:

```bash
cd ~/Downloads
version=v0.1.1-beta.5
backend=NVIDIA # Replace with AMD for the ROCm release.
sha256sum --check "SHA256SUMS-Linux-${backend}-GPU-DEB.txt"
cat "SonArcan-Linux-x86_64-${backend}-GPU-${version}.deb".part-* > "SonArcan-${backend}-GPU.deb"
sudo apt install "./SonArcan-${backend}-GPU.deb"
```

Linux NVIDIA and AMD packages are distributed as multipart DEB files. On
Windows, verify the hashes with `Get-FileHash`, concatenate the numbered files
as binary data, then extract the reconstructed `.zip` and launch
`SonArcan NVIDIA GPU.exe`:

```powershell
$ErrorActionPreference = 'Stop'
Set-Location "$HOME\Downloads"
$version = 'v0.1.1-beta.5'
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

The macOS release remains an ordinary one-file installer.

## Runtime capabilities

| Feature | Qualified GPU mode | Simplified mode |
| --- | --- | --- |
| Playback, pitch/tempo, loops and trainer | Yes | Yes |
| Lyrics | Yes | Yes; moves into the Mix column |
| Spectrum and stereo meters | Yes | Yes |
| Beat timeline, BPM and analysis metronome | Yes | No |
| Chord detection and navigation | Yes | No |
| Piano, guitar and ukulele chord views | Yes | No |
| Four-stem Mix and export | Yes | No |

Download **SonArcan** for an Apple-silicon Mac, **SonArcan NVIDIA GPU** for
Windows or NVIDIA Linux, or **SonArcan AMD GPU** for AMD Linux. The selected
package automatically starts in simplified mode when its accelerator probe does
not succeed. Detailed notes for the current beta are in
[RELEASE_NOTES.md](RELEASE_NOTES.md).

SonArcan is made for musicians who want the useful parts of an audio workstation
without the weight of a full DAW. Import a setlist, understand the music, build
loops, and practice while keeping projects portable and data on your computer.
Qualified GPU builds also analyze the music and isolate its parts locally.

## ✨ Highlights

- Import WAV, MP3, FLAC, local files, or public yt-dlp sources such as YouTube,
  SoundCloud, Bandcamp, and Mixcloud into portable `.sac` projects.
- Search by title on YouTube or SoundCloud, with provider logos and safe links
  back to every recognized public source.
- Edit markers, chords, and synchronized lyrics directly beside the waveform:
  resize block edges, double-click text, or use the explicit right-click menu.
  Chords use the same searchable option list as the grid. New blocks start at
  the playhead and extend to the next block or track end. Source chapters are
  imported as markers when the provider publishes them.
- Play, seek, change gain, and create seamless A/B loops through a dedicated Rust audio engine.
- Slow down or speed up from 50–200% independently of pitch, with ±12 semitones and fine cent correction.
- With a qualified GPU, detect BPM, beats, downbeats, and timed chords locally, with detected timelines and source-aware disposable caches.
- With a qualified GPU, separate vocals, drums, bass, and other locally with Fast
  HTDemucs or HQ SCNet Large through MLX or Torch, then mix or export them.
- Practice with a progressive loop trainer, waveform, spectrum, and stereo meter; qualified GPU mode adds the synchronized analysis metronome.
- Keep per-track practice settings, recent projects, diagnostics, and a multilingual interface.

For planned work and known product directions, see the [roadmap](docs/ROADMAP.md).

## 🧰 Built with—and grateful for

SonArcan stands on an outstanding open-source audio and desktop ecosystem:

| Area | Tools and projects |
| --- | --- |
| Desktop & interface | [Rust](https://www.rust-lang.org/), [Tauri 2](https://tauri.app/), [Svelte 5](https://svelte.dev/), [TypeScript](https://www.typescriptlang.org/), [Vite](https://vite.dev/) |
| Real-time audio | [CPAL](https://github.com/RustAudio/cpal), [Symphonia](https://github.com/pdeljanov/Symphonia), [Signalsmith Stretch](https://signalsmith-audio.co.uk/code/stretch/), [RustFFT](https://github.com/ejmahler/RustFFT) |
| Source separation | [Apple MLX](https://github.com/ml-explore/mlx), [demucs-mlx](https://pypi.org/project/demucs-mlx/), [PyTorch](https://pytorch.org/), [HTDemucs](https://github.com/facebookresearch/demucs), [SCNet](https://github.com/starrytong/SCNet), [Python](https://www.python.org/) |
| Musical analysis | [LV-Chordia](https://github.com/openmirlab/lv-chordia), [Beat This!](https://github.com/CPJKU/beat_this), [PyTorch](https://pytorch.org/), [librosa](https://librosa.org/) |
| Import & media | [FFmpeg](https://ffmpeg.org/), [LAME](https://lame.sourceforge.io/), [yt-dlp](https://github.com/yt-dlp/yt-dlp) |
| Reproducible builds | [npm](https://www.npmjs.com/), [Cargo](https://doc.rust-lang.org/cargo/), [uv](https://docs.astral.sh/uv/), GitHub Actions |

A heartfelt thank-you to every maintainer, researcher, tester, and contributor behind these projects. Their work makes SonArcan possible. Licensing and attribution details are collected in [Third-party notices](THIRD_PARTY_NOTICES.md).

## 🚀 Run from source

### Requirements

- macOS 14+ on Apple Silicon, Windows x64, or a Linux x64 desktop supported by Tauri 2
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

Choose the profile matching the target hardware.

### Apple Silicon: MLX + MPS

MLX handles four-stem separation; PyTorch MPS handles Beat and Chords. Stem
checkpoints are not part of the source tree or development preparation: the
Fast and HQ checkpoints are downloaded and verified during the first qualified launch.

```bash
npm run mlx:sync
npm run chords:downbeat-model
npm run python:runtime
npm run ytdlp:search
npm run ffmpeg:runtime
npm run quality
npm run tauri dev -- --config src-tauri/tauri.macos-arm.conf.json
```

### Windows or Linux: Torch GPU

Run `npm run stems:sync`, not `mlx:sync`. The runtime contains the selected
Torch backend and stem inference code, but no stem checkpoint. Fast HTDemucs and
HQ SCNet Large are downloaded into the application-data cache during the initial
model setup, then verified against their pinned sizes and SHA-256 values.

For NVIDIA on Linux or in a Unix-like Windows shell:

```bash
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
`src-tauri/tauri.amd-gpu.conf.json`. Windows AMD and Intel GPU profiles are
not qualified. Native Windows PowerShell sets the NVIDIA environment with:

```powershell
$env:SONARCAN_GPU_BACKEND = 'nvidia'
```

Then run the same `npm` commands without the `export` line.

### Simplified development without a qualified GPU

Leave `SONARCAN_GPU_BACKEND` unset. Development preparation skips accelerator
setup, and the application keeps playback, projects, imports, lyrics, spectrum,
meters, and time-based practice while disabling GPU analysis.

```bash
npm run ytdlp:search
npm run ffmpeg:runtime
npm run verify:ffmpeg-release
npm run quality
npm run tauri dev
```

`npm run dev` starts only the frontend. Playback, project management, native
menus, analysis, and stems require `npm run tauri dev` and the Rust backend.

## 📦 Desktop bundles

The contents of a desktop bundle depend on its target backend. End users never need to
install Python, `uv`, FFmpeg, or model dependencies themselves.

| Build profile | Targets | Analysis implementation | Bundled resources |
| --- | --- | --- | --- |
| **Apple MLX/MPS** | Apple Silicon | `sonarcan-mlx-worker` for four-stem separation and PyTorch MPS for Beat/Chords | MLX/MPS runtime, stem inference code, LV-Chordia, FFmpeg and yt-dlp; Beat This! and stem checkpoints install on first launch |
| **Torch GPU** | Windows/Linux NVIDIA; Linux AMD | `sonarcan-torch-worker` using CUDA 12.6 or ROCm 7.2 for four-stem separation and Beat/Chords | Backend-specific PyTorch runtime, stem inference code, LV-Chordia, FFmpeg and yt-dlp; Beat This! and stem checkpoints install on first launch |

The tag workflow is the authoritative cross-platform build recipe: it chooses
the correct worker, accelerator runtime, resources, and
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
