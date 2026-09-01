# Third-party notices

SonArcan source code is licensed under the MIT License. Binary distributions
also include or interact with third-party components under their own licenses.
Their license terms remain authoritative.

## Audio processing

- Signalsmith Stretch — MIT License — copyright Signalsmith Audio Ltd.
- Symphonia — MPL-2.0 License.
- CPAL — Apache-2.0 License.

## Stem separation

- `demucs-mlx` — MIT License.
- Apple MLX — MIT License.
- Python — Python Software Foundation License.
- Demucs source architecture — MIT License, Meta Platforms, Inc.

The HTDemucs 6s model file is not part of the SonArcan source repository or the
SonArcan license. Release builders supply it as a verified binary resource. Any
model-specific terms supplied by its distributor remain authoritative and must
be reviewed before public distribution.

## Chord recognition

- LV-Chordia source and pretrained checkpoints — MIT License — Music X Lab.
- `@tombatossals/chords-db` guitar, ukulele, and piano position corpus — MIT
  License — copyright David Rubert. SonArcan pins source revision
  `df06fa7b425cf5fd29485ff6591236b3557e3fac`; the complete license is retained
  in `docs/licenses/chords-db-MIT.txt`.
- PyTorch — BSD-3-Clause License.
- Librosa — ISC License.

These dependencies and checkpoints form SonArcan's local chord-recognition
runtime and are shipped with the desktop application. The worker project pins
source revision
`9d7de7bbf45efa6731ec8dc62d35280f141c0702` and verifies all five checkpoint
SHA-256 digests before loading them.

## Desktop and import tooling

- Tauri — Apache License 2.0 or MIT License.
- Svelte — MIT License.
- `yt-dlp` — The Unlicense.
- FFmpeg 8.0.3 — LGPL-2.1-or-later. Release builds include a static ARM64
  command-line runtime and its complete LGPL 2.1 license text.
- LAME 3.100 — LGPL-2.0-or-later. It provides MP3 encoding in the bundled
  FFmpeg runtime; its complete license text is shipped beside the executable.

This notice is informational and is not a substitute for the complete notices
shipped by each dependency.
