# Shared Python runtime

Release assembly installs one target-native CPython 3.13 runtime here. It
contains the chord/downbeat worker on every platform, plus the MLX stem worker
on Apple Silicon or the CPU-only Torch stem worker on Linux and Windows.

Generate and verify the ignored runtime with:

```sh
npm run python:runtime
npm run verify:chord-release
npm run verify:stem-release
```

The runtime is rebuilt independently on each target and uv is not shipped in
the application.
