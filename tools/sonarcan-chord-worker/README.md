# SonArcan chord worker

This is SonArcan's production chord-recognition worker. It invokes the pinned
LV-Chordia model directly and emits one bounded JSON document for Rust to
validate. It contains no SonArcan tonal rules, beat correction, stem fusion, or
neighbour-aware chord reinterpretation.

Run the contract tests with:

```sh
npm run test:chords
```

Run inference during development with:

```sh
uv run --project tools/sonarcan-chord-worker --locked \
  python -m sonarcan_chord_worker.worker /absolute/path/to/audio.mp3
```

The worker requires Python 3.12. Release builds use the self-contained runtime
assembled by `npm run chords:runtime` rather than a Python installation on the
user's Mac.
