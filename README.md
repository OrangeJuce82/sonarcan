<div align="center">
  <img src="docs/assets/sonarcan-rounded.png" alt="SonArcan app icon" width="184">

  # SonArcan

  **Dive into the music.**

  A local-first workspace to analyze, isolate, and rehearse music.

  ![Platform](https://img.shields.io/badge/platform-macOS%20Apple%20Silicon-0ea5e9)
  [![MIT License](https://img.shields.io/badge/license-MIT-22c55e.svg)](LICENSE)
</div>

SonArcan imports music into portable `.sac` projects and combines playback,
pitch and tempo controls, A/B loops, synchronized lyrics, chord and beat
analysis, and four-stem separation. Projects and analysis stay on the Mac.

## Minimum configuration

| | Minimum | Recommended |
| --- | --- | --- |
| Mac | Apple M1 or newer | Apple M2 or newer |
| macOS | 14 Sonoma | Latest stable macOS |
| Memory | 8 GB | 16 GB or more |
| Storage | 8 GB free | 16 GB free |

Only macOS on Apple Silicon is built, tested, and released. Chord/rhythm
analysis uses MPS and stem separation uses MLX. There is no CPU inference
fallback: when either accelerator probe fails, Beat, Chords, BPM, Mix, and the
analysis metronome remain disabled for that session.

The first qualified launch downloads and verifies the pinned Beat This!,
HTDemucs, and SCNet Large model files. An internet connection is required for
that initial setup and for online imports; normal playback and saved projects
remain local.

## Main features

- Import WAV, MP3, FLAC, local files, and supported public media links.
- Change speed from 50–200% independently of pitch, including fine cent tuning.
- Create seamless A/B loops and progressive practice sessions.
- Detect beats, downbeats, BPM, and timed chords locally through MPS.
- Separate vocals, drums, bass, and other with MLX, then mix or export them.
- Edit markers, chords, and synchronized lyrics beside the waveform.
- Keep per-track settings and media in inspectable `.sac` projects.

## Run from source

Requirements: an Apple-Silicon Mac, macOS 14+, Node.js 22+, stable Rust, Xcode
Command Line Tools, `uv` 0.9.26, and Python 3.13.5.

```bash
npm ci
uv python install 3.13.5
npm run ffmpeg:runtime
npm run ytdlp:search
npm run tauri dev -- --config src-tauri/tauri.macos-arm.conf.json
```

Run the complete repository gate before submitting a change:

```bash
npm run quality
```

Build the two supported desktop bundles with:

```bash
npm run build:macos:app
npm run build:macos:dmg
```

End users do not need Python, `uv`, Homebrew, or a system FFmpeg installation;
the release bundles its pinned runtime and media tools. GitHub builds are
ad-hoc signed but not notarized, so the first launch may require **System
Settings → Privacy & Security → Open Anyway**.

## Documentation

- [Architecture](docs/ARCHITECTURE.md)
- [Audio engine](docs/AUDIO_ENGINE.md)
- [Development](docs/DEVELOPMENT.md)
- [Release guide](docs/RELEASING.md)
- [Security](SECURITY.md)
- [Third-party notices](THIRD_PARTY_NOTICES.md)

SonArcan is available under the [MIT License](LICENSE).
