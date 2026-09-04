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
> SonArcan targets macOS 14+ on Apple Silicon, Windows x64, and Linux x64. MLX
> remains the optimized stem backend on Apple Silicon; Windows and Linux use the
> portable Torch backend with the exact same bundled model.

SonArcan is made for musicians who want the useful parts of an audio workstation without the weight of a full DAW. Import a setlist, understand the music, isolate parts, build loops, and practice—all while keeping projects portable and data on your Mac.

## ✨ Highlights

- Import WAV, MP3, FLAC, local files, or YouTube sources into portable `.sac` projects.
- Play, seek, change gain, and create seamless A/B loops through a dedicated Rust audio engine.
- Slow down or speed up from 50–200% independently of pitch, with ±12 semitones and fine cent correction.
- Detect BPM, beats, downbeats, and timed chords locally, with detected timelines and source-aware disposable caches.
- Separate six stems locally with HTDemucs 6s through MLX or portable Torch, then mix or export them.
- Practice with a progressive loop trainer, synchronized metronome, waveform, spectrum, and stereo meter.
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

- macOS 14+ on Apple Silicon, Windows x64, or a Linux x64 desktop supported by Tauri 2
- Node.js 22+ and npm
- Stable Rust 1.78+ with Cargo
- `uv` exactly `0.9.26`
- FFmpeg for development
- [Tauri 2 prerequisites for the target OS](https://v2.tauri.app/start/prerequisites/)

Install the native tools and pinned Python versions. Apple Silicon development
also prepares MLX; other targets prepare the Torch worker with
`npm run stems:sync`.

```bash
rustup toolchain install stable
brew install ffmpeg
curl -LsSf https://astral.sh/uv/0.9.26/install.sh | sh
uv python install 3.13.5
```

Prepare a fresh checkout:

```bash
npm ci
npm run stems:sync # use mlx:sync on Apple Silicon
npm run test:chords
npm run mlx:model
npm run quality
```

Start the complete desktop application:

```bash
npm run tauri dev
```

`npm run dev` starts only the frontend; playback, project management, analysis, and stems require Tauri and the Rust backend.

## 📦 Desktop bundles

Releases bundle pinned Python workers, analysis models, and a target-native
FFmpeg runtime. End users do not need to install Python, `uv`, FFmpeg, or model
dependencies. The tag workflow builds three platform releases: macOS ARM64,
Windows x64, and a Linux x64 DEB.

```bash
npm ci
npm run mlx:sync
npm run mlx:model
npm run python:runtime
npm run ffmpeg:runtime
npm run quality
npm run build:macos:dmg
```

The resulting Apple Silicon DMG is written under `src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/`. GitHub release builds are ad-hoc signed so every embedded executable has a consistent code signature, but they are not notarized or identified by Apple. On first launch, users must explicitly allow SonArcan under **System Settings → Privacy & Security → Open Anyway**. The complete workflow and trust model are documented in the [release guide](docs/RELEASING.md).

## 🎼 Portable projects

A `.sac` project is an inspectable directory that macOS presents as a single SonArcan document:

```text
My-Band.sac/
├── project.json
├── Audio/
├── Stems/
├── Analysis/
├── Chords/
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
