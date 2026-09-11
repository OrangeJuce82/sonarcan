<div align="center">
  <img src="docs/assets/sonarcan-rounded.png" alt="SonArcan" width="168">

  # SonArcan

  **See the music. Isolate it. Slow it down. Play it better.**

  A local-first desktop workspace for learning, transcribing, and rehearsing music.

  ![Platforms](https://img.shields.io/badge/platforms-Apple%20Silicon%20%7C%20Debian-0ea5e9)
  [![CI](https://github.com/OrangeJuce82/sonarcan/actions/workflows/ci.yml/badge.svg)](https://github.com/OrangeJuce82/sonarcan/actions/workflows/ci.yml)
  [![MIT](https://img.shields.io/badge/license-MIT-22c55e.svg)](LICENSE)
</div>

SonArcan brings the useful parts of a music workstation into one focused app:
waveform navigation, pitch and tempo control, seamless loops, synchronized
lyrics, chord and beat analysis, and four-stem separation. Projects stay on
your computer in an inspectable `.sac` package.

## What it does

- Imports local audio and public sources supported by yt-dlp, including
  YouTube, SoundCloud, Bandcamp, and Mixcloud.
- Changes tempo from 50–200% independently of pitch, with semitone and cent
  controls.
- Creates seamless A/B loops and progressive practice sessions.
- Detects beats, downbeats, BPM, and timed chords.
- Separates vocals, drums, bass, and other with HTDemucs or SCNet.
- Edits chords, markers, and synchronized lyrics directly on the waveform.
- Shows piano, guitar, and ukulele positions for detected or edited chords.
- Keeps original media and authored data separate from disposable caches.

## Hardware support

SonArcan exposes its complete feature set or refuses to open. It does not ship a
reduced mode.

| Package | Required accelerator |
| --- | --- |
| macOS Apple Silicon | Apple GPU through Core ML |
| Debian x64 NVIDIA | Compatible NVIDIA GPU through CUDA |

The startup probe exercises the complete production inference path. If the
runtime, driver, device, or model cannot execute it, SonArcan displays an
incompatibility error and does not open the workspace.

Download current draft and published packages from
[GitHub Releases](https://github.com/OrangeJuce82/sonarcan/releases). Large
Debian GPU packages are split into verified parts; reconstruction commands are
included in each release note.

## Build from source

Requirements:

- Node.js 22 and npm
- stable Rust with Cargo
- `uv` 0.9.26 and Python 3.13.5
- FFmpeg and the [Tauri 2 prerequisites](https://v2.tauri.app/start/prerequisites/)

Install frontend dependencies and run the full quality gate:

```bash
npm ci
npm run quality
```

For frontend and non-inference development:

```bash
npm run ytdlp:search
npm run ffmpeg:runtime
npm run tauri dev
```

Apple Silicon development uses `npm run mlx:sync`; Debian GPU development uses
`npm run stems:sync` with `SONARCAN_GPU_BACKEND=nvidia`. See the
[development guide](docs/DEVELOPMENT.md) for complete environment setup.

## Architecture at a glance

- Svelte renders the interface and coordinates interaction.
- Rust owns playback, real-time DSP, project safety, and worker supervision.
- ML inference runs outside the audio callback in bounded native or isolated
  worker processes.
- ExecuTorch migration work keeps model programs outside the installer and
  targets a small selective runtime; MLX remains the qualified Apple path.

The audio callback performs no I/O, allocation, logging, locking, IPC, or model
inference. See [Architecture](docs/ARCHITECTURE.md),
[Audio engine](docs/AUDIO_ENGINE.md), and
[ExecuTorch deployment](docs/EXECUTORCH_RUNTIME.md).

## Project format

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

The manifest is human-readable. Original audio and user edits remain portable;
generated analysis can be recreated.

## Contributing

Focused fixes, tests, accessibility improvements, and careful performance work
are welcome. Please read [CONTRIBUTING.md](CONTRIBUTING.md) and
[SECURITY.md](SECURITY.md) before opening a pull request.

Useful references:

- [Project management](docs/PROJECT_MANAGEMENT.md)
- [Stem separation](docs/STEM_SEPARATION.md)
- [Chord analysis](docs/CHORD_ANALYSIS.md)
- [Release guide](docs/RELEASING.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

SonArcan is available under the [MIT license](LICENSE).
