# Shared Python runtime

Release assembly installs one Apple-Silicon CPython 3.13 runtime here. It
contains the MPS chord/downbeat worker and the MLX stem worker. Production
analysis never falls back to CPU inference.

Generate and verify the ignored runtime with:

```sh
npm run python:runtime
npm run verify:chord-release
npm run verify:stem-release
```

uv is not shipped in the application.
