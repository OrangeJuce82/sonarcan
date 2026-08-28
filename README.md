<div align="center">
  <img src="src-tauri/icons/icon.png" alt="SonArcan logo" width="144" height="144">

  # SonArcan

  **Dive into the music.**

  A focused desktop workspace for learning, analyzing, and rehearsing music.

  Rust · Tauri 2 · Svelte · TypeScript
</div>

SonArcan helps musicians work through a band playlist without the complexity of a full DAW. Projects remain portable and inspectable, while playback and DSP stay inside a dedicated Rust real-time audio engine.

## What works today

- Portable `.sac` projects with WAV, MP3, and FLAC import
- Native project menus, Open Recent, Save As, and renaming
- Rust/CPAL playback with seek, gain, and seamless A/B loops
- Independent 50–200% time stretch and ±12-semitone pitch shift in Rust
- Fine pitch correction in 1-cent steps
- Automatic per-track BPM analysis with a persistent cache
- Editable beat grid and a synchronized Rust real-time metronome
- Progressive Loop Trainer and a Rust FFT spectrum worker
- Persistent decoded PCM caches for fast playlist navigation across sessions
- Cached, zoomable waveforms with an editable loop region
- Optional local HTDemucs four-stem separation with a cached Rust mixer
- Unified local/YouTube Import Center with bounded background downloads and one-pass conversion
- English/French preferences, smart clipboard detection, and native desktop menus
- A colored in-app console combining Rust and WebView logs for diagnostics
- Per-track practice-state persistence
- Structured diagnostics and project-format tests

Chord analysis, editable time signatures, and grid gestures are tracked in the [development roadmap](docs/ROADMAP.md).

## Development

Requirements: Node.js 22+, stable Rust 1.78+, and the [Tauri 2 platform prerequisites](https://v2.tauri.app/start/prerequisites/).

```bash
npm install
npm run quality
npm run tauri dev
```

Frontend-only development:

```bash
npm run dev
```

Dependency security audit (requires OSV-Scanner):

```bash
npm run security
```

## Project format

Each project is an inspectable directory:

```text
My-Band.sac/
├── project.json
├── Audio/
├── Stems/
├── Analysis/
├── Chords/
└── Cache/
```

The versioned `project.json` manifest is human-readable. Generated analysis and cache data remain separate from source media and user-authored project data.

## Documentation

- [Architecture](docs/ARCHITECTURE.md) · [Quality plan](docs/QUALITY.md) · [Development](docs/DEVELOPMENT.md)
- [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [Roadmap](docs/ROADMAP.md)
- [Practice workflow](docs/PRACTICE_WORKFLOW.md) · [Project management](docs/PROJECT_MANAGEMENT.md)
- [Waveforms](docs/WAVEFORM.md) · [Real-time audio](docs/AUDIO_ENGINE.md) · [Native menus](docs/NATIVE_MENUS.md)
- [Product specification](CAHIER_DES_CHARGES.md) — French specification; implementation documentation and code are written in English.

## License

SonArcan source code is available under the [MIT License](LICENSE). See
[third-party notices](THIRD_PARTY_NOTICES.md) for the audio, model, desktop,
and import components used by the application.
