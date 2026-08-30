# LV-Chordia runtime

Release builds require a local, generated Python 3.12 runtime in `runtime/`.
It contains the immutable LV-Chordia source revision, its five hash-verified
checkpoints, PyTorch, and the SonArcan worker. Generate it with:

```sh
npm run chords:runtime
```

The generated runtime is ignored by Git and validated before every Tauri
release build.
