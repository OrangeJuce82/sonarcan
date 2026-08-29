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
- Optional local HTDemucs 6s separation through Apple MLX with a cached six-channel Rust mixer
- Unified local/YouTube Import Center with bounded background downloads and one-pass conversion
- English/French preferences, explicit paste/drop import, and native desktop menus
- A colored in-app console combining Rust and WebView logs for diagnostics
- Per-track practice-state persistence
- Structured diagnostics and project-format tests

Chord analysis, editable time signatures, and grid gestures are tracked in the [development roadmap](docs/ROADMAP.md).

## Development

### Requirements

- macOS 14 or later on Apple Silicon for the complete application, including MLX stems;
- Node.js 22 or later and npm;
- stable Rust 1.78 or later with Cargo;
- uv exactly `0.9.26`;
- the [Tauri 2 macOS prerequisites](https://v2.tauri.app/start/prerequisites/).

The worker pins CPython `3.13.5` in
[`tools/sonarcan-mlx-worker/.python-version`](tools/sonarcan-mlx-worker/.python-version).
Install the pinned tools when needed:

```bash
rustup toolchain install stable
curl -LsSf https://astral.sh/uv/0.9.26/install.sh | sh
uv python install 3.13.5
```

### First checkout

```bash
npm ci
npm run mlx:sync
npm run mlx:model
npm run quality
```

`mlx:sync` creates the locked development environment used by `demucs-mlx`.
`mlx:model` converts and verifies the pinned `htdemucs_6s` six-stem model. Both
commands are required before testing stems from a fresh checkout.

### Run the application

Complete Tauri application with the Rust audio engine and MLX stems:

```bash
npm run tauri dev
```

Enable detailed Rust diagnostics when investigating a native issue:

```bash
RUST_LOG=sonarcan=debug npm run tauri dev
```

Frontend-only development does not start the Tauri/Rust backend:

```bash
npm run dev
```

Run the complete local gate before handing off a change:

```bash
npm run quality
```

This runs Svelte diagnostics, TypeScript tests, MLX worker contract tests, the
production frontend build, Rust formatting, Clippy, and all Rust tests. The
network-backed dependency audit requires OSV-Scanner and is only needed after a
dependency change:

```bash
npm run security
```

## Build and release on macOS

### Local Apple-silicon release

The release embeds its own relocatable CPython `3.13.5`, locked MLX packages,
worker, and verified `htdemucs_6s` model. uv and Python are never installed or
downloaded on the user's Mac.

Prepare and verify those resources, then build the `.app` and DMG:

```bash
npm ci
npm run mlx:sync
npm run mlx:model
npm run mlx:runtime
npm run verify:mlx-release
npm run quality
npm run tauri build -- --target aarch64-apple-darwin --bundles app,dmg
```

Generated artifacts are written to:

```text
src-tauri/target/aarch64-apple-darwin/release/bundle/macos/SonArcan.app
src-tauri/target/aarch64-apple-darwin/release/bundle/dmg/SonArcan_<version>_aarch64.dmg
```

During the first local DMG build, macOS may ask whether the terminal or build
host may control Finder. Allow this under **System Settings → Privacy & Security
→ Automation**: Finder writes the DMG background and icon positions to its
`.DS_Store`. If permission is denied, the `.app` is still assembled but DMG
creation ends with Apple event error `-1743`.

The local command can create an unsigned qualification build. Public downloads
must be Developer ID signed and Apple-notarized through the release workflow.
The DMG uses the project background at
[`src-tauri/dmg/background-v2.png`](src-tauri/dmg/background-v2.png).

### GitHub release

The repository workflow
[`release-macos.yml`](.github/workflows/release-macos.yml) runs when a matching
semantic-version tag is pushed. Keep the version identical in `package.json`,
`src-tauri/Cargo.toml`, and `src-tauri/tauri.conf.json`, then run:

```bash
npm run verify:release-version
git tag -s v0.2.0 -m "SonArcan 0.2.0"
git push origin v0.2.0
```

GitHub Actions rebuilds the pinned runtime/model, runs the quality gate, signs
and notarizes the application, creates the DMG, and uploads a draft GitHub
Release for manual validation. Apple certificate and notarization secrets must
first be configured as described in [the release guide](docs/RELEASING.md).

The supported complete release is `aarch64-apple-darwin` on macOS 14+. An Intel
Tauri shell can still be compiled separately, but Apple MLX and therefore the
current stems feature are unavailable. A universal DMG must not combine the ARM
MLX runtime with an Intel executable.

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
- [macOS release and GitHub deployment](docs/RELEASING.md)
- [Contributing](CONTRIBUTING.md) · [Security](SECURITY.md) · [Roadmap](docs/ROADMAP.md)
- [Practice workflow](docs/PRACTICE_WORKFLOW.md) · [Project management](docs/PROJECT_MANAGEMENT.md)
- [Waveforms](docs/WAVEFORM.md) · [Real-time audio](docs/AUDIO_ENGINE.md) · [Native menus](docs/NATIVE_MENUS.md)
- [Product specification](CAHIER_DES_CHARGES.md) — French specification; implementation documentation and code are written in English.

## License

SonArcan source code is available under the [MIT License](LICENSE). See
[third-party notices](THIRD_PARTY_NOTICES.md) for the audio, model, desktop,
and import components used by the application.
