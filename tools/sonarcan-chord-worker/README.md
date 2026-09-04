# SonArcan chord worker

This is SonArcan's production chord-recognition worker. It invokes the pinned
LV-Chordia model directly and emits one bounded JSON document for Rust to
validate. A separate pinned Beat This! model produces frame-level beat and
downbeat predictions. The worker returns two timelines: Beat This!'s official
minimal output and its optional madmom DBN output. The madmom source revision is
pinned with the rest of the worker. Frame predictions never cross IPC. Beat
intervals feed the stable local BPM display, while downbeats are
used only by the beat grid and metronome. Beat This! never splits or alters the
LV-Chordia chord timeline. The worker contains no SonArcan tonal rules,
stem fusion, or neighbour-aware chord reinterpretation.

Run the contract tests with:

```sh
npm run test:chords
```

Run inference during development with:

```sh
uv run --project tools/sonarcan-chord-worker --locked \
  python -m sonarcan_chord_worker.worker \
  --downbeat-model src-tauri/resources/models/beat-this/final0.ckpt \
  /absolute/path/to/audio.mp3
```

The worker requires Python 3.13; `audioop-lts` supplies the module still used by
pydub. Release builds use the self-contained runtime assembled by
`npm run python:runtime` rather than a Python installation on the user's
computer.
